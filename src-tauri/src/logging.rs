//! 应用日志的原生容量轮转和运行期清理策略。

use std::{ffi::OsString, fs, io};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};

use crate::filesystem;

pub const LOG_FILE_MAX_BYTES: u64 = 512 * 1024;
pub const LOG_ARCHIVE_FILE_LIMIT: usize = 9;
pub const LOG_STORAGE_CAPACITY_BYTES: u64 =
    LOG_FILE_MAX_BYTES * (LOG_ARCHIVE_FILE_LIMIT as u64 + 1);

#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("无法注册日志插件: {0}")]
    RegisterPlugin(tauri::Error),
}

/// 注册官方日志插件，并使用其原生文件容量轮转能力。
pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<(), LoggingError> {
    let plugin = Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .max_file_size(LOG_FILE_MAX_BYTES.into())
        .rotation_strategy(RotationStrategy::KeepSome(LOG_ARCHIVE_FILE_LIMIT))
        .build();

    app.plugin(plugin).map_err(LoggingError::RegisterPlugin)
}

/// 删除轮转历史文件并保留当前日志句柄对应的活动文件。
pub fn clear_history<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位日志目录: {error}"))?;
    match filesystem::ensure_directory(&log_directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("日志目录边界检查失败: {error}")),
    }
    let entries = match fs::read_dir(&log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("无法读取日志目录: {error}")),
    };
    let active_file_name = OsString::from(format!("{}.log", app.package_info().name));
    log::logger().flush();
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取日志文件: {error}"))?;
        let metadata = filesystem::entry_metadata_without_reparse(&entry)
            .map_err(|error| format!("日志文件边界检查失败: {error}"))?;
        if metadata.is_file() && entry.file_name() != active_file_name {
            fs::remove_file(entry.path()).map_err(|error| format!("无法删除历史日志: {error}"))?;
        }
    }
    Ok(())
}
