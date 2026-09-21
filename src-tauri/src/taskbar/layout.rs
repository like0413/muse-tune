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

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个左边界不同的布局候选；测试只关心两次采样是否为同一个候选。
    fn layout(left: i32) -> BarLayout {
        let rect = ScreenRect {
            left,
            top: 1080,
            right: left + 250,
            bottom: 1128,
        };
        BarLayout {
            window_rect: rect,
            visible_rect: rect,
        }
    }

    /// 连续两次读到同一候选才提交：否则任务栏动画中的中间矩形会被当成最终位置。
    #[test]
    fn layout_is_submitted_after_two_matching_samples() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();

        assert!(stabilizer.observe(layout(0), now).is_none());
        assert_eq!(
            stabilizer.observe(layout(0), now + SAMPLE_INTERVAL),
            Some(layout(0))
        );
    }

    /// 候选变化必须重新计数，否则两次不同的采样会被误判为已经稳定。
    #[test]
    fn changing_candidate_restarts_the_sample_count() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();

        assert!(stabilizer.observe(layout(0), now).is_none());
        assert!(
            stabilizer
                .observe(layout(40), now + SAMPLE_INTERVAL)
                .is_none()
        );
        assert_eq!(
            stabilizer.observe(layout(40), now + SAMPLE_INTERVAL * 2),
            Some(layout(40))
        );
    }

    /// 采样周期内出现的失效事件不能让下一次重复采样直接提交旧候选。
    #[test]
    fn invalidation_discards_the_next_repeat() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();

        assert!(stabilizer.observe(layout(0), now).is_none());
        stabilizer.invalidate(now);
        assert!(stabilizer.observe(layout(0), now).is_none());
        assert!(stabilizer.observe(layout(0), now).is_some());
    }

    /// 截止时间按固定采样间隔推进，调用方依赖它决定等待多久。
    #[test]
    fn sample_deadline_follows_the_sample_interval() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();
        stabilizer.observe(layout(0), now);

        assert!(!stabilizer.sample_due(now + SAMPLE_INTERVAL - Duration::from_millis(1)));
        assert!(stabilizer.sample_due(now + SAMPLE_INTERVAL));
    }

    /// 查询失败只延后重试，不能提交空布局或清掉已经应用的矩形。
    #[test]
    fn failure_defers_the_next_sample() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();
        stabilizer.retry_after_failure(now);

        assert!(!stabilizer.sample_due(now + FAILURE_RETRY_INTERVAL - Duration::from_millis(1)));
        assert!(stabilizer.sample_due(now + FAILURE_RETRY_INTERVAL));
    }

    /// 重置后旧候选与截止时间一并清除，稳定次数需要重新累计。
    #[test]
    fn reset_clears_pending_state() {
        let mut stabilizer = LayoutStabilizer::new();
        let now = Instant::now();
        stabilizer.observe(layout(0), now);
        stabilizer.reset();

        assert_eq!(stabilizer.next_sample_at(), None);
        assert!(!stabilizer.sample_due(now + SAMPLE_INTERVAL));
        assert!(stabilizer.observe(layout(0), now).is_none());
    }
}
