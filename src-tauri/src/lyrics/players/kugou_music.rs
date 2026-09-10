use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
    parser::parse_krc_lines,
    track::{TrackDescriptor, split_artists},
};

const MAX_KRC_BYTES: u64 = 2 * 1024 * 1024;
static CACHE_FILE_SUFFIX: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"(?i)-[0-9a-f]{32}-\d+-\d+$"));

/// 酷狗 20.1.41 读取播放器配置指定的本地 KRC。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let Some(cache_path) = cache_path.filter(|path| path.is_dir()) else {
        return Ok(None);
    };
    let mut best: Option<(u8, PathBuf)> = None;
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
        let artists = split_artists(&artist);
        let Some(score) = accepted_score(
            track,
            SongCandidate {
                title: &title,
                artists: &artists,
                duration_ms: None,
            },
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, entry.path()));
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
