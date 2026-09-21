//! 开关播放器主窗口：已在前台时关闭，最小化或隐藏到托盘时还原或显示。
//! 隐藏窗口优先由外部直接显示，只有窗口由 Chromium 托管的客户端才交给播放器官方入口恢复。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HMODULE, HWND, LPARAM, WPARAM},
    System::{
        ProcessStatus::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL},
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GA_ROOTOWNER, GW_OWNER, GWL_EXSTYLE, GetAncestor, GetClassNameW,
        GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowPlacement,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        PostMessageW, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindowAsync, WINDOWPLACEMENT,
        WM_CLOSE, WS_EX_TOOLWINDOW,
    },
};
use windows_core::BOOL;

use super::{
    players::IdentifiedPlayer,
    process::{find_process_executable, find_process_ids},
};
use crate::error::Error;

/// 过短的歌曲名不足以用来匹配窗口标题。
const MIN_RELATED_TITLE_LENGTH: usize = 2;
/// 官方启动入口相对客户端可执行文件向上查找的层数。
const LAUNCH_ENTRY_SEARCH_LEVELS: usize = 3;
/// 单次模块枚举的最大数量，超出部分不影响插件判定。
const MAX_PROCESS_MODULES: usize = 1024;
/// Chromium 托管窗口的类名前缀。
const CHROMIUM_WINDOW_CLASS_PREFIX: &str = "chrome_widgetwin_";

/// 开关当前播放器主窗口：窗口已在前台时关闭它（客户端通常会缩回托盘继续播放），
/// 最小化或隐藏到托盘时还原或显示。
///
/// 候选窗口必须同时满足：非工具窗口、无属主、标题非空、还原面积大于零。标题非空用于排除
/// 播放器内部承载渲染或计时的隐藏宿主窗口（例如 Chrome_WidgetWin_0、AsyncDNSWindow），
/// 它们面积往往比主窗口更大，显示它们就会出现空白或黑屏窗口。
/// 候选排序依次为：适配器声明的主窗口类名、标题包含当前播放内容、还原面积，因此
/// 最小化到托盘导致窗口不可见或缺失任务栏标志时仍能选回真正的主窗口。
pub(super) fn toggle_player_window(
    source_app_id: &str,
    player: &IdentifiedPlayer,
    media_title: &str,
) -> Result<(), Error> {
    let process_ids = find_process_ids(source_app_id, player.executable_names());
    if process_ids.is_empty() {
        log::warn!("未找到当前播放器进程: source={source_app_id}");
        return Err(Error::Message("未找到当前播放器进程".to_owned()));
    }

    let search = find_main_window(&process_ids, player.preferred_window_classes(), media_title)?;
    let Some(candidate) = search.candidate else {
        log::warn!(
            "播放器没有可操作的主窗口: processes={}, eligible={}",
            process_ids.len(),
            search.eligible
        );
        return Err(Error::Message("当前播放器没有可操作的窗口".to_owned()));
    };

    // 只关闭已经在前台的窗口：窗口在后台时先置前，避免把用户没在看的窗口关掉。
    if candidate.visible && !candidate.minimized && is_window_active(candidate.window) {
        close_window(candidate.window);
        return Ok(());
    }

    open_player_window(&process_ids, &candidate, player)
}

/// 还原最小化窗口，或显示隐藏到托盘的主窗口。
fn open_player_window(
    process_ids: &HashSet<u32>,
    candidate: &WindowCandidate,
    player: &IdentifiedPlayer,
) -> Result<(), Error> {
    if candidate.visible {
        restore_and_activate_window(candidate.window, candidate.minimized);
        return Ok(());
    }

    // 应用自己隐藏的窗口优先由外部直接显示：客户端自有窗口类的可见位就是它的真实状态，
    // 显示与置前瞬时生效。只有下面两种情况必须换路径。
    if loads_blocking_plugin(process_ids, player.relaunch_blocking_module_fragments()) {
        show_and_activate_window(candidate.window, candidate.minimized);
        return Ok(());
    }

    // Chromium 托管的窗口（Electron/CEF 外壳）可见性由渲染进程单独维护，外部显示只会改到
    // Windows 的可见位，应用内部仍认为窗口隐藏并继续节流渲染，界面会卡死，因此交给官方入口。
    if is_chromium_managed_window(&candidate.class_name) {
        return relaunch_player(process_ids, player.relaunch_entry_names());
    }

    show_and_activate_window(candidate.window, candidate.minimized);
    Ok(())
}

/// 判断窗口（或它所属的顶层窗口，例如它弹出的对话框）当前是否在前台。
fn is_window_active(window: HWND) -> bool {
    // SAFETY: 只读取前台窗口句柄，不改变任何窗口状态。
    let foreground = unsafe { GetForegroundWindow() };
    // SAFETY: 顶层窗口取根属主后返回自身，因此同一次比较即可覆盖窗口与它的对话框。
    let root_owner = unsafe { GetAncestor(foreground, GA_ROOTOWNER) };
    root_owner == window
}

