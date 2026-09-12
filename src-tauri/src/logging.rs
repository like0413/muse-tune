//! 应用日志的按日/容量轮转、保留和运行期清理策略。

use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Local, NaiveDate};
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::{Builder, Target, TargetKind, TimezoneStrategy, fern};

pub const LOG_FILE_MAX_BYTES: u64 = 1024 * 1024;
pub const LOG_RETENTION_DAYS: i64 = 14;
pub const LOG_ARCHIVE_FILE_LIMIT: usize = 19;
pub const LOG_STORAGE_CAPACITY_BYTES: u64 =
    LOG_FILE_MAX_BYTES * (LOG_ARCHIVE_FILE_LIMIT as u64 + 1);

#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("无法定位日志目录: {0}")]
    ResolveDirectory(tauri::Error),
    #[error("无法创建日志目录 {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("无法初始化滚动日志 {path}: {source}")]
    InitializeWriter {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("无法注册日志插件: {0}")]
    RegisterPlugin(tauri::Error),
}

struct ManagedRollingWriter {
    writer: BasicRollingFileAppender,
    log_directory: PathBuf,
    active_file_name: OsString,
    last_cleanup_date: NaiveDate,
}

impl ManagedRollingWriter {
    /// 创建每日或达到容量上限即轮转的日志写入器。
    fn new(log_directory: PathBuf, active_file_name: OsString) -> io::Result<Self> {
        let active_path = log_directory.join(&active_file_name);
        let should_rollover = active_file_needs_startup_rollover(&active_path)?;
        let condition = RollingConditionBasic::new()
            .daily()
            .max_size(LOG_FILE_MAX_BYTES);
        let mut writer =
            BasicRollingFileAppender::new(&active_path, condition, LOG_ARCHIVE_FILE_LIMIT)?;

        // rolling-file 只记录本进程的上次写入日期，启动时需校正跨日遗留文件。
        if should_rollover {
            writer.rollover()?;
        }
        prune_history(&log_directory, &active_file_name, Local::now().date_naive())?;

        Ok(Self {
            writer,
            log_directory,
            active_file_name,
            last_cleanup_date: Local::now().date_naive(),
        })
    }

    /// 日期变化后清理超过保留期的归档，不让长期运行绕过保留策略。
    fn cleanup_after_date_change(&mut self, today: NaiveDate) {
        if today == self.last_cleanup_date {
            return;
        }
        self.last_cleanup_date = today;
        if let Err(error) = prune_history(&self.log_directory, &self.active_file_name, today) {
            eprintln!("WARNING: 清理过期日志失败: {error}");
        }
    }
}

impl Write for ManagedRollingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let now = Local::now();
        let written = self.writer.write_with_datetime(buffer, &now)?;
        self.cleanup_after_date_change(now.date_naive());
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// 在应用路径可用后注册官方日志插件，并将文件目标接入 rolling-file。
pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<(), LoggingError> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(LoggingError::ResolveDirectory)?;
    fs::create_dir_all(&log_directory).map_err(|source| LoggingError::CreateDirectory {
        path: log_directory.clone(),
        source,
    })?;

    let active_file_name = OsString::from(format!("{}.log", app.package_info().name));
    let active_path = log_directory.join(&active_file_name);
    let writer = ManagedRollingWriter::new(log_directory, active_file_name).map_err(|source| {
        LoggingError::InitializeWriter {
            path: active_path,
            source,
        }
    })?;
    let file_dispatch = fern::Dispatch::new().chain(Box::new(writer) as Box<dyn Write + Send>);
    let plugin = Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::Dispatch(file_dispatch)),
        ])
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build();

    app.plugin(plugin).map_err(LoggingError::RegisterPlugin)
}

/// 删除轮转历史文件并保留当前日志句柄对应的活动文件。
pub fn clear_history<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位日志目录: {error}"))?;
    let entries = match fs::read_dir(&log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("无法读取日志目录: {error}")),
    };
    let active_file_name = OsString::from(format!("{}.log", app.package_info().name));
    log::logger().flush();
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取日志文件: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("无法识别日志文件: {error}"))?;
        if file_type.is_file() && entry.file_name() != active_file_name {
            fs::remove_file(entry.path()).map_err(|error| format!("无法删除历史日志: {error}"))?;
        }
    }
    Ok(())
}

/// 判断活动文件是否跨日遗留或已经达到单卷容量上限。
fn active_file_needs_startup_rollover(path: &Path) -> io::Result<bool> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if metadata.len() == 0 {
        return Ok(false);
    }
    let modified: DateTime<Local> = metadata.modified()?.into();
    Ok(metadata.len() >= LOG_FILE_MAX_BYTES || modified.date_naive() != Local::now().date_naive())
}

/// 按自然日和总容量清理历史文件，活动文件始终保留。
fn prune_history(
    log_directory: &Path,
    active_file_name: &OsStr,
    today: NaiveDate,
) -> io::Result<()> {
    let entries = match fs::read_dir(log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    let mut total_bytes = 0_u64;
    let mut retained_history = Vec::<(PathBuf, SystemTime, u64)>::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        let file_size = metadata.len();
        if entry.file_name() == active_file_name {
            total_bytes = total_bytes.saturating_add(file_size);
            continue;
        }
        let modified = metadata.modified()?;
        let modified: DateTime<Local> = modified.into();
        if today
            .signed_duration_since(modified.date_naive())
            .num_days()
            >= LOG_RETENTION_DAYS
        {
            fs::remove_file(entry.path())?;
            continue;
        }
        total_bytes = total_bytes.saturating_add(file_size);
        retained_history.push((entry.path(), modified.into(), file_size));
    }

    retained_history.sort_unstable_by_key(|(_, modified, _)| *modified);
    for (path, _, file_size) in retained_history {
        if total_bytes <= LOG_STORAGE_CAPACITY_BYTES {
            break;
        }
        fs::remove_file(path)?;
        total_bytes = total_bytes.saturating_sub(file_size);
    }
    Ok(())
}
