use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{filesystem, media::MediaPlayer};

use super::{
    error::LyricsError,
    model::{LyricsCacheDiagnostics, LyricsChineseVariant, LyricsSnapshot, LyricsSourceKind},
    schema::lyrics_cache_schema_label,
};

mod diagnostics;
mod entry;
mod freshness;
mod migration;
mod pruning;

use diagnostics::CacheDiagnostics;
use entry::{CacheEntry, MAX_CACHE_ENTRY_BYTES, decode_entry, encode_entry, replace_file};
use freshness::{is_cacheable_status, is_fresh, needs_revalidation, now_seconds};
use migration::{migrate_legacy_entries_directory, remove_obsolete_schema_directories};
use pruning::{ensure_directory_boundary, prune_after_write};

const MAX_CACHE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
/// `write_generations` 表的上限：超过后裁剪最旧的写入记录，避免长时间切歌时单调增长。
const MAX_WRITE_GENERATIONS: usize = 2048;
/// 裁剪后保留的比例，防止在临界值附近反复触发裁剪。
const KEEP_WRITE_GENERATIONS: usize = MAX_WRITE_GENERATIONS * 3 / 4;

/// 单个曲目键的写入代数记录；`order` 为单调递增的写入顺序，用于裁剪最旧的条目。
struct GenerationEntry {
    generation: u64,
    order: u64,
}

/// 带容量上限的写入代数表；`next_order` 为全局单调递增的写入次序。
#[derive(Default)]
struct GenerationTable {
    entries: HashMap<String, GenerationEntry>,
    next_order: u64,
}

impl GenerationTable {
    /// 移除写入次序最旧的条目，直到保留 `KEEP_WRITE_GENERATIONS` 条。
    fn prune_oldest(&mut self) {
        if self.entries.len() <= KEEP_WRITE_GENERATIONS {
            return;
        }
        // 收集属主（而非借用）键，避免迭代期间可变借用 `entries` 与其冲突。
        let mut ordered: Vec<(u64, String)> = self
            .entries
            .iter()
            .map(|(key, value)| (value.order, key.clone()))
            .collect();
        ordered.sort_unstable();
        let remove_count = ordered.len().saturating_sub(KEEP_WRITE_GENERATIONS);
        for (_, key) in ordered.into_iter().take(remove_count) {
            self.entries.remove(&key);
        }
    }
}

/// 缓存命中及其时效状态。
pub struct CacheLookup {
    pub snapshot: LyricsSnapshot,
    /// 条目落盘时已经应用的中文字形目标。
    pub chinese_variant: LyricsChineseVariant,
    /// 保留原来源刷新时间；仅改写字形时不能借机延长缓存有效期。
    pub refreshed_at_seconds: u64,
    /// 本轮读取后已按设置转换并覆盖缓存时，记录转换前字形供解析链路展示。
    pub variant_updated_from: Option<LyricsChineseVariant>,
    /// 缓存归一化时正文是否真的发生字形变化。
    pub variant_text_converted: bool,
    /// 本轮字形转换是否成功持久化；失败时仍可展示内存中的转换结果。
    pub variant_rewrite_succeeded: bool,
    /// “原文”不能从已转换文本无损还原，此时缓存只用于诊断，不能用于展示或候选比较。
    pub requires_original_refresh: bool,
    /// 仍在展示可信期内：可以直接展示，不必阻塞式重解析。
    pub is_fresh: bool,
    /// 命中的是逐行结果：本轮需要静默确认能否升级到逐字（升级成功后缓存变成长期结论）。
    pub needs_revalidation: bool,
}

/// 版本化的解析后歌词文件缓存。
pub struct ParsedLyricsCache {
    cache_path: PathBuf,
    diagnostics: CacheDiagnostics,
    /// 每个键最近一次写入所属的解析代数，用于拒绝被取代的旧写入。
    ///
    /// 为避免长时间切歌时内存单调增长，此表带容量上限：超过 `MAX_WRITE_GENERATIONS`
    /// 后裁剪最旧的写入记录。裁剪只影响用于拒绝过期写入的排序表，不影响磁盘缓存淘汰。
    write_generations: Mutex<GenerationTable>,
}

