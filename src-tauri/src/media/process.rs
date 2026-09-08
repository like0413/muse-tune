//! 集中维护桌面播放器进程识别，供图标与应用音量共同复用。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// 找出来源标识或播放器适配器所对应的全部进程，避免遗漏多进程音频会话。
pub(super) fn find_process_ids(source_app_id: &str, executable_names: &[&str]) -> HashSet<u32> {
    let source_name = Path::new(source_app_id)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_ascii_lowercase);
    let normalized_executables = executable_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let process_name = process.name().to_string_lossy().to_ascii_lowercase();
            let process_stem = process_name.strip_suffix(".exe").unwrap_or(&process_name);
            let matches_source = source_name.as_ref().is_some_and(|name| {
                name == &process_name || name.strip_suffix(".exe") == Some(process_stem)
            });
            let matches_adapter = normalized_executables.iter().any(|name| {
                name == &process_name || name.strip_suffix(".exe") == Some(process_stem)
            });
            (matches_source || matches_adapter).then(|| pid.as_u32())
        })
        .collect()
}

/// 从已识别进程中读取一个仍在运行的可执行文件路径，供隐藏到托盘时交还播放器自身唤起。
pub(super) fn find_process_executable(process_ids: &HashSet<u32>) -> Option<PathBuf> {
    if process_ids.is_empty() {
        return None;
    }

    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    process_ids.iter().find_map(|process_id| {
        system
            .process(Pid::from_u32(*process_id))
            .and_then(|process| process.exe())
            .map(Path::to_path_buf)
    })
}
