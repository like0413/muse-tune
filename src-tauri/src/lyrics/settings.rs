use std::{collections::HashMap, path::PathBuf, sync::RwLock};

use tauri::Runtime;
use tauri_plugin_store::StoreExt;

use crate::media::MediaPlayer;

use super::{model::LyricsCachePathState, players};

const SETTINGS_STORE_PATH: &str = "settings.json";
const LYRICS_DISPLAY_KEY: &str = "taskbar.lyrics";
const SUPPORTED_PLAYERS: [MediaPlayer; 4] = [
    MediaPlayer::QqMusic,
    MediaPlayer::NeteaseCloudMusic,
    MediaPlayer::SodaMusic,
    MediaPlayer::KugouMusic,
];

/// 线程安全保存四家播放器的用户目录覆盖。
pub struct LyricsPathSettings {
    overrides: RwLock<HashMap<MediaPlayer, PathBuf>>,
}

/// 从版本化前端设置中恢复歌词总开关，损坏数据保持默认开启。
pub fn restore_lyrics_enabled<R: Runtime>(app: &tauri::App<R>) -> bool {
    app.store(SETTINGS_STORE_PATH)
        .ok()
        .and_then(|store| store.get(LYRICS_DISPLAY_KEY))
        .and_then(|stored| stored.get("value").cloned())
        .and_then(|value| value.get("enabled").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
}

impl LyricsPathSettings {
    /// 从官方 Store 恢复有效字符串值；目录是否存在由状态接口实时报告。
    pub fn restore<R: Runtime>(app: &tauri::App<R>) -> Self {
        let mut overrides = HashMap::new();
        if let Ok(store) = app.store(SETTINGS_STORE_PATH) {
            for player in SUPPORTED_PLAYERS {
                let Some(key) = override_store_key(player) else {
                    continue;
                };
                if let Some(path) = store
                    .get(key)
                    .and_then(|value| value.as_str().map(PathBuf::from))
                {
                    overrides.insert(player, path);
                }
            }
        }
        Self {
            overrides: RwLock::new(overrides),
        }
    }

    /// 返回当前播放器实际使用的目录。
    pub fn effective_path(&self, player: MediaPlayer) -> Option<PathBuf> {
        self.override_path(player)
            .or_else(|| players::automatic_cache_path(player))
    }

    /// 更新内存覆盖值；持久化由 command 边界负责。
    pub fn set_override(&self, player: MediaPlayer, path: Option<PathBuf>) -> Result<(), String> {
        let mut overrides = self
            .overrides
            .write()
            .map_err(|_| "歌词目录设置不可用".to_owned())?;
        if let Some(path) = path {
            overrides.insert(player, path);
        } else {
            overrides.remove(&player);
        }
        Ok(())
    }

    /// 生成设置页所需的四个平台状态。
    pub fn states(&self) -> Vec<LyricsCachePathState> {
        SUPPORTED_PLAYERS
            .into_iter()
            .map(|player| {
                let automatic = players::automatic_cache_path(player);
                let override_path = self.override_path(player);
                let effective = override_path.clone().or_else(|| automatic.clone());
                LyricsCachePathState {
                    player,
                    automatic_path: path_string(automatic),
                    override_path: path_string(override_path),
                    exists: effective.as_ref().is_some_and(|path| path.is_dir()),
                    effective_path: path_string(effective),
                }
            })
            .collect()
    }

    fn override_path(&self, player: MediaPlayer) -> Option<PathBuf> {
        self.overrides
            .read()
            .ok()
            .and_then(|overrides| overrides.get(&player).cloned())
    }
}

/// 返回单个平台稳定的 Store 键。
pub fn override_store_key(player: MediaPlayer) -> Option<&'static str> {
    match player {
        MediaPlayer::QqMusic => Some("lyrics.cachePathOverrides.qqMusic"),
        MediaPlayer::NeteaseCloudMusic => Some("lyrics.cachePathOverrides.neteaseCloudMusic"),
        MediaPlayer::SodaMusic => Some("lyrics.cachePathOverrides.sodaMusic"),
        MediaPlayer::KugouMusic => Some("lyrics.cachePathOverrides.kugouMusic"),
        MediaPlayer::Other => None,
    }
}

fn path_string(path: Option<PathBuf>) -> Option<String> {
    path.map(|path| path.to_string_lossy().into_owned())
}
