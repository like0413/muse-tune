use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::media::MediaPlayer;

use super::{
    error::LyricsError,
    model::{LyricsPrecision, LyricsSnapshot, LyricsSourceKind, LyricsStatus},
};

const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CACHE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const WORD_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const LINE_REFRESH_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const UNAVAILABLE_REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// 缓存命中及其是否仍处于免联网刷新期。
pub struct CacheLookup {
    pub snapshot: LyricsSnapshot,
    pub is_fresh: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheEntry {
    refreshed_at_seconds: u64,
    snapshot: LyricsSnapshot,
}

/// 版本化的解析后歌词文件缓存。
pub struct ParsedLyricsCache {
    entries_path: PathBuf,
}

impl ParsedLyricsCache {
    /// 在应用缓存目录下创建歌词专用版本目录。
    pub fn new(app_cache_dir: &Path) -> Result<Self, std::io::Error> {
        let entries_path = app_cache_dir.join("lyrics").join("v3").join("entries");
        fs::create_dir_all(&entries_path)?;
        Ok(Self { entries_path })
    }

    /// 读取并校验单个缓存条目，损坏条目按未命中处理。
    pub fn load(&self, track_key: &str) -> Option<CacheLookup> {
        let path = self.entry_path(track_key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > MAX_CACHE_ENTRY_BYTES {
            let _ = fs::remove_file(path);
            return None;
        }
        let content = fs::read(&path).ok()?;
        let entry = serde_json::from_slice::<CacheEntry>(&content).ok();
        if let Some(entry) = entry.filter(|entry| {
            entry.snapshot.track_key.as_deref() == Some(track_key)
                && matches!(
                    entry.snapshot.status,
                    LyricsStatus::Ready | LyricsStatus::Unavailable
                )
        }) {
            return Some(CacheLookup {
                is_fresh: is_fresh(&entry),
                snapshot: entry.snapshot,
            });
        }
        let _ = fs::remove_file(path);
        None
    }

    /// 写入本次解析结果和刷新时间；损坏缓存可在下次播放时自动重建。
    pub fn store(&self, snapshot: &LyricsSnapshot) -> Result<(), LyricsError> {
        let Some(track_key) = snapshot.track_key.as_deref() else {
            return Ok(());
        };
        if !matches!(
            snapshot.status,
            LyricsStatus::Ready | LyricsStatus::Unavailable
        ) {
            return Ok(());
        }
        let target = self.entry_path(track_key);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary =
            self.entries_path
                .join(format!(".{track_key}.{}.{}.tmp", std::process::id(), nonce));
        let content = serde_json::to_vec(&CacheEntry {
            refreshed_at_seconds: now_seconds(),
            snapshot: snapshot.clone(),
        })?;
        if content.len() as u64 > MAX_CACHE_ENTRY_BYTES {
            return Err(LyricsError::InvalidData(
                "规范化歌词超过缓存大小上限".to_owned(),
            ));
        }
        fs::write(&temporary, content)?;
        if target.exists()
            && let Err(error) = fs::remove_file(&target)
        {
            let _ = fs::remove_file(temporary);
            return Err(error.into());
        }
        match fs::rename(&temporary, &target) {
            Ok(()) => {
                self.prune_to_size_limit(&target)?;
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(temporary);
                Err(error.into())
            }
        }
    }

    /// 删除当前歌曲的解析结果，使播放器源文件变化后只重建受影响条目。
    pub fn remove(&self, track_key: &str) -> Result<(), std::io::Error> {
        match fs::remove_file(self.entry_path(track_key)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// 只删除依赖指定播放器本地目录的结果，在线结果与其他播放器缓存继续保留。
    pub fn clear_local_source(&self, player: MediaPlayer) -> Result<(), std::io::Error> {
        for entry in fs::read_dir(&self.entries_path)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let should_remove = fs::read(&path)
                .ok()
                .and_then(|content| serde_json::from_slice::<CacheEntry>(&content).ok())
                .is_some_and(|entry| {
                    entry.snapshot.source.is_some_and(|source| {
                        source.player == player && source.kind == LyricsSourceKind::Local
                    })
                });
            if should_remove {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    fn entry_path(&self, track_key: &str) -> PathBuf {
        self.entries_path.join(format!("{track_key}.json"))
    }

    /// 按最近写入时间淘汰旧条目，使永久运行也不会无限占用磁盘。
    fn prune_to_size_limit(&self, protected_path: &Path) -> Result<(), std::io::Error> {
        let mut total_bytes = 0_u64;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.entries_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            total_bytes = total_bytes.saturating_add(metadata.len());
            entries.push((
                metadata.modified().unwrap_or(UNIX_EPOCH),
                metadata.len(),
                entry.path(),
            ));
        }
        if total_bytes <= MAX_CACHE_TOTAL_BYTES {
            return Ok(());
        }
        entries.sort_unstable_by_key(|(modified, _, _)| *modified);
        for (_, size, path) in entries {
            if total_bytes <= MAX_CACHE_TOTAL_BYTES {
                break;
            }
            if path == protected_path {
                continue;
            }
            fs::remove_file(path)?;
            total_bytes = total_bytes.saturating_sub(size);
        }
        Ok(())
    }
}

fn is_fresh(entry: &CacheEntry) -> bool {
    is_fresh_at(entry, now_seconds())
}

fn is_fresh_at(entry: &CacheEntry, current_seconds: u64) -> bool {
    let Some(age) = current_seconds.checked_sub(entry.refreshed_at_seconds) else {
        return false;
    };
    let interval = match entry.snapshot.status {
        LyricsStatus::Ready if entry.snapshot.precision == Some(LyricsPrecision::Word) => {
            WORD_REFRESH_INTERVAL
        }
        LyricsStatus::Ready => LINE_REFRESH_INTERVAL,
        LyricsStatus::Unavailable => UNAVAILABLE_REFRESH_INTERVAL,
        LyricsStatus::Loading | LyricsStatus::Error => return false,
    };
    age < interval.as_secs()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        CacheEntry, LINE_REFRESH_INTERVAL, LyricsPrecision, LyricsSnapshot, LyricsStatus,
        WORD_REFRESH_INTERVAL, is_fresh_at,
    };

    fn entry(status: LyricsStatus, precision: Option<LyricsPrecision>) -> CacheEntry {
        CacheEntry {
            refreshed_at_seconds: 100,
            snapshot: LyricsSnapshot {
                status,
                precision,
                ..LyricsSnapshot::default()
            },
        }
    }

    #[test]
    fn word_cache_expires_after_long_refresh_interval() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Word));

        assert!(!is_fresh_at(&entry, 100 + WORD_REFRESH_INTERVAL.as_secs()));
    }

    #[test]
    fn line_cache_expires_before_word_cache() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Line));

        assert!(!is_fresh_at(&entry, 100 + LINE_REFRESH_INTERVAL.as_secs()));
    }

    #[test]
    fn backward_clock_change_forces_safe_refresh() {
        let entry = entry(LyricsStatus::Ready, Some(LyricsPrecision::Line));

        assert!(!is_fresh_at(&entry, 50));
    }
}
