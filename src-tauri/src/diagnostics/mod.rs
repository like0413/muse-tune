//! 汇总已有运行时快照，供设置页按需读取应用级诊断。

mod model;

use std::{
    fs,
    path::Path,
    sync::{LazyLock, Mutex},
};

use tauri::{AppHandle, Manager, Runtime};

use crate::{
    lyrics::{LyricsDiagnostics, LyricsService, LyricsStatus},
    media::MediaService,
    storage::StoragePaths,
    taskbar,
};

pub use model::{
    ApplicationDiagnostics, DiagnosticIssue, DiagnosticIssueSeverity, DiagnosticsSnapshot,
    MediaDiagnostics, StorageDiagnostics, TaskbarDiagnostics, TaskbarWindowDiagnostics,
};

static STORAGE_DIAGNOSTICS: LazyLock<Mutex<Option<StorageDiagnostics>>> =
    LazyLock::new(|| Mutex::new(None));

/// 一次性采集现有服务和窗口状态，不建立诊断专用后台任务。
pub fn collect<R: Runtime>(
    app: &AppHandle<R>,
    media_service: &MediaService,
    lyrics_service: &LyricsService,
    refresh_storage: bool,
) -> DiagnosticsSnapshot {
    let package = app.package_info();
    let mut bar_windows = app
        .webview_windows()
        .values()
        .filter(|window| window.label() == "taskbar" || window.label().starts_with("taskbar-"))
        .map(|window| TaskbarWindowDiagnostics {
            label: window.label().to_owned(),
            visible: window.is_visible().unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    bar_windows.sort_unstable_by(|left, right| left.label.cmp(&right.label));
    let visible_bar_window_count = bar_windows.iter().filter(|window| window.visible).count();
    let displays = taskbar::available_displays();
    let (
        display_target,
        placement,
        overlap_priority,
        width_mode,
        content_width_dip,
        horizontal_offset_dip,
    ) = taskbar::diagnostic_settings();
    let storage_paths = app.state::<StoragePaths>();
    let cache_directory = storage_paths.cache_directory();
    let log_directory = storage_paths.log_directory();
    let settings_file = storage_paths.settings_file();
    let lyrics = lyrics_service.diagnostics();
    let media = MediaDiagnostics::from_snapshots(
        media_service.diagnostics_snapshot(),
        media_service.runtime_diagnostics(),
    );
    let taskbar = TaskbarDiagnostics {
        detected_display_count: displays.len(),
        bar_window_count: bar_windows.len(),
        visible_bar_window_count,
        content_visible: taskbar::content_visible(),
        display_target,
        placement,
        overlap_priority,
        content_width_dip,
        horizontal_offset_dip,
        width_mode,
        displays,
        windows: bar_windows,
    };
    let storage = storage_diagnostics(Some(&settings_file), Some(log_directory), refresh_storage);
    let issues = collect_issues(&taskbar, &media, &lyrics);

    DiagnosticsSnapshot {
        application: ApplicationDiagnostics {
            name: package.name.clone(),
            version: package.version.to_string(),
            build_profile: if cfg!(debug_assertions) {
                "debug".to_owned()
            } else {
                "release".to_owned()
            },
            target_arch: std::env::consts::ARCH.to_owned(),
            target_os: std::env::consts::OS.to_owned(),
            cache_directory: Some(display_path(cache_directory)),
            log_directory: Some(display_path(log_directory)),
        },
        taskbar,
        media,
        lyrics,
        storage,
        issues,
    }
}

fn storage_diagnostics(
    settings_file: Option<&Path>,
    log_directory: Option<&Path>,
    refresh: bool,
) -> StorageDiagnostics {
    if !refresh
        && let Ok(cache) = STORAGE_DIAGNOSTICS.lock()
        && let Some(cached) = cache.as_ref()
    {
        return cached.clone();
    }
    let settings_metadata = settings_file.and_then(|path| fs::metadata(path).ok());
    let (log_file_count, log_total_bytes) = log_directory
        .and_then(|path| fs::read_dir(path).ok())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .filter(|metadata| metadata.is_file())
                .fold((0_usize, 0_u64), |(count, bytes), metadata| {
                    (count + 1, bytes.saturating_add(metadata.len()))
                })
        })
        .unwrap_or_default();
    let diagnostics = StorageDiagnostics {
        settings_file: settings_file.map(display_path),
        settings_file_exists: settings_metadata.is_some(),
        settings_file_bytes: settings_metadata.map(|metadata| metadata.len()),
        log_file_count,
        log_total_bytes,
    };
    if let Ok(mut cache) = STORAGE_DIAGNOSTICS.lock() {
        *cache = Some(diagnostics.clone());
    }
    diagnostics
}

fn collect_issues(
    taskbar: &TaskbarDiagnostics,
    media: &MediaDiagnostics,
    lyrics: &LyricsDiagnostics,
) -> Vec<DiagnosticIssue> {
    let mut issues = Vec::new();
    if taskbar.detected_display_count == 0 {
        issues.push(issue("任务栏", "没有检测到 Windows 任务栏显示器", true));
    } else if taskbar.bar_window_count == 0 {
        issues.push(issue("任务栏", "尚未创建 MuseTune Bar 窗口", false));
    }
    if let Some(error) = media.runtime_error.as_deref() {
        issues.push(issue("媒体", error, true));
    } else if media.session_available && !media.timeline_available {
        issues.push(issue("媒体", "当前播放器没有提供有效 GSMTC 时间线", false));
    }
    match lyrics.snapshot.status {
        LyricsStatus::Error => issues.push(issue(
            "歌词",
            lyrics
                .snapshot
                .error_reason
                .as_deref()
                .unwrap_or("歌词解析失败"),
            true,
        )),
        LyricsStatus::Unavailable if lyrics.enabled && lyrics.current_player.is_some() => {
            issues.push(issue(
                "歌词",
                lyrics
                    .snapshot
                    .error_reason
                    .as_deref()
                    .unwrap_or("没有可用歌词"),
                false,
            ));
        }
        LyricsStatus::Loading
        | LyricsStatus::Ready
        | LyricsStatus::Instrumental
        | LyricsStatus::NoLyrics
        | LyricsStatus::Unavailable => {}
    }
    if lyrics.cache.total_bytes > lyrics.cache.limit_bytes {
        issues.push(issue(
            "存储",
            "歌词缓存占用超过上限，将在下次写入时自动清理",
            false,
        ));
    }
    issues
}

fn issue(area: &str, message: &str, error: bool) -> DiagnosticIssue {
    DiagnosticIssue {
        severity: if error {
            DiagnosticIssueSeverity::Error
        } else {
            DiagnosticIssueSeverity::Warning
        },
        area: area.to_owned(),
        message: message.to_owned(),
    }
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}
