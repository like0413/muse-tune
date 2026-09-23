//! 对外缓存清理与手动刷新操作。

use std::{io, sync::atomic::Ordering};

use crate::{error::Error, lyrics::track::TrackDescriptor};

use super::LyricsService;

impl LyricsService {
    /// 清空应用管理的歌词缓存，并通知诊断页刷新缓存状态。
    pub fn clear_cache(&self) -> Result<(), io::Error> {
        if let Ok(_guard) = self.inner.current_track.lock() {
            self.inner.generation.fetch_add(1, Ordering::AcqRel);
        }
        self.cancel_resolution();
        self.inner.cache.clear()?;
        (self.inner.diagnostics_notifier)();
        Ok(())
    }

    /// 只删除当前歌曲的应用歌词缓存，不改变正在展示的歌词快照。
    pub fn clear_current_cache(&self) -> Result<(), Error> {
        let (track, _) = self.prepare_current_cache_mutation()?;
        self.cancel_resolution();
        self.inner
            .cache
            .remove(&track.key)
            .map_err(|error| Error::Message(format!("清理当前歌曲缓存失败: {error}")))?;
        (self.inner.diagnostics_notifier)();
        Ok(())
    }

    /// 删除当前歌曲缓存并立即启动一次完整解析。
    pub fn refresh_current(&self) -> Result<(), Error> {
        let (track, generation) = self.prepare_current_cache_mutation()?;
        self.cancel_resolution();
        self.inner
            .cache
            .remove(&track.key)
            .map_err(|error| Error::Message(format!("清理当前歌曲缓存失败: {error}")))?;
        (self.inner.diagnostics_notifier)();
        self.start_resolution(Some(track), generation, false, true);
        Ok(())
    }

    /// 在当前歌曲锁内提升解析代数，确保旧任务不能在删除之后回写缓存。
    fn prepare_current_cache_mutation(&self) -> Result<(TrackDescriptor, u64), Error> {
        let current = self
            .inner
            .current_track
            .lock()
            .map_err(|_| Error::Message("当前歌曲状态不可用".to_owned()))?;
        let track = current
            .clone()
            .ok_or_else(|| Error::Message("当前没有可清理的歌曲".to_owned()))?;
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        Ok((track, generation))
    }
}
