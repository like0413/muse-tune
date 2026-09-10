use std::collections::BTreeSet;

use windows::Win32::{
    Foundation::LPARAM,
    Graphics::Gdi::{
        CreateCompatibleDC, DEFAULT_CHARSET, DeleteDC, EnumFontFamiliesExW, HDC, LOGFONTW,
        TEXTMETRICW,
    },
};

struct FontEnumerationDc(HDC);

impl Drop for FontEnumerationDc {
    fn drop(&mut self) {
        // SAFETY: 该句柄只由 CreateCompatibleDC 创建，并且仅在此处释放一次。
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// 收集 Windows 当前可用的字体族名称并去除字符集造成的重复项。
pub(crate) fn list_system_fonts() -> windows::core::Result<Vec<String>> {
    // SAFETY: 空参数创建与当前屏幕兼容的内存 DC，不借用外部句柄。
    let dc = FontEnumerationDc(unsafe { CreateCompatibleDC(None) });
    if dc.0.is_invalid() {
        return Err(windows::core::Error::from_win32());
    }

    let query = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    let mut families: BTreeSet<String> = BTreeSet::new();
    let context = LPARAM((&mut families as *mut BTreeSet<String>) as isize);

    // SAFETY: query 和 families 在同步枚举期间持续有效；回调不会保存传入指针。
    let result = unsafe {
        EnumFontFamiliesExW(
            dc.0,
            &raw const query,
            Some(collect_font_family),
            context,
            0,
        )
    };
    if result == 0 && families.is_empty() {
        return Err(windows::core::Error::from_win32());
    }

    let mut families = families.into_iter().collect::<Vec<_>>();
    families.sort_by_key(|name| !contains_chinese_character(name));
    Ok(families)
}

/// 判断字体族名称是否包含汉字，用于把中文名称分组到列表前部。
fn contains_chinese_character(name: &str) -> bool {
    name.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3134F
        )
    })
}

/// GDI 字体枚举回调；返回 1 以继续枚举剩余字体族。
unsafe extern "system" fn collect_font_family(
    log_font: *const LOGFONTW,
    _text_metric: *const TEXTMETRICW,
    _font_type: u32,
    context: LPARAM,
) -> i32 {
    if log_font.is_null() || context.0 == 0 {
        return 1;
    }

    // SAFETY: 两个指针均由 list_system_fonts 在本次同步调用中提供并保持有效。
    let (face_name, families) = unsafe {
        (
            &(*log_font).lfFaceName,
            &mut *(context.0 as *mut BTreeSet<String>),
        )
    };
    let length = face_name
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(face_name.len());
    let name = String::from_utf16_lossy(&face_name[..length]);

    // 竖排字体以 @ 开头，CSS 横排歌词无需重复展示。
    if !name.is_empty() && !name.starts_with('@') {
        families.insert(name);
    }
    1
}
