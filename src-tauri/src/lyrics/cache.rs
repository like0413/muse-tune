use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{error::LyricsError, model::LyricsSnapshot};

const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;

/// 版本化的解析后歌词文件缓存。
pub struct ParsedLyricsCache {
    entries_path: PathBuf,
}

impl ParsedLyricsCache {
    /// 在应用缓存目录下创建歌词专用版本目录。
    pub fn new(app_cache_dir: &Path) -> Result<Self, std::io::Error> {
        let entries_path = app_cache_dir.join("lyrics").join("v2").join("entries");
        fs::create_dir_all(&entries_path)?;
        Ok(Self { entries_path })
    }

    /// 读取并校验单个缓存条目，损坏条目按未命中处理。
    pub fn load(&self, track_key: &str) -> Option<LyricsSnapshot> {
        let path = self.entry_path(track_key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > MAX_CACHE_ENTRY_BYTES {
            return None;
        }
        let content = fs::read(&path).ok()?;
        let snapshot = serde_json::from_slice::<LyricsSnapshot>(&content).ok()?;
        (snapshot.track_key.as_deref() == Some(track_key)
            && snapshot.status == super::model::LyricsStatus::Ready)
            .then_some(snapshot)
    }

    /// 通过同目录临时文件和重命名写入新条目。
    pub fn store(&self, snapshot: &LyricsSnapshot) -> Result<(), LyricsError> {
        let Some(track_key) = snapshot.track_key.as_deref() else {
            return Ok(());
        };
        let target = self.entry_path(track_key);
        if target.exists() {
            return Ok(());
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary =
            self.entries_path
                .join(format!(".{track_key}.{}.{}.tmp", std::process::id(), nonce));
        let content = serde_json::to_vec(snapshot)?;
        if content.len() as u64 > MAX_CACHE_ENTRY_BYTES {
            return Err(LyricsError::InvalidData(
                "规范化歌词超过缓存大小上限".to_owned(),
            ));
        }
        fs::write(&temporary, content)?;
        match fs::rename(&temporary, &target) {
            Ok(()) => Ok(()),
            Err(_) if target.exists() => {
                let _ = fs::remove_file(temporary);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(temporary);
                Err(error.into())
            }
        }
    }

    /// 目录覆盖变化后删除解析结果，确保下一次使用新来源重新解析。
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
