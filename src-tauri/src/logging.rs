//! 应用日志的轮转、容量和运行期清理策略。

use std::{ffi::OsStr, fs};

use tauri::{AppHandle, Manager, Runtime, plugin::TauriPlugin};
use tauri_plugin_log::{Builder, FileOpenStrategy, RotationStrategy, TimezoneStrategy};

pub const LOG_FILE_MAX_BYTES: u64 = 1024 * 1024;
pub const LOG_ARCHIVE_FILE_LIMIT: usize = 4;
pub const LOG_STORAGE_CAPACITY_BYTES: u64 =
    LOG_FILE_MAX_BYTES * (LOG_ARCHIVE_FILE_LIMIT as u64 + 1);

/// 构建有界、使用本地时间且跨启动连续写入的官方日志插件。
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .max_file_size(LOG_FILE_MAX_BYTES.into())
        .rotation_strategy(RotationStrategy::KeepSome(LOG_ARCHIVE_FILE_LIMIT))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .file_open_strategy(FileOpenStrategy::Append)
        .build()
}

/// 删除轮转历史文件并保留当前日志句柄对应的活动文件。
pub fn clear_history<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位日志目录: {error}"))?;
    let entries = match fs::read_dir(&log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("无法读取日志目录: {error}")),
    };
    let active_file_name = format!("{}.log", app.package_info().name);
    log::logger().flush();
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取日志文件: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("无法识别日志文件: {error}"))?;
        if file_type.is_file() && entry.file_name() != OsStr::new(&active_file_name) {
            fs::remove_file(entry.path()).map_err(|error| format!("无法删除历史日志: {error}"))?;
        }
    }
    Ok(())
}
