use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
    thread,
};

use base64::Engine;
use reqwest::{
    blocking::Client,
    header::{REFERER, USER_AGENT},
};
use serde::Deserialize;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, REG_NOTIFY_CHANGE_LAST_SET, RRF_RT_REG_SZ, RegCloseKey,
    RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};

use crate::media::MediaPlayer;

use super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics, has_word_timing},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_qrc_lines},
    track::{TrackDescriptor, split_artists},
};
use super::file_index::DirectoryFileIndex;

mod online;

const MAX_QRC_BYTES: u64 = 2 * 1024 * 1024;
static LOCAL_QRC_INDEX: LazyLock<DirectoryFileIndex<IndexedQrcFile>> =
    LazyLock::new(DirectoryFileIndex::new);

#[derive(Clone)]
struct IndexedQrcFile {
    path: PathBuf,
    artist: String,
    title: String,
    duration_seconds: u64,
}

/// QQ 音乐本地缓存优先，未命中后使用官方域名下的网页内部 HTTPS 接口。
pub fn resolve(
    track: &TrackDescriptor,
    cache_path: Option<&Path>,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let local = cache_path.and_then(|path| match resolve_local(track, path) {
        Ok(resolved) => resolved,
        Err(error) => {
            log::warn!("QQ 音乐本地歌词不可用，回退在线源: {error}");
            None
        }
    });
    if local
        .as_ref()
        .is_some_and(|resolved| has_word_timing(&resolved.lines))
    {
        return Ok(local);
    }
    match resolve_online(track, client, deadline) {
        Ok(Some(online)) if has_word_timing(&online.lines) => Ok(Some(online)),
        Ok(online) => Ok(local.or(online)),
        Err(error) if local.is_some() => {
            log::warn!("QQ 音乐在线逐字升级失败，保留本地逐行歌词: {error}");
            Ok(local)
        }
        Err(error) => Err(error),
    }
}

/// 从注册表的缓存根目录定位 QQMusicLyricNew。
pub fn automatic_cache_path() -> Option<PathBuf> {
    read_cache_root_from_registry().map(|path| path.join("QQMusicLyricNew"))
}

/// 使用 Windows 注册表原生通知监听 QQ 音乐缓存根目录调整。
pub fn watch_cache_path_changes(on_change: Arc<dyn Fn() + Send + Sync>) -> Result<(), io::Error> {
    thread::Builder::new()
        .name("qq-music-cache-registry".to_owned())
        .spawn(move || {
            let mut key = HKEY::default();
            // SAFETY: 只读打开当前用户的固定 QQ 音乐配置键，并仅申请通知权限。
            let result = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    windows::core::w!(r"Software\Tencent\QQMusic\LogConfig"),
                    None,
                    KEY_NOTIFY,
                    &mut key,
                )
            };
            if result.is_err() {
                log::debug!("QQ 音乐注册表配置不存在，跳过键值监听");
                return;
            }
            loop {
                // SAFETY: key 在线程结束前保持有效；同步等待不需要事件句柄。
                let result = unsafe {
                    RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false)
                };
                if result.is_err() {
                    break;
                }
                on_change();
            }
            // SAFETY: key 只由本线程持有且仅关闭一次。
            let _ = unsafe { RegCloseKey(key) };
        })?;
    Ok(())
}

