//! 酷狗音乐歌词适配器门面；本地 KRC 与在线接口分别实现。

mod local;
mod online;

pub(super) use local::{
    automatic_cache_path, changed_paths_affect_track, configuration_watch_path,
    invalidate_local_index, resolve,
};
pub(super) use online::resolve as resolve_online;
