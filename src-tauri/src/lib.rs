mod commands;
mod taskbar;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::settings::open_settings_window,
            commands::taskbar::set_taskbar_overlap_priority,
            commands::taskbar::set_taskbar_placement
        ])
        .setup(|app| {
            taskbar::initialize(app)?;

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