/// 匿名搜索 QQ 曲目并读取行歌词。
pub fn resolve_online(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let query = format!("{} {}", track.title, track.artists.join(" "));
    let response = parse_json::<QqSearchResponse>(
        deadline
            .apply(
                client
                    .get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://y.qq.com/")
                    .query(&[("format", "json"), ("p", "1"), ("n", "10"), ("w", &query)]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(song) = response
        .data
        .song
        .list
        .into_iter()
        .filter_map(|song| {
            let artists = song
                .singer
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>();
            let score = accepted_score(
                track,
                SongCandidate {
                    title: &song.song_name,
                    artists: &artists,
                    duration_ms: Some(u64::from(song.interval) * 1_000),
                },
            )?;
            Some((score, song))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, song)| song)
    else {
        return Ok(None);
    };

    // QQ 的新版匿名接口可返回加密 QRC。优先尝试真实逐字数据；接口缺失、
    // 格式变化或曲目本身没有 QRC 时，继续走下方已验证的行级 LRC 兜底。
    match online::fetch_word_lyrics(client, &song.song_mid, deadline) {
        Ok(Some(lines)) => {
            return Ok(Some(ResolvedLyrics {
                source: LyricsSource {
                    player: MediaPlayer::QqMusic,
                    kind: LyricsSourceKind::Online,
                    song_id: Some(song.song_mid),
                },
                lines,
            }));
        }
        Ok(None) => {}
        Err(error) => log::debug!("QQ 音乐在线 QRC 不可用，回退行级歌词: {error}"),
    }

    let response = parse_json::<QqLyricsResponse>(
        deadline
            .apply(
                client
                    .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://y.qq.com/")
                    .query(&[
                        ("songmid", song.song_mid.as_str()),
                        ("format", "json"),
                        ("nobase64", "0"),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(original) = decode_base64_text(response.lyric.as_deref()) else {
        return Ok(None);
    };
    let mut lines = parse_lrc_lines(&original)?;
    if let Some(translation) = decode_base64_text(response.trans.as_deref()) {
        let translation = parse_lrc_lines(&translation)?;
        merge_auxiliary_lines(&mut lines, &translation, AuxiliaryKind::Translation);
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::QqMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song.song_mid),
        },
        lines,
    }))
}

pub(super) fn resolve_local(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    if !cache_path.is_dir() {
        return Ok(None);
    }
    let entries = LOCAL_QRC_INDEX.load(cache_path, || scan_local_qrc_files(cache_path))?;
    let mut best: Option<(u8, PathBuf)> = None;
    for entry in entries {
        let artists = split_artists(&entry.artist);
        let Some(score) = accepted_score(
            track,
            SongCandidate {
                title: &entry.title,
                artists: &artists,
                duration_ms: Some(entry.duration_seconds * 1_000),
            },
        ) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best = Some((score, entry.path));
        }
    }
    let Some((_, original_path)) = best else {
        return Ok(None);
    };
    let mut lines = decrypt_local_qrc(&original_path, LocalQrcKind::Primary)?;
    let base = original_path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("_qm.qrc"))
        .ok_or_else(|| LyricsError::InvalidData("QQ QRC 文件名无效".to_owned()))?;
    for (suffix, kind) in [
        ("_qmts.qrc", AuxiliaryKind::Translation),
        ("_qmRoma.qrc", AuxiliaryKind::Romanization),
    ] {
        let auxiliary_path = original_path.with_file_name(format!("{base}{suffix}"));
        if auxiliary_path.is_file()
            && let Ok(auxiliary) = decrypt_local_qrc(&auxiliary_path, LocalQrcKind::Auxiliary)
        {
            merge_auxiliary_lines(&mut lines, &auxiliary, kind);
        }
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::QqMusic,
            kind: LyricsSourceKind::Local,
            song_id: None,
        },
        lines,
    }))
}

/// 扫描一次 QQ 主 QRC 文件并缓存稳定的文件名元数据。
fn scan_local_qrc_files(cache_path: &Path) -> io::Result<Vec<IndexedQrcFile>> {
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
        let Some(metadata) = parse_original_file_name(file_name) else {
            continue;
        };
        files.push(IndexedQrcFile {
            path: entry.path(),
            artist: metadata.artist.to_owned(),
            title: metadata.title.to_owned(),
            duration_seconds: metadata.duration_seconds,
        });
    }
    Ok(files)
}

/// 判断一组 QQ 缓存文件事件中是否包含当前歌曲的主歌词或辅助歌词。
pub(super) fn changed_paths_affect_track(track: &TrackDescriptor, paths: &[PathBuf]) -> bool {
    paths.iter().any(|path| {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let primary_name = name
            .strip_suffix("_qmts.qrc")
            .or_else(|| name.strip_suffix("_qmRoma.qrc"))
            .map_or_else(|| name.to_owned(), |base| format!("{base}_qm.qrc"));
        let Some(metadata) = parse_original_file_name(&primary_name) else {
            return false;
        };
        let artists = split_artists(metadata.artist);
        accepted_score(
            track,
            SongCandidate {
                title: metadata.title,
                artists: &artists,
                duration_ms: Some(metadata.duration_seconds * 1_000),
            },
        )
        .is_some()
    })
}

#[derive(Clone, Copy)]
enum LocalQrcKind {
    Primary,
    Auxiliary,
}

/// 解密 QQ 本地 QRC，并按主歌词与辅助歌词各自的真实格式解析。
fn decrypt_local_qrc(
    path: &Path,
    kind: LocalQrcKind,
) -> Result<Vec<super::super::model::LyricLine>, LyricsError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_QRC_BYTES {
        return Err(LyricsError::InvalidData("QQ QRC 文件大小无效".to_owned()));
    }
    let decoded = decode_qmc_mask(&fs::read(path)?);
    let newline = decoded
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| LyricsError::InvalidData("QQ QRC 缺少偏移头".to_owned()))?;
    let header = String::from_utf8_lossy(&decoded[..newline]);
    if !header.trim().starts_with("[offset:") || !header.trim().ends_with(']') {
        return Err(LyricsError::InvalidData("QQ QRC 偏移头无效".to_owned()));
    }
    let encrypted = hex::encode(&decoded[newline + 1..]);
    let text = lyrics_crypto::decrypter::qrc::decrypter::decrypt_lyrics(&encrypted)
        .ok_or_else(|| LyricsError::InvalidData("QQ QRC 解密失败".to_owned()))?;
    match kind {
        LocalQrcKind::Primary => parse_qrc_lines(&text),
        // QQ 的 _qmts 与 _qmRoma 外层仍是 QRC 加密，但明文是行级 LRC。
        LocalQrcKind::Auxiliary => parse_lrc_lines(&text),
    }
}