/// 请求关闭窗口；客户端通常会拦截关闭并缩回托盘继续播放，因此不能假定进程会退出。
fn close_window(window: HWND) {
    // SAFETY: window 来自本次窗口枚举，只投递关闭请求，不销毁窗口，调用立即返回。
    let _ = unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) };
}

/// 判断窗口类是否由 Chromium 托管；比对去掉大小写差异后的类名前缀。
fn is_chromium_managed_window(class_name: &str) -> bool {
    class_name
        .get(..CHROMIUM_WINDOW_CLASS_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(CHROMIUM_WINDOW_CLASS_PREFIX))
}

/// 枚举目标进程的顶层窗口，选出最可能的主窗口。
fn find_main_window<'a>(
    process_ids: &'a HashSet<u32>,
    preferred_window_classes: &'a [&'a str],
    media_title: &'a str,
) -> Result<WindowSearch<'a>, Error> {
    let mut search = WindowSearch {
        process_ids,
        preferred_window_classes,
        media_title,
        candidate: None,
        eligible: 0,
    };

    // SAFETY: 回调只在 EnumWindows 调用期间同步使用指向 search 的有效指针。
    unsafe {
        EnumWindows(
            Some(enumerate_player_window),
            LPARAM((&raw mut search).cast::<()>() as isize),
        )
    }
    .map_err(|error| Error::Message(format!("枚举播放器窗口失败: {error}")))?;

    Ok(search)
}

/// 还原最小化窗口，并请求 Windows 将其带到前台。
fn restore_and_activate_window(window: HWND, minimized: bool) {
    // SAFETY: window 来自本次 EnumWindows 枚举，在调用期间仍是有效的顶层窗口句柄。
    unsafe {
        if minimized {
            let _ = ShowWindowAsync(window, SW_RESTORE);
        }
        if !SetForegroundWindow(window).as_bool() {
            // Windows 可能依据前台锁策略拒绝抢焦点；窗口本身未被改动，不用输入模拟绕过系统限制。
            log::warn!("Windows 拒绝将播放器窗口切换到前台");
        }
    }
}

/// 直接显示已被应用隐藏的主窗口；仅用于不能重新启动自身入口的客户端。
fn show_and_activate_window(window: HWND, minimized: bool) {
    // SAFETY: window 来自本次 EnumWindows 枚举，在调用期间仍是有效的顶层窗口句柄。
    unsafe {
        let command = if minimized { SW_RESTORE } else { SW_SHOW };
        let _ = ShowWindowAsync(window, command);
        if !SetForegroundWindow(window).as_bool() {
            // Windows 可能依据前台锁策略拒绝抢焦点；窗口本身已被显示，不用输入模拟绕过系统限制。
            log::warn!("Windows 拒绝将播放器窗口切换到前台");
        }
    }
}

/// 启动播放器官方入口，由它的单实例处理把已隐藏的主窗口显示出来。
fn relaunch_player(process_ids: &HashSet<u32>, entry_names: &[&str]) -> Result<(), Error> {
    let executable = find_process_executable(process_ids)
        .ok_or_else(|| Error::Message("当前播放器窗口已隐藏，但无法读取其启动路径".to_owned()))?;
    let entry = find_launch_entry(&executable, entry_names).unwrap_or(executable);
    let mut command = Command::new(&entry);
    if let Some(directory) = entry.parent() {
        command.current_dir(directory);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Message(format!("通过播放器自身入口打开窗口失败: {error}")))
}

