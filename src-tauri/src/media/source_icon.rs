use std::{collections::HashMap, path::Path, sync::OnceLock};

use windows::{ApplicationModel::AppInfo, Foundation::Size, core::HSTRING};
use windows_icons::{
    IconSize, get_icon_base64_by_path_with_size, get_icon_base64_by_process_id_with_size,
};

use super::{process::find_process_ids, thumbnail::read_image_data_url};

const SOURCE_ICON_SIZE: f32 = 32.0;
const MAX_SOURCE_ICON_BYTES: u64 = 1024 * 1024;
static SOURCE_ICON_CACHE: OnceLock<std::sync::RwLock<HashMap<String, String>>> = OnceLock::new();

/// 读取 Windows 为 AUMID 或桌面进程提供的应用 Logo，并缓存成功结果。
pub(super) fn read_source_icon_data_url(
    source_app_id: &str,
    executable_names: &[&str],
) -> Option<String> {
    if source_app_id.is_empty() {
        return None;
    }

    let cache = SOURCE_ICON_CACHE.get_or_init(Default::default);
    if let Ok(cache) = cache.read()
        && let Some(icon) = cache.get(source_app_id)
    {
        return Some(icon.clone());
    }

    let icon = read_registered_app_logo(source_app_id)
        .ok()
        .flatten()
        .or_else(|| read_desktop_process_icon(source_app_id, executable_names));
    if let (Some(icon), Ok(mut cache)) = (&icon, cache.write()) {
        cache.insert(source_app_id.to_owned(), icon.clone());
    }
    icon
}

/// 为传统桌面播放器定位同名进程，并提取 Explorer 使用的真实应用图标。
fn read_desktop_process_icon(source_app_id: &str, executable_names: &[&str]) -> Option<String> {
    let source_path = Path::new(source_app_id);
    if source_path.is_file() {
        return get_icon_base64_by_path_with_size(source_path, IconSize::Small)
            .ok()
            .map(|base64| format!("data:image/png;base64,{base64}"));
    }

    let process_id = find_process_ids(source_app_id, executable_names)
        .into_iter()
        .next()?;
    let base64 = get_icon_base64_by_process_id_with_size(process_id, IconSize::Small).ok()?;
    Some(format!("data:image/png;base64,{base64}"))
}

/// 通过官方 AppInfo API 获取应用展示 Logo。
fn read_registered_app_logo(source_app_id: &str) -> windows::core::Result<Option<String>> {
    let app_info = AppInfo::GetFromAppUserModelId(&HSTRING::from(source_app_id))?;
    let logo = app_info.DisplayInfo()?.GetLogo(Size {
        Width: SOURCE_ICON_SIZE,
        Height: SOURCE_ICON_SIZE,
    })?;
    read_image_data_url((&logo).into(), MAX_SOURCE_ICON_BYTES)
}
