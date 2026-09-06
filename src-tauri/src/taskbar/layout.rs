//! 管理任务栏布局候选值的稳定确认，避免提交动画中的中间矩形。

use std::time::{Duration, Instant};

use super::geometry::ScreenRect;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(120);
const FAILURE_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const REQUIRED_STABLE_SAMPLES: u8 = 2;

/// 一次完整的播放器布局测量结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BarLayout {
    pub(super) window_rect: ScreenRect,
    pub(super) visible_rect: ScreenRect,
}

/// 对事件触发的候选布局限频采样，并仅提交连续稳定的结果。
pub(super) struct LayoutStabilizer {
    pending: Option<BarLayout>,
    matching_samples: u8,
    invalidated_since_sample: bool,
    next_sample_at: Option<Instant>,
}

impl LayoutStabilizer {
    /// 创建尚未等待任何布局样本的状态机。
    pub(super) const fn new() -> Self {
        Self {
            pending: None,
            matching_samples: 0,
            invalidated_since_sample: false,
            next_sample_at: None,
        }
    }

    /// 标记布局已失效；采样周期已经启动时不被密集事件反复提前。
    pub(super) fn invalidate(&mut self, now: Instant) {
        if self.next_sample_at.is_some() {
            self.invalidated_since_sample = true;
        } else {
            self.next_sample_at = Some(now);
        }
    }

    /// 判断当前是否应读取一次新的任务栏几何。
    pub(super) fn sample_due(&self, now: Instant) -> bool {
        self.next_sample_at.is_some_and(|deadline| now >= deadline)
    }

    /// 记录成功样本；相同候选连续出现两次后返回可提交布局。
    pub(super) fn observe(&mut self, candidate: BarLayout, now: Instant) -> Option<BarLayout> {
        if self.pending == Some(candidate) && !self.invalidated_since_sample {
            self.matching_samples = self.matching_samples.saturating_add(1);
        } else {
            self.pending = Some(candidate);
            self.matching_samples = 1;
        }
        self.invalidated_since_sample = false;

        if self.matching_samples >= REQUIRED_STABLE_SAMPLES {
            self.reset();
            Some(candidate)
        } else {
            self.next_sample_at = Some(now + SAMPLE_INTERVAL);
            None
        }
    }

    /// 查询失败不产生空布局，只延后重试并保留已应用矩形。
    pub(super) fn retry_after_failure(&mut self, now: Instant) {
        self.next_sample_at = Some(now + FAILURE_RETRY_INTERVAL);
    }

    /// 返回下一次内部采样的截止时间。
    pub(super) const fn next_sample_at(&self) -> Option<Instant> {
        self.next_sample_at
    }

    /// 清除旧任务栏或旧策略留下的候选状态。
    pub(super) fn reset(&mut self) {
        self.pending = None;
        self.matching_samples = 0;
        self.invalidated_since_sample = false;
        self.next_sample_at = None;
    }
}