impl ParsedLyricsCache {
    /// 在应用缓存目录下创建歌词专用版本目录。
    pub fn new(app_cache_dir: &Path) -> Result<Self, std::io::Error> {
        filesystem::ensure_managed_directory(app_cache_dir)?;
        let lyrics_path = app_cache_dir.join("lyrics");
        filesystem::ensure_managed_directory(&lyrics_path)?;
        remove_obsolete_schema_directories(&lyrics_path);
        let cache_path = lyrics_path.join(lyrics_cache_schema_label());
        filesystem::ensure_managed_directory(&cache_path)?;
        migrate_legacy_entries_directory(&cache_path);
        Ok(Self {
            cache_path,
            diagnostics: CacheDiagnostics::default(),
            write_generations: Mutex::new(GenerationTable::default()),
        })
    }

    /// 返回歌词缓存根目录，供数据页查看全部版本目录。
    pub fn directory(&self) -> &Path {
        self.cache_path.parent().unwrap_or(&self.cache_path)
    }

    /// 读取并校验单个缓存条目，损坏条目按未命中处理。
    pub fn load(&self, track_key: &str) -> Option<CacheLookup> {
        let path = self.entry_path(track_key);
        let metadata = filesystem::metadata_without_reparse(&path).ok()?;
        if metadata.len() > MAX_CACHE_ENTRY_BYTES {
            let _ = fs::remove_file(&path);
            self.diagnostics.invalidate();
            return None;
        }
        let content = fs::read(&path).ok()?;
        let entry = decode_entry(&content);
        if let Some(entry) = entry.filter(|entry| {
            entry.snapshot.track_key.as_deref() == Some(track_key)
                && is_cacheable_status(entry.snapshot.status)
        }) {
            let is_fresh = is_fresh(&entry);
            let revalidate = is_fresh && needs_revalidation(&entry);
            self.diagnostics
                .record_current(track_key, metadata.len(), &entry);
            return Some(CacheLookup {
                is_fresh,
                needs_revalidation: revalidate,
                chinese_variant: entry.chinese_variant,
                refreshed_at_seconds: entry.refreshed_at_seconds,
                variant_updated_from: None,
                variant_text_converted: false,
                variant_rewrite_succeeded: false,
                requires_original_refresh: false,
                snapshot: entry.snapshot,
            });
        }
        let _ = fs::remove_file(path);
        self.diagnostics.invalidate();
        None
    }

    /// 写入本次解析结果和刷新时间；损坏缓存可在下次播放时自动重建。
    ///
    /// `generation` 用于排序：同一首歌可能有多轮解析在途（手动刷新、监听事件、切歌），
    /// 代数较小的写入属于已被取代的那一轮，直接丢弃，避免它覆盖同键的较新结果——
    /// 否则下次启动会读回旧内容，而内存里展示的是新内容。
    pub fn store(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
    ) -> Result<(), LyricsError> {
        self.store_at(snapshot, chinese_variant, generation, now_seconds())
    }

    /// 原位改写同一首歌的缓存字形，并保留歌词来源原本的刷新时间。
    pub fn rewrite_variant(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
        refreshed_at_seconds: u64,
    ) -> Result<(), LyricsError> {
        self.store_at(snapshot, chinese_variant, generation, refreshed_at_seconds)
    }

