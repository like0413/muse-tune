mod commands;
mod diagnostics;
mod lyrics;
mod media;
mod settings_store;
mod system;
mod taskbar;
mod tray;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = commands::settings::open_settings_window(app).await {
                    log::error!("再次启动应用时打开设置窗口失败: {error}");
                }
            });
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::diagnostics::get_diagnostics,
            commands::settings::open_settings_window,
            commands::media::activate_current_media_player,
            commands::media::control_media_session,
            commands::media::get_current_media_volume,
            commands::media::get_current_media_session,
            commands::media::set_current_media_volume,
            commands::media::set_media_spectrum_enabled,
            commands::media::set_media_session_selection_policy,
            commands::media::toggle_current_media_mute,
            commands::lyrics::get_current_lyrics,
            commands::lyrics::set_lyrics_enabled,
            commands::system::list_system_fonts,
            commands::system::get_system_accent_color,
            commands::system::get_system_foreground_color,
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
            let lyrics_service = lyrics::initialize(app)?;
            app.manage(lyrics_service);
            let media_service = media::initialize(app.handle().clone())?;
            app.manage(media_service);
            let system_theme_service = system::initialize(app.handle().clone())?;
            app.manage(system_theme_service);
            taskbar::initialize(app)?;
            tray::initialize(app)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                // Explorer 重启时会销毁由任务栏持有的 bar 窗口，因此保留进程，交由监控线程重建窗口。
                api.prevent_exit();
            }
        });
}
