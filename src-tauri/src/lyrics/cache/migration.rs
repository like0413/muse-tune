//! 旧版缓存目录布局的一次性迁移与清理。
//!
//! 只在启动时跑一次，失败必须保留旧数据供下次重试——迁移逻辑一旦“尽力而为”地删掉源目录，
//! 用户会直接丢缓存。这里所有失败路径都只记日志、不删除。

use std::{fs, path::Path};

use crate::{filesystem, lyrics::schema::LYRICS_CACHE_SCHEMA_VERSION};

/// 将旧的 `vN/entries/*.json` 原地迁移到版本目录，失败时保留旧目录供下次重试。
pub(super) fn migrate_legacy_entries_directory(cache_path: &Path) {
    let legacy_path = cache_path.join("entries");
    let Ok(legacy_metadata) = filesystem::metadata_without_reparse(&legacy_path) else {
        return;
    };
    if !legacy_metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(&legacy_path) else {
        return;
    };
    let mut migration_failed = false;
    for entry in entries {
        let Ok(entry) = entry else {
            migration_failed = true;
            continue;
        };
        let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
            migration_failed = true;
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let source = entry.path();
        if source.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let target = cache_path.join(entry.file_name());
        let result = match filesystem::metadata_without_reparse(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::rename(&source, &target)
            }
            Ok(metadata) if metadata.is_file() => fs::remove_file(&source),
            Ok(_) => {
                log::warn!("迁移歌词缓存时目标路径不是普通文件: {}", target.display());
                migration_failed = true;
                continue;
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            log::warn!("迁移旧版歌词缓存 {} 失败: {error}", source.display());
            migration_failed = true;
        }
    }
    if !migration_failed && let Err(error) = fs::remove_dir_all(&legacy_path) {
        log::warn!("删除已迁移的歌词 entries 目录失败: {error}");
    }
}

/// 启动时仅清理旧版目录；未知目录和更高版本需保留，以支持安全回退。
pub(super) fn remove_obsolete_schema_directories(lyrics_path: &Path) {
    if let Err(error) = filesystem::ensure_directory(lyrics_path) {
        log::warn!("歌词缓存根目录边界检查失败: {error}");
        return;
    }
    let Ok(entries) = fs::read_dir(lyrics_path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }
        let Some(version) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix('v'))
            .and_then(|version| version.parse::<u32>().ok())
        else {
            continue;
        };
        if version >= LYRICS_CACHE_SCHEMA_VERSION {
            continue;
        }
        if let Err(error) = fs::remove_dir_all(entry.path()) {
            log::warn!("清理旧版歌词缓存 v{version} 失败: {error}");
        }
    }
}
