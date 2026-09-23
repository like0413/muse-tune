//! 歌词缓存目录边界检查与容量裁剪。

use std::{fs, path::Path, time::UNIX_EPOCH};

use crate::filesystem;

use super::{MAX_CACHE_TOTAL_BYTES, diagnostics::CacheTotals};

/// 逐级验证应用缓存根、歌词根和版本目录，避免父级 junction 隐藏最终真实位置。
pub(super) fn ensure_directory_boundary(cache_path: &Path) -> Result<(), std::io::Error> {
    let lyrics_path = cache_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "歌词缓存目录缺少父级")
    })?;
    let app_cache_dir = lyrics_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "歌词缓存根目录缺少父级")
    })?;
    filesystem::ensure_directory(app_cache_dir)?;
    filesystem::ensure_directory(lyrics_path)?;
    filesystem::ensure_directory(cache_path)
}

/// 写入后按已有可信总量增量记账，接近上限时回退到真实目录扫描。
pub(super) fn prune_after_write(
    cache_path: &Path,
    tracked: Option<CacheTotals>,
    protected_path: &Path,
    written_bytes: u64,
    replaced_bytes: Option<u64>,
) -> Result<CacheTotals, std::io::Error> {
    let Some(tracked) = tracked else {
        return prune_to_size_limit(cache_path, protected_path);
    };
    let entry_count = if replaced_bytes.is_some() {
        tracked.entry_count
    } else {
        tracked.entry_count.saturating_add(1)
    };
    let total_bytes = tracked
        .total_bytes
        .saturating_add(written_bytes)
        .saturating_sub(replaced_bytes.unwrap_or(0));
    if total_bytes > MAX_CACHE_TOTAL_BYTES {
        return prune_to_size_limit(cache_path, protected_path);
    }
    Ok(CacheTotals {
        entry_count,
        total_bytes,
    })
}

/// 按最近写入时间淘汰旧条目，使永久运行也不会无限占用磁盘。
fn prune_to_size_limit(
    cache_path: &Path,
    protected_path: &Path,
) -> Result<CacheTotals, std::io::Error> {
    ensure_directory_boundary(cache_path)?;
    let mut total_bytes = 0_u64;
    let mut entry_count = 0_usize;
    let mut entries = Vec::new();
    for entry in fs::read_dir(cache_path)? {
        let entry = entry?;
        let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        entry_count += 1;
        total_bytes = total_bytes.saturating_add(metadata.len());
        entries.push((
            metadata.modified().unwrap_or(UNIX_EPOCH),
            metadata.len(),
            entry.path(),
        ));
    }
    if total_bytes <= MAX_CACHE_TOTAL_BYTES {
        return Ok(CacheTotals {
            entry_count,
            total_bytes,
        });
    }

    entries.sort_unstable_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in entries {
        if total_bytes <= MAX_CACHE_TOTAL_BYTES {
            break;
        }
        if path == protected_path {
            continue;
        }
        fs::remove_file(path)?;
        entry_count = entry_count.saturating_sub(1);
        total_bytes = total_bytes.saturating_sub(size);
    }
    Ok(CacheTotals {
        entry_count,
        total_bytes,
    })
}
