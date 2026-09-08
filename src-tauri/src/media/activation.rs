//! 通过 Windows 顶层窗口激活当前播放器，不在播放器适配器中混入窗口管理逻辑。

use std::{collections::HashSet, process::Command};

use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindowAsync,
        WS_EX_TOOLWINDOW,
    },
};
use windows_core::BOOL;

use super::process::{find_process_executable, find_process_ids};

/// 显示当前播放器主窗口；隐藏到托盘时由播放器自身的单实例入口负责恢复。
pub(super) fn activate_player(
    source_app_id: &str,
    executable_names: &[&str],
) -> Result<(), String> {
    let process_ids = find_process_ids(source_app_id, executable_names);
    if process_ids.is_empty() {
        return Err("未找到当前播放器进程".to_owned());
    }

    if let Some(window) = find_main_window(&process_ids)? {
        show_and_activate_window(window);
        return Ok(());
    }

    let executable = find_process_executable(&process_ids)
        .ok_or_else(|| "当前播放器已隐藏，但无法读取其启动路径".to_owned())?;
    let mut command = Command::new(&executable);
    if let Some(directory) = executable.parent() {
        command.current_dir(directory);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("通过播放器自身入口打开窗口失败: {error}"))
}

/// 枚举目标进程的可见顶层窗口，并用有效显示面积选出最可能的主窗口。
fn find_main_window(process_ids: &HashSet<u32>) -> Result<Option<HWND>, String> {
    let mut search = WindowSearch {
        process_ids,
        best: None,
    };

    // SAFETY: 回调只在 EnumWindows 调用期间同步使用指向 search 的有效指针。
    unsafe {
        EnumWindows(
            Some(enumerate_player_window),
            LPARAM((&raw mut search).cast::<()>() as isize),
        )
    }
    .map_err(|error| format!("枚举播放器窗口失败: {error}"))?;

    Ok(search.best.map(|candidate| candidate.window))
}

/// 恢复最小化窗口并请求 Windows 将其带到前台。
fn show_and_activate_window(window: HWND) {
    // SAFETY: window 来自本次 EnumWindows 枚举，在调用期间仍是有效的顶层窗口句柄。
    unsafe {
        let command = if IsIconic(window).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindowAsync(window, command);
        if !SetForegroundWindow(window).as_bool() {
            // Windows 可能依据前台锁策略拒绝抢焦点；窗口仍已被显示，不采用输入模拟绕过系统限制。
            log::warn!("Windows 拒绝将播放器窗口切换到前台");
        }
    }
}

struct WindowCandidate {
    window: HWND,
    area: i64,
}

struct WindowSearch<'a> {
    process_ids: &'a HashSet<u32>,
    best: Option<WindowCandidate>,
}

/// EnumWindows 回调：过滤工具窗口与无面积窗口，仅保留目标播放器窗口。
unsafe extern "system" fn enumerate_player_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 由 find_main_window 传入，并在整个同步枚举期间保持有效。
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch<'_>) };
    let mut process_id = 0;
    // SAFETY: window 由 Windows 枚举提供，process_id 是有效的可写地址。
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut process_id)) };
    if !search.process_ids.contains(&process_id) {
        return BOOL::from(true);
    }

    // SAFETY: window 由 Windows 枚举提供，读取其可见性和扩展样式不会转移所有权。
    let is_eligible = unsafe {
        IsWindowVisible(window).as_bool()
            && (GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) == 0
    };
    if !is_eligible {
        return BOOL::from(true);
    }

    let mut bounds = RECT::default();
    // SAFETY: bounds 是有效的可写地址，window 仍来自当前枚举回调。
    if unsafe { GetWindowRect(window, &raw mut bounds) }.is_err() {
        return BOOL::from(true);
    }
    let width = i64::from((bounds.right - bounds.left).max(0));
    let height = i64::from((bounds.bottom - bounds.top).max(0));
    let area = width * height;
    if area > 0 && search.best.as_ref().is_none_or(|best| area > best.area) {
        search.best = Some(WindowCandidate { window, area });
    }

    BOOL::from(true)
}