/// 客户端运行在带版本号的子目录时，官方入口在上层目录；
/// 按声明顺序在可执行文件目录及其上层目录中查找，找不到则沿用可执行文件本身。
fn find_launch_entry(executable: &Path, entry_names: &[&str]) -> Option<PathBuf> {
    if entry_names.is_empty() {
        return None;
    }

    let mut directory = executable.parent();
    for _ in 0..LAUNCH_ENTRY_SEARCH_LEVELS {
        let current = directory?;
        for name in entry_names {
            let candidate = current.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        directory = current.parent();
    }

    None
}

/// 判断目标进程中是否加载了会阻止重新启动入口的插件模块。
/// 读取模块列表失败时按“未加载”处理，交给窗口托管类型决定路径；
/// 若此时误判为已加载，Chromium 托管的窗口会被强制外部显示而卡死。
fn loads_blocking_plugin(process_ids: &HashSet<u32>, fragments: &[&str]) -> bool {
    if fragments.is_empty() {
        return false;
    }

    process_ids
        .iter()
        .any(|process_id| process_loads_blocking_module(*process_id, fragments))
}

/// 枚举单个进程的模块路径并匹配插件片段。
fn process_loads_blocking_module(process_id: u32, fragments: &[&str]) -> bool {
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

    let matched = unsafe { any_module_matches(process, fragments) };
    // SAFETY: process 由 OpenProcess 成功返回，这里关闭唯一句柄。
    let _ = unsafe { CloseHandle(process) };
    matched
}

/// 读取模块完整路径；插件常把文件改名放进自己的目录，因此按路径片段匹配。
unsafe fn any_module_matches(process: HANDLE, fragments: &[&str]) -> bool {
    let mut modules = [HMODULE::default(); MAX_PROCESS_MODULES];
    let mut needed = 0_u32;
    // SAFETY: modules 是可写数组，cb 与 lpcbneeded 均按同一数组计算。
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

struct WindowCandidate {
    window: HWND,
    class_name: String,
    area: i64,
    visible: bool,
    minimized: bool,
    preferred: bool,
    related: bool,
}

struct WindowSearch<'a> {
    process_ids: &'a HashSet<u32>,
    preferred_window_classes: &'a [&'a str],
    media_title: &'a str,
    candidate: Option<WindowCandidate>,
    /// 通过基础过滤的窗口数量，用于区分“没有窗口”和“窗口都不符合条件”。
    eligible: usize,
}

/// EnumWindows 回调：过滤播放器内部窗口，并保留最可能的主窗口。
unsafe extern "system" fn enumerate_player_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 由 find_main_window 传入，并在整个同步枚举期间保持有效。
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch<'_>) };
    let mut process_id = 0;
    // SAFETY: window 由 Windows 枚举提供，process_id 是有效的可写地址。
    unsafe { GetWindowThreadProcessId(window, Some(&raw mut process_id)) };
    if !search.process_ids.contains(&process_id) {
        return BOOL::from(true);
    }

    // SAFETY: 读取窗口样式、属主和可见性都不会改变窗口状态；无属主时 GetWindow 返回错误。
    let is_tool_window =
        unsafe { (GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0) != 0 };
    let has_owner = unsafe { GetWindow(window, GW_OWNER) }.is_ok();
    let visible = unsafe { IsWindowVisible(window).as_bool() };
    let minimized = unsafe { IsIconic(window).as_bool() };
    let area = window_display_area(window);
    if is_tool_window || has_owner || area <= 0 {
        return BOOL::from(true);
    }
    search.eligible += 1;

    let title = window_title(window);
    if title.is_empty() {
        return BOOL::from(true);
    }
    let class_name = window_class_name(window);
    let candidate = WindowCandidate {
        window,
        preferred: search
            .preferred_window_classes
            .contains(&class_name.as_str()),
        related: is_related_title(&title, search.media_title),
        class_name,
        area,
        visible,
        minimized,
    };
    if search.candidate.as_ref().is_none_or(|best| {
        (candidate.preferred, candidate.related, candidate.area)
            > (best.preferred, best.related, best.area)
    }) {
        search.candidate = Some(candidate);
    }

    BOOL::from(true)
}

/// 判断窗口标题是否指向当前播放内容；播放器通常把歌名写进主窗口标题。
fn is_related_title(window_title: &str, media_title: &str) -> bool {
    media_title.chars().count() >= MIN_RELATED_TITLE_LENGTH && window_title.contains(media_title)
}

/// 读取窗口标题，空标题表示该窗口不是用户可见的主窗口。
fn window_title(window: HWND) -> String {
    // SAFETY: 只查询窗口标题长度，不转移所有权。
    let length = unsafe { GetWindowTextLengthW(window) };
    if length <= 0 {
        return String::new();
    }

    let mut buffer = vec![0_u16; length as usize + 1];
    // SAFETY: buffer 的完整长度均可写，window 来自当前 EnumWindows 回调。
    let written = unsafe { GetWindowTextW(window, &mut buffer) };
    if written <= 0 {
        return String::new();
    }

    String::from_utf16_lossy(&buffer[..written as usize])
}

/// 读取顶层窗口类名，用于匹配播放器声明的主窗口类。
fn window_class_name(window: HWND) -> String {
    let mut buffer = [0_u16; 256];
    // SAFETY: buffer 的完整长度均可写，window 来自当前 EnumWindows 回调。
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return String::new();
    }

    String::from_utf16_lossy(&buffer[..length as usize])
}

/// 读取窗口的还原尺寸；最小化和隐藏窗口的当前矩形不可用，必须使用还原位置计算面积。
fn window_display_area(window: HWND) -> i64 {
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: placement 是有效可写地址，length 已按结构体大小初始化。
    if unsafe { GetWindowPlacement(window, &raw mut placement) }.is_err() {
        return 0;
    }

    let bounds = placement.rcNormalPosition;
    let width = i64::from((bounds.right - bounds.left).max(0));
    let height = i64::from((bounds.bottom - bounds.top).max(0));
    width * height
}
