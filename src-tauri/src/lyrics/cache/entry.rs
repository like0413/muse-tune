//! 单条歌词缓存的磁盘格式、压缩与原子提交。

use std::{
    io::{Read, Write},
    path::Path,
};

#[cfg(not(windows))]
use std::fs;

use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use serde::{Deserialize, Serialize};

use crate::lyrics::{
    error::LyricsError,
    model::{LyricsChineseVariant, LyricsSnapshot},
};

/// 解压后的单条缓存上限；压缩只是落盘手段，容量约束始终按规范化歌词本体计算。
pub(super) const MAX_CACHE_ENTRY_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CacheEntry {
    pub(super) refreshed_at_seconds: u64,
    pub(super) chinese_variant: LyricsChineseVariant,
    pub(super) snapshot: LyricsSnapshot,
}

/// 把条目编码为 deflate 压缩的 JSON。
pub(super) fn encode_entry(entry: &CacheEntry) -> Result<Vec<u8>, LyricsError> {
    let json = serde_json::to_vec(entry)?;
    // 上限约束的是规范化歌词本体，压缩后的字节数不能作为判据。
    if json.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return Err(LyricsError::InvalidData(
            "规范化歌词超过缓存大小上限".to_owned(),
        ));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    Ok(encoder.finish()?)
}

/// 解压单个条目；损坏或解压后超过上限时按无效条目处理。
pub(super) fn decode_entry(content: &[u8]) -> Option<CacheEntry> {
    let mut json = Vec::new();
    // 压缩流可以声称解压出任意大小，必须先限流再解析。
    DeflateDecoder::new(content)
        .take(MAX_CACHE_ENTRY_BYTES + 1)
        .read_to_end(&mut json)
        .ok()?;
    if json.len() as u64 > MAX_CACHE_ENTRY_BYTES {
        return None;
    }
    serde_json::from_slice(&json).ok()
}

/// 原子地用临时文件替换已有缓存文件。
pub(super) fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        use windows::core::PCWSTR;

        let source = wide_null(source);
        let target = wide_null(target);
        // SAFETY: 两个指针均指向以 NUL 结尾的有效 UTF-16 缓冲区，生命周期覆盖整个调用。
        unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(|error| std::io::Error::other(error.to_string()))
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, target)
    }
}

/// 把路径编码为 Win32 宽字符接口使用的 NUL 结尾 UTF-16。
#[cfg(windows)]
fn wide_null(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
