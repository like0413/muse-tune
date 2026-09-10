use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::{
    error::LyricsError,
    model::{LyricsPrecision, LyricsSnapshot, LyricsStatus},
};

const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;
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
            Ok(()) => Ok(()),
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

    /// 播放器自身目录配置变化后删除解析结果，确保下一次使用新来源重新解析。
    pub fn clear(&self) -> Result<(), std::io::Error> {
        for entry in fs::read_dir(&self.entries_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    fn entry_path(&self, track_key: &str) -> PathBuf {
        self.entries_path.join(format!("{track_key}.json"))
    }
}

fn is_fresh(entry: &CacheEntry) -> bool {
    if entry.snapshot.precision == Some(LyricsPrecision::Word) {
        return true;
    }
    let age = now_seconds().saturating_sub(entry.refreshed_at_seconds);
    let interval = match entry.snapshot.status {
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
