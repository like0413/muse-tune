//! 集中维护桌面播放器进程识别，供图标与应用音量共同复用。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// 复用的进程枚举器：避免每次查询都重建整张进程表。
static PROCESS_SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

/// 找出来源标识或播放器适配器所对应的全部进程，避免遗漏多进程音频会话。
pub(super) fn find_process_ids(source_app_id: &str, executable_names: &[&str]) -> HashSet<u32> {
    let mut candidates = Vec::with_capacity(executable_names.len() + 1);
    let source_name = Path::new(source_app_id)
        .file_name()
        .and_then(|name| name.to_str());
    candidates.extend(
        source_name
            .into_iter()
            .chain(executable_names.iter().copied())
            .map(str::to_ascii_lowercase),
    );

    let system = PROCESS_SYSTEM.get_or_init(|| Mutex::new(System::new()));
    let Ok(mut system) = system.lock() else {
        return HashSet::new();
    };
    // 复用同一实例做增量刷新；remove_dead_processes 会清掉已退出的进程。
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            // 无效 UTF-8 时退回有损字符串；有效时借用而非分配。
            let process_name = process.name().to_string_lossy();
            name_matches(&process_name, &candidates).then(|| pid.as_u32())
        })
        .collect()
}

/// 与旧实现等价地比较进程名与候选项，但不再为每个进程分配小写字符串。
fn name_matches(process_name: &str, candidates: &[String]) -> bool {
    let process_stem = strip_exe(process_name).unwrap_or(process_name);
    candidates.iter().any(|candidate| {
        candidate.eq_ignore_ascii_case(process_name)
            || strip_exe(candidate).is_some_and(|stem| stem.eq_ignore_ascii_case(process_stem))
    })
}

/// 去掉结尾的 `.exe`（大小写不敏感）；不以 `.exe` 结尾时返回 None。
fn strip_exe(name: &str) -> Option<&str> {
    match name.as_bytes() {
        // 末尾四个字节若为 ".exe" 均为 ASCII，`head.len()` 必然是字符边界。
        [head @ .., b'.', b'e' | b'E', b'x' | b'X', b'e' | b'E'] => name.get(..head.len()),
        _ => None,
    }
}

/// 优先读取进程树根节点的可执行文件，避免把多进程客户端的渲染子进程当作启动入口。
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
    process_ids
        .iter()
        .filter_map(|process_id| {
            let process = system.process(Pid::from_u32(*process_id))?;
            let executable = process.exe()?.to_path_buf();
            let is_child = process
                .parent()
                .is_some_and(|parent| process_ids.contains(&parent.as_u32()));
            Some((is_child, executable))
        })
        .min_by_key(|(is_child, executable)| (*is_child, executable.components().count()))
        .map(|(_, executable)| executable)
}
