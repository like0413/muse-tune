//! Spotify 歌词适配器门面；本地能力再按 HTTP 缓存与播放状态细分。

mod cache;
mod local;
mod state;

pub(super) use local::{
    additional_watch_paths, automatic_cache_path, changed_paths_affect_track, resolve,
};
