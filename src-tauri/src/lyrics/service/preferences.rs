//! 歌词偏好更新及其对当前解析生命周期的影响。

use std::sync::atomic::Ordering;

use crate::{
    error::Error,
    lyrics::model::{LyricsChineseVariant, LyricsOnlineStrategy, LyricsSnapshot},
    media::MediaPlayer,
};

use super::{LyricsPreferences, LyricsService};

impl LyricsService {
    /// 原子更新歌词开关、中文输出目标、联网能力、在线调度策略与在线接口集合。
    pub fn set_preferences(
        &self,
        enabled: bool,
        chinese_variant: LyricsChineseVariant,
        allow_online: bool,
        online_strategy: LyricsOnlineStrategy,
        online_sources: Vec<MediaPlayer>,
    ) -> Result<(), Error> {
        let next = LyricsPreferences {
            enabled,
            chinese_variant,
            allow_online,
            online_strategy,
            online_sources: online_sources.clone(),
        };
        let previous = {
            let mut current = self
                .inner
                .preferences
                .write()
                .map_err(|_| Error::Message("歌词偏好状态不可用".to_owned()))?;
            if *current == next {
                return Ok(());
            }
            let previous = current.clone();
            *current = next;
            previous
        };
        let enabled_changed = previous.enabled != enabled;
        let chinese_variant_changed = previous.chinese_variant != chinese_variant;
        let online_changed = previous.allow_online != allow_online;
        if !enabled {
            let (track_key, generation) = {
                let current = self
                    .inner
                    .current_track
                    .lock()
                    .map_err(|_| Error::Message("当前歌曲状态不可用".to_owned()))?;
                let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
                (current.as_ref().map(|track| track.key.clone()), generation)
            };
            self.cancel_resolution();
            self.release_watchers();
            self.publish_if_current(
                LyricsSnapshot::unavailable(track_key, "歌词显示已关闭"),
                generation,
            );
            return Ok(());
        }
        if enabled_changed {
            self.refresh_watchers();
            self.start_registry_watcher();
        }
        // 策略与接口集合只影响后续选源；能力或字形改变才重跑当前歌曲。
        if enabled_changed || online_changed || chinese_variant_changed {
            self.force_resolve_current()
        } else {
            (self.inner.diagnostics_notifier)();
            Ok(())
        }
    }

    /// 读取一致的歌词偏好快照；锁损坏时回退到兼容旧版本的默认值。
    pub(super) fn preferences(&self) -> LyricsPreferences {
        self.inner.preferences.read().map_or_else(
            |_| LyricsPreferences::default(),
            |preferences| preferences.clone(),
        )
    }

    /// 歌词总开关是否开启；关闭后所有后台歌词工作都应停止。
    pub(super) fn lyrics_enabled(&self) -> bool {
        self.preferences().enabled
    }

    /// 用最新偏好重新解析当前歌曲，并在解析期间保留已显示的歌词。
    fn force_resolve_current(&self) -> Result<(), Error> {
        let (current, generation) = {
            let current = self
                .inner
                .current_track
                .lock()
                .map_err(|_| Error::Message("当前歌曲状态不可用".to_owned()))?;
            let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
            (current.clone(), generation)
        };
        self.start_resolution(current, generation, true, false);
        Ok(())
    }
}
