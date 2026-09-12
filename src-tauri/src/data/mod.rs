//! 提供设置页数据管理所需的目录、容量和清理能力。

use std::{fs, path::Path};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;
use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    core::{HSTRING, w},
};

use crate::{lyrics::LyricsService, settings_store};

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDirectoryKind {
    Cache,
    Config,
    Logs,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataOverview {
    pub cache: CacheOverview,
    pub config: ConfigOverview,
    pub logs: LogsOverview,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheOverview {
    pub entry_count: usize,
    pub used_bytes: u64,
    pub capacity_bytes: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOverview {
    pub settings_file_exists: bool,
    pub settings_file_bytes: Option<u64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsOverview {
    pub file_count: usize,
    pub total_bytes: u64,
}

/// 汇总缓存、配置和日志的轻量存储状态。
pub fn overview<R: Runtime>(
    app: &AppHandle<R>,
    lyrics: &LyricsService,
) -> Result<DataOverview, String> {
    let config_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位配置目录: {error}"))?;
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位日志目录: {error}"))?;
    let settings_file = config_directory.join(settings_store::PATH);
    let settings_metadata = fs::metadata(&settings_file).ok();
    let (log_file_count, log_total_bytes) = directory_file_totals(&log_directory);
    let cache = lyrics.cache_diagnostics();

    Ok(DataOverview {
        cache: CacheOverview {
            entry_count: cache.entry_count,
            used_bytes: cache.total_bytes,
            capacity_bytes: cache.limit_bytes,
        },
        config: ConfigOverview {
            settings_file_exists: settings_metadata.is_some(),
            settings_file_bytes: settings_metadata.map(|metadata| metadata.len()),
        },
        logs: LogsOverview {
            file_count: log_file_count,
            total_bytes: log_total_bytes,
        },
    })
}

/// 使用 Windows Shell 打开指定的应用数据目录。
pub fn open_directory<R: Runtime>(
    app: &AppHandle<R>,
    lyrics: &LyricsService,
    kind: DataDirectoryKind,
) -> Result<(), String> {
    let path = match kind {
        DataDirectoryKind::Cache => lyrics.cache_directory().to_path_buf(),
        DataDirectoryKind::Config => app
            .path()
            .app_data_dir()
            .map_err(|error| format!("无法定位配置目录: {error}"))?,
        DataDirectoryKind::Logs => app
            .path()
            .app_log_dir()
            .map_err(|error| format!("无法定位日志目录: {error}"))?,
    };
    fs::create_dir_all(&path).map_err(|error| format!("无法创建目录: {error}"))?;

    let target = HSTRING::from(path.to_string_lossy().as_ref());
    // SAFETY: 传入的路径由 Tauri PathResolver 生成，字符串在调用期间保持有效。
    let result = unsafe { ShellExecuteW(None, w!("open"), &target, None, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        return Err(format!(
            "Windows 无法打开目录，错误代码: {}",
            result.0 as isize
        ));
    }
    Ok(())
}

/// 使用官方 Store API 恢复默认配置并立即持久化。
pub fn reset_configuration<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let store = app
        .store(settings_store::PATH)
        .map_err(|error| format!("无法读取配置存储: {error}"))?;
    store.reset();
    store
        .save()
        .map_err(|error| format!("无法保存默认配置: {error}"))
}

/// 删除应用日志目录中的文件；调用方应随后重启以重新建立日志文件句柄。
pub fn clear_logs<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位日志目录: {error}"))?;
    let entries = match fs::read_dir(&log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("无法读取日志目录: {error}")),
    };

    log::logger().flush();
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取日志文件: {error}"))?;
        if entry
            .file_type()
            .map_err(|error| format!("无法识别日志文件: {error}"))?
            .is_file()
        {
            fs::remove_file(entry.path()).map_err(|error| format!("无法删除日志文件: {error}"))?;
        }
    }
    Ok(())
}

fn directory_file_totals(path: &Path) -> (usize, u64) {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .fold((0, 0), |(count, bytes), metadata| {
            (count + 1, bytes.saturating_add(metadata.len()))
        })
}
