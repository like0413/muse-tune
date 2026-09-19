mod commands;
mod data;
mod diagnostics;
mod filesystem;
mod ipc;
mod logging;
mod lyrics;
mod media;
mod settings_store;
mod system;
mod taskbar;
mod tray;

use std::sync::Arc;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = commands::settings::open_settings_window(app).await {
                    log::error!("再次启动应用时打开设置窗口失败: {error}");
                }
            });
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::data::clear_lyrics_cache,
            commands::data::clear_current_lyrics_cache,
            commands::data::clear_log_history,
            commands::data::get_data_overview,
            commands::data::open_data_directory,
            commands::data::reset_configuration,
            commands::data::refresh_current_lyrics,
            commands::diagnostics::collect_diagnostics,
            commands::settings::open_settings_window,
            commands::media::control_media_session,
            commands::media::toggle_current_media_player,
            commands::media::get_current_media_volume,
            commands::media::get_current_media_session,
            commands::media::set_current_media_volume,
            commands::media::set_media_spectrum_enabled,
            commands::media::set_media_session_selection_policy,
            commands::media::toggle_current_media_mute,
            commands::lyrics::get_current_lyrics,
            commands::lyrics::set_lyrics_preferences,
            commands::system::list_system_fonts,
            commands::system::get_system_accent_color,
            commands::system::get_system_foreground_color,
            commands::system::restart_application,
            commands::tray::set_tray_menu_state,
            commands::taskbar::list_taskbar_displays,
            commands::taskbar::set_taskbar_display_target,
            commands::taskbar::set_taskbar_content_visibility,
            commands::taskbar::set_taskbar_overlap_priority,
            commands::taskbar::set_taskbar_placement,
            commands::taskbar::set_taskbar_width,
            commands::taskbar::show_volume_popup,
            commands::taskbar::hide_volume_popup
        ])
        .setup(|app| {
            logging::initialize(app.handle())?;
            let lyrics_service = lyrics::initialize(app)?;
            let lyrics_snapshot_subscriber = lyrics_service.clone();
            let media_service = media::initialize(
                app.handle().clone(),
                Arc::new(move |snapshot| {
                    lyrics_snapshot_subscriber.update_media(snapshot.as_ref());
                }),
            )?;
            app.manage(lyrics_service);
            app.manage(media_service);
            let system_theme_service = system::initialize(app.handle().clone())?;
            app.manage(system_theme_service);
            let taskbar_service = taskbar::initialize(app)?;
            app.manage(taskbar_service);
            tray::initialize(app)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } => {
                // Explorer 重启时会销毁由任务栏持有的 bar 窗口，因此保留进程，交由监控线程重建窗口。
                api.prevent_exit();
            }
            tauri::RunEvent::ExitRequested { code: Some(_), .. } => {
                // 先发停止信号，让依赖主事件循环的后台操作有机会在最终 Exit 前返回。
                app.state::<media::MediaService>().request_shutdown();
                app.state::<taskbar::TaskbarService>().request_shutdown();
                app.state::<lyrics::LyricsService>().shutdown();
                app.state::<system::SystemThemeService>().shutdown();
            }
            tauri::RunEvent::Exit => {
                app.state::<media::MediaService>().shutdown();
                app.state::<taskbar::TaskbarService>().shutdown();
                app.state::<lyrics::LyricsService>().shutdown();
                app.state::<system::SystemThemeService>().shutdown();
            }
            _ => {}
        });
}
