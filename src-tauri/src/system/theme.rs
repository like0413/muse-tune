use std::sync::{Arc, RwLock};

use tauri::{AppHandle, Emitter, Runtime};
use windows::{
    Foundation::TypedEventHandler,
    UI::ViewManagement::{UIColorType, UISettings},
    core::IInspectable,
};

const SYSTEM_ACCENT_COLOR_CHANGED_EVENT: &str = "system://accent-color-changed";
const SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT: &str = "system://foreground-color-changed";

struct SystemColors {
    accent: String,
    foreground: String,
}

/// 保持 Windows 颜色事件订阅在应用生命周期内有效。
pub(crate) struct SystemThemeService {
    settings: UISettings,
    color_changed_token: i64,
    colors: Arc<RwLock<SystemColors>>,
}

impl Drop for SystemThemeService {
    fn drop(&mut self) {
        let _ = self
            .settings
            .RemoveColorValuesChanged(self.color_changed_token);
    }
}

/// 订阅 Windows 系统颜色变化，并只广播真正变化的颜色。
pub(crate) fn initialize<R: Runtime>(
    app: AppHandle<R>,
) -> windows::core::Result<SystemThemeService> {
    let settings = UISettings::new()?;
    let colors = Arc::new(RwLock::new(read_system_colors(&settings)?));
    let event_settings = settings.clone();
    let event_colors = Arc::clone(&colors);
    let color_changed_token = settings.ColorValuesChanged(&TypedEventHandler::<
        UISettings,
        IInspectable,
    >::new(move |_, _| {
        if let Ok(next) = read_system_colors(&event_settings) {
            let changes = event_colors.write().ok().map(|mut current| {
                let accent = (current.accent != next.accent).then(|| next.accent.clone());
                let foreground =
                    (current.foreground != next.foreground).then(|| next.foreground.clone());
                *current = next;
                (accent, foreground)
            });
            if let Some((accent, foreground)) = changes {
                if let Some(color) = accent {
                    let _ = app.emit(SYSTEM_ACCENT_COLOR_CHANGED_EVENT, color);
                }
                if let Some(color) = foreground {
                    let _ = app.emit(SYSTEM_FOREGROUND_COLOR_CHANGED_EVENT, color);
                }
            }
        }
        Ok(())
    }))?;

    Ok(SystemThemeService {
        settings,
        color_changed_token,
        colors,
    })
}

impl SystemThemeService {
    /// 返回服务缓存的 Windows 当前强调色。
    pub(crate) fn accent_color(&self) -> Result<String, String> {
        self.colors
            .read()
            .map(|colors| colors.accent.clone())
            .map_err(|_| "Windows 系统颜色缓存不可用".to_owned())
    }

    /// 返回服务缓存的 Windows 当前前景色。
    pub(crate) fn foreground_color(&self) -> Result<String, String> {
        self.colors
            .read()
            .map(|colors| colors.foreground.clone())
            .map_err(|_| "Windows 系统颜色缓存不可用".to_owned())
    }
}

/// 从 UISettings 一次读取任务栏需要的系统颜色。
fn read_system_colors(settings: &UISettings) -> windows::core::Result<SystemColors> {
    Ok(SystemColors {
        accent: read_color(settings, UIColorType::Accent)?,
        foreground: read_color(settings, UIColorType::Foreground)?,
    })
}

/// 将 Windows Color 转换为 CSS 六位十六进制颜色。
fn read_color(settings: &UISettings, color_type: UIColorType) -> windows::core::Result<String> {
    let color = settings.GetColorValue(color_type)?;
    Ok(format!("#{:02x}{:02x}{:02x}", color.R, color.G, color.B))
}
