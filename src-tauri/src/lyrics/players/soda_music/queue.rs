use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use serde::Deserialize;

use crate::lyrics::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    track::TrackDescriptor,
};

const MAX_QUEUE_BYTES: u64 = 16 * 1024 * 1024;

/// 判断汽水队列缓存里是否包含当前歌曲。
///
/// 汽水没有本地歌词，QueueCache 只是定位歌曲 ID 的入口；文件监听必须靠它判断事件是否
/// 与当前歌曲相关，否则队列刷新、预加载都会触发一次清缓存与完整重解析。
pub(in crate::lyrics::players) fn queue_contains_track(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> bool {
    // 队列损坏或超限时按“不相关”处理：宁可漏一次刷新，也不要无谓地作废当前结果。
    matches!(find_song(track, cache_path), Ok(Some(_)))
}

/// 从 QueueCache 中选出与当前媒体会话最吻合的歌曲。
pub(super) fn find_song(
    track: &TrackDescriptor,
    cache_path: &Path,
) -> Result<Option<Track>, LyricsError> {
    let queue_path = queue_cache_path(cache_path);
    let metadata = match fs::metadata(&queue_path) {
        Ok(metadata) if metadata.len() <= MAX_QUEUE_BYTES => metadata,
        Ok(_) => {
            return Err(LyricsError::InvalidData(
                "汽水队列缓存超过大小上限".to_owned(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() < 6 {
        return Ok(None);
    }
    let bytes = fs::read(queue_path)?;
    let gzip_offset = bytes
        .windows(2)
        .position(|window| window == [0x1f, 0x8b])
        .ok_or_else(|| LyricsError::InvalidData("汽水 QueueCache 缺少 gzip 数据".to_owned()))?;
    let decoder = GzDecoder::new(&bytes[gzip_offset..]);
    let mut json = String::new();
    decoder
        .take(MAX_QUEUE_BYTES + 1)
        .read_to_string(&mut json)?;
    if json.len() as u64 > MAX_QUEUE_BYTES {
        return Err(LyricsError::InvalidData(
            "汽水队列解压结果超过大小上限".to_owned(),
        ));
    }
    let feeds = serde_json::from_str::<HashMap<String, Feed>>(&json)?;
    Ok(feeds
        .into_values()
        .flat_map(|feed| feed.playables)
        .filter_map(|playable| playable.track)
        .filter_map(|song| {
            let artists = song
                .artists
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>();
            let score = song
                .matching_durations()
                .into_iter()
                .flatten()
                .filter_map(|duration_ms| {
                    accepted_score(
                        track,
                        SongCandidate {
                            title: &song.name,
                            artists: &artists,
                            duration_ms: Some(duration_ms),
                        },
                    )
                })
                .max()?;
            Some((score, song))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, song)| song))
}

/// 兼容用户直接配置 LunaStorage 或它的上级目录。
fn queue_cache_path(cache_path: &Path) -> PathBuf {
    if cache_path.join("QueueCache").is_file() {
        cache_path.join("QueueCache")
    } else {
        cache_path.join("LunaStorage").join("QueueCache")
    }
}

#[derive(Deserialize)]
struct Feed {
    #[serde(default)]
    playables: Vec<Playable>,
}

#[derive(Deserialize)]
struct Playable {
    track: Option<Track>,
}

#[derive(Deserialize)]
pub(super) struct Track {
    pub(super) id: String,
    name: String,
    pub(super) duration: u64,
    #[serde(default)]
    artists: Vec<Artist>,
    #[serde(default)]
    preview: Option<Preview>,
    #[serde(default)]
    audition_info: Option<AuditionInfo>,
}

impl Track {
    /// 返回接口中可能参与媒体会话匹配的全部时长。
    fn matching_durations(&self) -> [Option<u64>; 3] {
        [
            Some(self.duration),
            self.audition_info
                .as_ref()
                .and_then(|value| value.duration_ms),
            self.preview.as_ref().and_then(|value| value.duration),
        ]
    }

    /// 返回接口声明的试听时长。
    pub(super) fn audition_duration_ms(&self) -> Option<u64> {
        self.audition_info
            .as_ref()
            .and_then(|value| value.duration_ms)
            .or_else(|| self.preview.as_ref().and_then(|value| value.duration))
            .filter(|duration_ms| *duration_ms > 0)
    }
}

#[derive(Deserialize)]
struct Preview {
    duration: Option<u64>,
}

#[derive(Deserialize)]
struct AuditionInfo {
    duration_ms: Option<u64>,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}
