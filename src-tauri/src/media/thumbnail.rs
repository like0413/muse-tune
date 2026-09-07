use base64::{Engine as _, engine::general_purpose::STANDARD};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// 将 SMTC 缩略图流读取为可直接交给 WebView 的安全图片 Data URL。
pub(super) fn read_thumbnail_data_url(
    reference: &IRandomAccessStreamReference,
) -> windows::core::Result<Option<String>> {
    read_image_data_url(reference, MAX_IMAGE_BYTES)
}

/// 将通用 WinRT 图片流读取为 Data URL，并限制最大数据量。
pub(super) fn read_image_data_url(
    reference: &IRandomAccessStreamReference,
    max_bytes: u64,
) -> windows::core::Result<Option<String>> {
    let stream = reference.OpenReadAsync()?.get()?;
    let result = read_stream_data_url(&stream, max_bytes);
    let _ = stream.Close();
    result
}

/// 读取已打开的随机访问流，调用方负责最终关闭流。
fn read_stream_data_url(
    stream: &windows::Storage::Streams::IRandomAccessStreamWithContentType,
    max_bytes: u64,
) -> windows::core::Result<Option<String>> {
    let size = stream.Size()?;
    if size == 0 || size > max_bytes || size > u32::MAX.into() {
        return Ok(None);
    }

    let input = stream.GetInputStreamAt(0)?;
    let reader = DataReader::CreateDataReader(&input)?;
    let result =
        (|| {
            let loaded = reader.LoadAsync(size as u32)?.get()?;
            if loaded == 0 {
                return Ok(None);
            }

            let mut bytes = vec![0; loaded as usize];
            reader.ReadBytes(&mut bytes)?;
            let declared_mime_type = stream.ContentType()?.to_string();
            let mime_type = image_mime_type(&bytes, &declared_mime_type);
            Ok(mime_type
                .map(|mime_type| format!("data:{mime_type};base64,{}", STANDARD.encode(bytes))))
        })();
    let _ = reader.Close();
    result
}

/// 优先依据文件内容识别格式，只接受浏览器可安全显示的位图类型。
fn image_mime_type<'a>(bytes: &[u8], declared: &'a str) -> Option<&'a str> {
    let detected = infer::get(bytes).map(|kind| kind.mime_type());
    match detected {
        Some("image/png") => Some("image/png"),
        Some("image/jpeg") => Some("image/jpeg"),
        Some("image/webp") => Some("image/webp"),
        Some("image/gif") => Some("image/gif"),
        Some("image/bmp") => Some("image/bmp"),
        _ => match declared {
            "image/png" | "image/jpeg" | "image/webp" | "image/gif" | "image/bmp" => Some(declared),
            _ => None,
        },
    }
}
