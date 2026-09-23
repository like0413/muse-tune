//! QQ 音乐歌词适配器门面；本地 QRC 与在线接口分别实现。

mod local;
mod online;

pub(super) use local::{
    automatic_cache_path, changed_paths_affect_track, invalidate_local_index,
    resolve as resolve_local, watch_cache_path_changes,
};
pub(super) use online::resolve as resolve_online;
