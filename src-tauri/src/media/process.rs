//! 集中维护桌面播放器进程识别，供图标与应用音量共同复用。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HMODULE},
    System::{
        ProcessStatus::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL},
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
};

/// 单次模块枚举的最大数量，超出部分不影响低基数插件判定。
const MAX_PROCESS_MODULES: usize = 1024;

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
    let candidates: Vec<Pid> = process_ids.iter().copied().map(Pid::from_u32).collect();
    // 已知目标 PID 时只读取这些进程的路径与父 PID，避免为整张进程表查询可执行文件。
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
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

/// 判断目标进程中是否加载了路径包含任意指定片段的模块。
///
/// 读取模块列表失败时按未加载处理，由调用方选择保守的后续路径。
pub(super) fn loads_module_matching(process_ids: &HashSet<u32>, fragments: &[&str]) -> bool {
    if fragments.is_empty() {
        return false;
    }

    process_ids
        .iter()
        .any(|process_id| process_loads_matching_module(*process_id, fragments))
}

/// 枚举单个进程的模块路径并匹配路径片段。
fn process_loads_matching_module(process_id: u32, fragments: &[&str]) -> bool {
    // SAFETY: 只申请读取模块列表所需的权限，不修改目标进程。
    let Ok(process) = (unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            process_id,
        )
    }) else {
        return false;
    };

    let matched = any_module_matches(process, fragments);
    // SAFETY: process 由 OpenProcess 成功返回，这里关闭唯一句柄。
    let _ = unsafe { CloseHandle(process) };
    matched
}

/// 读取模块完整路径；插件常把文件改名放进自己的目录，因此按路径片段匹配。
fn any_module_matches(process: HANDLE, fragments: &[&str]) -> bool {
    let mut modules = [HMODULE::default(); MAX_PROCESS_MODULES];
    let mut needed = 0_u32;
    // SAFETY: modules 是可写数组，缓冲区字节数与输出长度地址均有效。
    let Ok(()) = (unsafe {
        EnumProcessModulesEx(
            process,
            modules.as_mut_ptr(),
            std::mem::size_of_val(&modules) as u32,
            &raw mut needed,
            LIST_MODULES_ALL,
        )
    }) else {
        return false;
    };

    let count = (needed as usize / std::mem::size_of::<HMODULE>()).min(MAX_PROCESS_MODULES);
    (0..count).any(|index| module_matches(process, modules[index], fragments))
}

/// 读取单个模块的完整路径并匹配任意片段。
fn module_matches(process: HANDLE, module: HMODULE, fragments: &[&str]) -> bool {
    let mut buffer = [0_u16; 512];
    // SAFETY: buffer 的完整长度均可写，process 与 module 来自本次模块枚举。
    let length = unsafe { GetModuleFileNameExW(Some(process), Some(module), &mut buffer) };
    let Some(path) = buffer.get(..length as usize) else {
        return false;
    };

    let path = String::from_utf16_lossy(path).to_ascii_lowercase();
    fragments.iter().any(|fragment| path.contains(fragment))
}