    fn store_at(
        &self,
        snapshot: &LyricsSnapshot,
        chinese_variant: LyricsChineseVariant,
        generation: u64,
        refreshed_at_seconds: u64,
    ) -> Result<(), LyricsError> {
        let Some(track_key) = snapshot.track_key.as_deref() else {
            return Ok(());
        };
        // 瞬时未命中不持久化；真实歌词、纯音乐与“没有歌词”都可跨播放复用。
        if !is_cacheable_status(snapshot.status) {
            return Ok(());
        }
        if !self.reserve_write(track_key, generation) {
            return Ok(());
        }
        ensure_directory_boundary(&self.cache_path)?;
        let target = self.entry_path(track_key);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary =
            self.cache_path
                .join(format!(".{track_key}.{}.{}.tmp", std::process::id(), nonce));
        let cache_entry = CacheEntry {
            refreshed_at_seconds,
            chinese_variant,
            snapshot: snapshot.clone(),
        };
        let content = encode_entry(&cache_entry)?;
        let replaced_bytes = match filesystem::metadata_if_exists(&target)? {
            Some(metadata) if metadata.is_file() => Some(metadata.len()),
            Some(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("歌词缓存目标不是普通文件: {}", target.display()),
                )
                .into());
            }
            None => None,
        };
        let content_bytes = content.len() as u64;
        let write_result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?
            .write_all(&content);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        match replace_file(&temporary, &target) {
            Ok(()) => match prune_after_write(
                &self.cache_path,
                self.diagnostics.tracked_totals(),
                &target,
                content_bytes,
                replaced_bytes,
            ) {
                Ok(totals) => {
                    self.diagnostics
                        .record_stored(track_key, content_bytes, &cache_entry, totals);
                    Ok(())
                }
                Err(error) => {
                    self.diagnostics.invalidate();
                    Err(error.into())
                }
            },
            Err(error) => {
                let _ = fs::remove_file(temporary);
                self.diagnostics.invalidate();
                Err(error.into())
            }
        }
    }

    /// 删除当前歌曲的解析结果，使播放器源文件变化后只重建受影响条目。
    pub fn remove(&self, track_key: &str) -> Result<(), std::io::Error> {
        ensure_directory_boundary(&self.cache_path)?;
        let path = self.entry_path(track_key);
        let Some(metadata) = filesystem::metadata_if_exists(&path)? else {
            self.diagnostics.record_removed(track_key, None);
            return Ok(());
        };
        if !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("歌词缓存目标不是普通文件: {}", path.display()),
            ));
        }
        let removed_bytes = Some(metadata.len());
        match fs::remove_file(path) {
            Ok(()) => {
                self.diagnostics.record_removed(track_key, removed_bytes);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.diagnostics.record_removed(track_key, None);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// 记录本次写入的代数；已有更大代数的写入时返回 false，表示本轮已被取代。
    fn reserve_write(&self, track_key: &str, generation: u64) -> bool {
        let Ok(mut table) = self.write_generations.lock() else {
            // 锁不可用时按允许写入处理：宁可偶发乱序，也不要静默丢掉歌词。
            return true;
        };
        if table
            .entries
            .get(track_key)
            .is_some_and(|written| written.generation > generation)
        {
            return false;
        }
        table.next_order += 1;
        let order = table.next_order;
        table
            .entries
            .insert(track_key.to_owned(), GenerationEntry { generation, order });
        if table.entries.len() > MAX_WRITE_GENERATIONS {
            table.prune_oldest();
        }
        true
    }

    /// 清空全部规范化歌词缓存，同时保留版本目录供后续写入复用。
    pub fn clear(&self) -> Result<(), std::io::Error> {
        ensure_directory_boundary(&self.cache_path)?;
        for entry in fs::read_dir(&self.cache_path)? {
            let entry = entry?;
            // 云同步占位符等特殊条目取不到元数据：跳过它，不能让单个条目让整次清理失败。
            let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
                continue;
            };
            if metadata.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        self.diagnostics.invalidate();
        Ok(())
    }

    /// 只删除依赖指定播放器本地目录的结果，在线结果与其他播放器缓存继续保留。
    pub fn clear_local_source(&self, player: MediaPlayer) -> Result<(), std::io::Error> {
        let result = (|| {
            ensure_directory_boundary(&self.cache_path)?;
            for entry in fs::read_dir(&self.cache_path)? {
                let entry = entry?;
                let Ok(metadata) = filesystem::entry_metadata_without_reparse(&entry) else {
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                let path = entry.path();
                let should_remove = fs::read(&path)
                    .ok()
                    .and_then(|content| decode_entry(&content))
                    .is_some_and(|entry| {
                        entry.snapshot.source.is_some_and(|source| {
                            source.player == player && source.kind == LyricsSourceKind::Local
                        })
                    });
                if should_remove {
                    fs::remove_file(path)?;
                }
            }
            Ok(())
        })();
        self.diagnostics.invalidate();
        result
    }

    /// 读取缓存占用和当前歌曲条目状态；文件未变化时复用轻量索引。
    pub fn diagnostics(&self, track_key: Option<&str>) -> LyricsCacheDiagnostics {
        self.diagnostics.snapshot(&self.cache_path, track_key)
    }

    fn entry_path(&self, track_key: &str) -> PathBuf {
        self.cache_path.join(format!("{track_key}.bin"))
    }
}