fn decode_qmc_mask(bytes: &[u8]) -> Vec<u8> {
    const SEED: [[u8; 7]; 8] = [
        [0x4a, 0xd6, 0xca, 0x90, 0x67, 0xf7, 0x52],
        [0x5e, 0x95, 0x23, 0x9f, 0x13, 0x11, 0x7e],
        [0x47, 0x74, 0x3d, 0x90, 0xaa, 0x3f, 0x51],
        [0xc6, 0x09, 0xd5, 0x9f, 0xfa, 0x66, 0xf9],
        [0xf3, 0xd6, 0xa1, 0x90, 0xa0, 0xf7, 0xf0],
        [0x1d, 0x95, 0xde, 0x9f, 0x84, 0x11, 0xf4],
        [0x0e, 0x74, 0xbb, 0x90, 0xbc, 0x3f, 0x92],
        [0x00, 0x09, 0x5b, 0x9f, 0x62, 0x66, 0xa1],
    ];
    let mut x = -1_i32;
    let mut y = 8_i32;
    let mut direction = 1_i32;
    let mut mask_index = -1_i32;
    bytes
        .iter()
        .map(|byte| {
            let mask = loop {
                mask_index += 1;
                let current = if x < 0 {
                    direction = 1;
                    y = (8 - y) % 8;
                    0xc3
                } else if x > 6 {
                    direction = -1;
                    y = 7 - y;
                    0xd8
                } else {
                    SEED[y as usize][x as usize]
                };
                x += direction;
                if mask_index != 0x8000 && !(mask_index > 0x8000 && (mask_index + 1) % 0x8000 == 0)
                {
                    break current;
                }
            };
            byte ^ mask
        })
        .collect()
}

fn read_cache_root_from_registry() -> Option<PathBuf> {
    let mut byte_count = 0_u32;
    // SAFETY: 仅查询当前用户下静态 QQ 音乐键值的所需缓冲区大小。
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!(r"Software\Tencent\QQMusic\LogConfig"),
            windows::core::w!("CACHEPATH"),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut byte_count),
        )
    };
    if result.is_err() || byte_count < 2 {
        return None;
    }
    let mut buffer = vec![0_u16; byte_count as usize / 2];
    // SAFETY: 缓冲区按上一次调用返回的字节数分配，指针在调用期间有效且可写。
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            windows::core::w!(r"Software\Tencent\QQMusic\LogConfig"),
            windows::core::w!("CACHEPATH"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut byte_count),
        )
    };
    if result.is_err() {
        return None;
    }
    let length = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    let path = String::from_utf16_lossy(&buffer[..length]);
    (!path.trim().is_empty()).then(|| PathBuf::from(path))
}

struct FileNameMetadata<'a> {
    artist: &'a str,
    title: &'a str,
    duration_seconds: u64,
}

fn parse_original_file_name(file_name: &str) -> Option<FileNameMetadata<'_>> {
    let stem = file_name.strip_suffix("_qm.qrc")?;
    let mut right = stem.rsplitn(3, " - ");
    let _album = right.next()?;
    let duration_seconds = right.next()?.parse().ok()?;
    let (artist, title) = right.next()?.split_once(" - ")?;
    Some(FileNameMetadata {
        artist,
        title,
        duration_seconds,
    })
}

fn decode_base64_text(value: Option<&str>) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value?)
        .ok()?;
    String::from_utf8(bytes).ok()
}

#[derive(Deserialize)]
struct QqSearchResponse {
    data: QqSearchData,
}

#[derive(Deserialize)]
struct QqSearchData {
    song: QqSongList,
}

#[derive(Deserialize)]
struct QqSongList {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Deserialize)]
struct QqSong {
    #[serde(rename = "songmid")]
    song_mid: String,
    #[serde(rename = "songname")]
    song_name: String,
    #[serde(default)]
    singer: Vec<QqArtist>,
    interval: u32,
}

#[derive(Deserialize)]
struct QqArtist {
    name: String,
}

#[derive(Deserialize)]
struct QqLyricsResponse {
    lyric: Option<String>,
    trans: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::parse_original_file_name;

    #[test]
    fn filename_parser_keeps_dashes_inside_title() {
        let metadata = parse_original_file_name("歌手 - 标题 - 副标题 - 240 - 专辑_qm.qrc")
            .expect("当前 QQ 文件名应可解析");

        assert_eq!(metadata.title, "标题 - 副标题");
    }
}
