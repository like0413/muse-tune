//! 应用日志的原生容量轮转、防刷屏和运行期清理策略。

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    ffi::OsString,
    fs, io, panic,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};
use time::{format_description::FormatItem, macros::format_description};

use crate::{error::Error, filesystem};

pub const LOG_FILE_MAX_BYTES: u64 = 512 * 1024;
pub const LOG_ARCHIVE_FILE_LIMIT: usize = 9;
pub const LOG_STORAGE_CAPACITY_BYTES: u64 =
    LOG_FILE_MAX_BYTES * (LOG_ARCHIVE_FILE_LIMIT as u64 + 1);

/// 行内时间戳与归档文件名共用同一时区策略，避免两处各写一份导致对不上。
const TIMEZONE: TimezoneStrategy = TimezoneStrategy::UseLocal;

/// 时间戳格式，与插件默认值保持一致：`[2026-09-21][14:03:52]`。
const TIMESTAMP_FORMAT: &[FormatItem<'_>] =
    format_description!("[[[year]-[month]-[day]][[[hour]:[minute]:[second]]");

/// 同一调用点的重复告警最短间隔。
///
/// 热路径上的读取失败会按事件频率触发（时间线事件可达每秒一次），不限频就会以同样的文案
/// 填满 512KB 的当前文件并开始轮转，把真正有用的历史冲掉。
const REPEATED_WARNING_INTERVAL: Duration = Duration::from_secs(5 * 60);

thread_local! {
    /// 各线程最近一次记录重复告警的时间。键是调用点而不是具体对象：条目数因此有界，
    /// 不会随会话或歌曲数量增长。相关线程都是长期存活的后台 worker。
    static LAST_WARNING_AT: RefCell<HashMap<&'static str, Instant>> = RefCell::new(HashMap::new());
}

/// 本 crate 的模块前缀：模块路径每条记录都要写一遍，而它永远是同一个值。
const CRATE_PREFIX: &str = concat!(env!("CARGO_CRATE_NAME"), "::");

/// 压缩 target 的展示长度。
///
/// 前端日志的 target 由插件拼成 `webview:函数@资源路径:行:列`。正式构建里资源名带哈希、
/// 行列号指向压缩后的产物，两者都定位不到任何东西，只保留函数名。
fn shorten_target(target: &str) -> Cow<'_, str> {
    if let Some(rest) = target.strip_prefix("webview") {
        let name = rest
            .strip_prefix(':')
            .unwrap_or(rest)
            .split('@')
            .next()
            .unwrap_or_default();
        return if name.is_empty() {
            Cow::Borrowed("前端")
        } else {
            Cow::Owned(format!("前端:{name}"))
        };
    }
    Cow::Borrowed(target.strip_prefix(CRATE_PREFIX).unwrap_or(target))
}

/// 当前本地时间；无法确定本地时区时回退到 UTC，与插件的处理一致。
fn local_timestamp() -> String {
    TIMEZONE
        .get_now()
        .format(TIMESTAMP_FORMAT)
        .unwrap_or_else(|_| "[未知时间]".to_owned())
}

/// 组装日志插件。
///
/// 只记录 warn 与 error：正常运行信息会随播放行为持续累积（每切一首歌、每次窗口重建各一条），
/// 很快就会把有限的日志预算挤满，反而淹没真正要排查的报错。级别在这里统一设定，
/// 新增的 info 日志不会意外撑大文件。
///
/// 行格式沿用插件默认布局，只把 target 换成压缩后的形式。
///
/// 必须注册在 `Builder` 上而不是 `setup` 内：插件初始化顺序在 `setup` 之前，
/// 晚注册会让其它插件初始化阶段产生的日志无处可去。
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .level(tauri_plugin_log::log::LevelFilter::Warn)
        .timezone_strategy(TIMEZONE)
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                local_timestamp(),
                shorten_target(record.target()),
                record.level(),
                message
            ))
        })
        .max_file_size(LOG_FILE_MAX_BYTES.into())
        .rotation_strategy(RotationStrategy::KeepSome(LOG_ARCHIVE_FILE_LIMIT))
        .build()
}

/// 把 panic 写进日志文件。
///
/// 正式构建设置了 `windows_subsystem = "windows"`，标准 panic 钩子写往的 stderr 没有任何
/// 落点：线程崩溃后只能看到 join 失败的“线程异常退出”，无法知道崩在哪里、为什么崩。
pub fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(|| "未知位置".to_owned(), ToString::to_string);
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "非字符串 panic 负载".to_owned());
        log::error!("线程 panic: {payload} @ {location}");
        // 保留标准钩子：开发构建下 stderr 仍然可见，行为与未安装时一致。
        previous(info);
    }));
}

/// 记录同一调用点的重复告警，最短间隔内的同类告警直接丢弃。
///
/// 只用于“失败会按事件频率持续复现”的热路径：限频后仍会周期性留下记录，因此不会丢失
/// “这个问题还在持续”的信号，但不会把日志预算吃光。
pub fn warn_throttled(key: &'static str, message: impl FnOnce() -> String) {
    let now = Instant::now();
    let should_log = LAST_WARNING_AT.with(|cell| {
        let mut entries = cell.borrow_mut();
        match entries.get(key) {
            Some(last) if now.duration_since(*last) < REPEATED_WARNING_INTERVAL => false,
            _ => {
                entries.insert(key, now);
                true
            }
        }
    });
    if should_log {
        log::warn!("{}", message());
    }
}

/// 删除轮转历史文件并保留当前日志句柄对应的活动文件。
pub fn clear_history<R: Runtime>(app: &AppHandle<R>) -> Result<(), Error> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| Error::Message(format!("无法定位日志目录: {error}")))?;
    match filesystem::ensure_directory(&log_directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::Message(format!("日志目录边界检查失败: {error}"))),
    }
    let entries = match fs::read_dir(&log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::Message(format!("无法读取日志目录: {error}"))),
    };
    let active_file_name = OsString::from(format!("{}.log", app.package_info().name));
    log::logger().flush();
    for entry in entries {
        let entry = entry.map_err(|error| Error::Message(format!("无法读取日志文件: {error}")))?;
        let metadata = filesystem::entry_metadata_without_reparse(&entry)
            .map_err(|error| Error::Message(format!("日志文件边界检查失败: {error}")))?;
        if metadata.is_file() && entry.file_name() != active_file_name {
            fs::remove_file(entry.path())
                .map_err(|error| Error::Message(format!("无法删除历史日志: {error}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 前端 target 只保留函数名：压缩产物路径与行列号在正式构建里定位不到任何东西。
    #[test]
    fn frontend_target_keeps_only_function_name() {
        assert_eq!(shorten_target("webview"), "前端");
        assert_eq!(shorten_target("webview:"), "前端");
        assert_eq!(
            shorten_target(
                "webview:refreshFonts@http://tauri.localhost/assets/index-B3xK9q.js:1:23841"
            ),
            "前端:refreshFonts"
        );
    }

    /// 自身模块前缀每条记录都重复，去掉即可；第三方 target 必须原样保留以区分来源。
    #[test]
    fn own_module_prefix_is_stripped_but_others_kept() {
        assert_eq!(
            shorten_target("muse_tune_lib::media::monitor::playback"),
            "media::monitor::playback"
        );
        assert_eq!(
            shorten_target("tauri_plugin_updater::updater"),
            "tauri_plugin_updater::updater"
        );
    }
}
