use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use reqwest::blocking::Client;

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    matcher::{SongCandidate, TrackMatchKey, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
    network::ResolutionDeadline,
    parser::parse_krc_lines,
    track::{TrackDescriptor, split_artists},
};
use super::file_index::DirectoryFileIndex;

mod online;

const MAX_KRC_BYTES: u64 = 2 * 1024 * 1024;
static CACHE_FILE_SUFFIX: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"(?i)-[0-9a-f]{32}-\d+-[0-9a-f]+$"));
static LOCAL_KRC_INDEX: LazyLock<DirectoryFileIndex<IndexedKrcFile>> =
    LazyLock::new(DirectoryFileIndex::new);

#[derive(Clone)]
struct IndexedKrcFile {
    path: PathBuf,
    artist: String,
    title: String,
}

/// 酷狗 20.1.41 读取播放器配置指定的本地 KRC。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let Some(cache_path) = cache_path.filter(|path| path.is_dir()) else {
        return Ok(None);
    };
    let entries = LOCAL_KRC_INDEX.load(cache_path, || scan_local_krc_files(cache_path))?;
    // 索引可能有上千条目，当前歌曲的归一化只做一次。
    let match_key = TrackMatchKey::new(track);
    let mut best: Option<(u8, PathBuf)> = None;
    for entry in entries.iter() {
        let artists = split_artists(&entry.artist);
        let Some(score) = match_key.score(SongCandidate {
            title: &entry.title,
            artists: &artists,
            duration_ms: None,
        }) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, entry.path.clone()));
        }
    }
    let Some((_, path)) = best else {
        return Ok(None);
    };
    let metadata = fs::metadata(&path)?;
    if metadata.len() == 0 || metadata.len() > MAX_KRC_BYTES {
        return Err(LyricsError::InvalidData("酷狗 KRC 文件大小无效".to_owned()));
    }
    let content = fs::read(path)?;
    if !content.starts_with(b"krc1") {
        return Err(LyricsError::InvalidData("酷狗 KRC 魔数无效".to_owned()));
    }
    let text = lyrics_crypto::decrypter::krc::decrypter::decrypt_lyrics_from_file(&content)
        .ok_or_else(|| LyricsError::InvalidData("酷狗 KRC 解密失败".to_owned()))?;
    let lines = parse_krc_lines(&text)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::KugouMusic,
            kind: LyricsSourceKind::Local,
            song_id: None,
        },
        lines,
    }))
}

/// 匿名搜索酷狗曲目并读取在线逐字 KRC 歌词。
pub fn resolve_online(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    online::resolve(track, client, deadline)
}

/// 扫描一次酷狗 KRC 文件并缓存稳定的文件名元数据。
fn scan_local_krc_files(cache_path: &Path) -> std::io::Result<Vec<IndexedKrcFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(cache_path)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some((artist, title)) = parse_file_name(file_name) else {
            continue;
        };
        files.push(IndexedKrcFile {
            path: entry.path(),
            artist,
            title,
        });
    }
    Ok(files)
}

/// 判断酷狗文件事件中的 KRC 文件名是否与当前歌曲匹配。
pub(super) fn changed_paths_affect_track(track: &TrackDescriptor, paths: &[PathBuf]) -> bool {
    paths.iter().any(|path| {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let Some((artist, title)) = parse_file_name(name) else {
            return false;
        };
        accepted_score(
            track,
            SongCandidate {
                title: &title,
                artists: &split_artists(&artist),
                duration_ms: None,
            },
        )
        .is_some()
    })
}

/// 文件监听确认酷狗歌词目录变化后，淘汰可能早于目录时间戳更新的索引。
pub(super) fn invalidate_local_index(cache_path: &Path) {
    LOCAL_KRC_INDEX.invalidate(cache_path);
}

/// 从酷狗 UTF-16LE 配置中的 LyricPath 定位歌词目录。
pub fn automatic_cache_path() -> Option<PathBuf> {
    let app_data = std::env::var_os("APPDATA")?;
    let ini_path = PathBuf::from(app_data).join("KuGou8").join("KuGou.ini");
    let bytes = fs::read(ini_path).ok()?;
    let words = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let content = String::from_utf16_lossy(&words);
    let mut in_lyric_section = false;
    for line in content.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_lyric_section = line.eq_ignore_ascii_case("[LyricConfigSection]");
            continue;
        }
        if in_lyric_section
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("LyricPath")
            && !value.trim().is_empty()
        {
            return Some(PathBuf::from(value.trim()));
        }
    }
    None
}

/// 返回包含酷狗 INI 的目录，由服务层以文件系统事件监听路径调整。
pub fn configuration_watch_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|root| PathBuf::from(root).join("KuGou8"))
}

/// 从酷狗缓存文件名中提取歌手和歌名，不让返回值借用临时替换结果。
fn parse_file_name(file_name: &str) -> Option<(String, String)> {
    let stem = file_name.strip_suffix(".krc")?;
    let regex = CACHE_FILE_SUFFIX.as_ref().ok()?;
    let without_ids = regex.replace(stem, "");
    let (artist, title) = without_ids.split_once(" - ")?;
    Some((artist.to_owned(), title.to_owned()))
}
