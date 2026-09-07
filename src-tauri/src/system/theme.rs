use std::sync::{Arc, RwLock};

use tauri::{AppHandle, Emitter, Runtime};
use windows::{
    Foundation::TypedEventHandler,
    UI::ViewManagement::{UIColorType, UISettings},
    core::IInspectable,
};

const SYSTEM_ACCENT_COLOR_CHANGED_EVENT: &str = "system://accent-color-changed";

/// 保持 Windows 颜色事件订阅在应用生命周期内有效。
pub(crate) struct SystemThemeService {
    settings: UISettings,
    color_changed_token: i64,
    accent_color: Arc<RwLock<String>>,
}

impl Drop for SystemThemeService {
    fn drop(&mut self) {
        let _ = self
            .settings
            .RemoveColorValuesChanged(self.color_changed_token);
    }
}

/// 订阅 Windows 强调色变化，并向全部 WebView 广播最新颜色。
pub(crate) fn initialize<R: Runtime>(
    app: AppHandle<R>,
) -> windows::core::Result<SystemThemeService> {
    let settings = UISettings::new()?;
    let accent_color = Arc::new(RwLock::new(read_accent_color(&settings)?));
    let event_settings = settings.clone();
    let event_accent_color = Arc::clone(&accent_color);
    let color_changed_token = settings.ColorValuesChanged(&TypedEventHandler::<
        UISettings,
        IInspectable,
    >::new(move |_, _| {
        if let Ok(color) = read_accent_color(&event_settings) {
            let changed = event_accent_color.write().is_ok_and(|mut current| {
                if *current == color {
                    return false;
                }
                current.clone_from(&color);
                true
            });
            if changed {
                let _ = app.emit(SYSTEM_ACCENT_COLOR_CHANGED_EVENT, color);
            }
        }
        Ok(())
    }))?;

    Ok(SystemThemeService {
        settings,
        color_changed_token,
        accent_color,
    })
}

impl SystemThemeService {
    /// 返回服务缓存的 Windows 当前强调色。
    pub(crate) fn accent_color(&self) -> Result<String, String> {
        self.accent_color
            .read()
            .map(|color| color.clone())
            .map_err(|_| "Windows 强调色缓存不可用".to_owned())
    }
}

/// 从 UISettings 读取强调色并转换格式。
fn read_accent_color(settings: &UISettings) -> windows::core::Result<String> {
    let color = settings.GetColorValue(UIColorType::Accent)?;
    Ok(format!("#{:02x}{:02x}{:02x}", color.R, color.G, color.B))
}
