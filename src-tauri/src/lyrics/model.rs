use crate::media::MediaPlayer;

use super::track::normalize_text;

const MIN_WORD_TIMING_COVERAGE_PERCENT: usize = 80;

/// 歌词解析生命周期；技术错误与“确实无歌词”保持可区分。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsStatus {
    Loading,
    Ready,
    /// 平台已明确声明当前歌曲为纯音乐，不属于歌词时间轴。
    Instrumental,
    /// 平台已明确声明当前歌曲没有歌词；与纯音乐、技术失败都是不同结论。
    NoLyrics,
    #[default]
    Unavailable,
    Error,
}

/// 歌词时间精度。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsPrecision {
    Word,
    Line,
}

/// 歌词来自播放器缓存还是国内在线接口。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSourceKind {
    Local,
    Online,
}

/// 当前歌词在本次播放中的实际取得方式，与歌词的原始平台来源分开记录。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionMethod {
    #[default]
    None,
    ApplicationCache,
    PlayerLocal,
    Online,
}

/// 最近一次歌词解析步骤的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionOutcome {
    Hit,
    Miss,
    Error,
}

/// 在线歌词来源的调度策略。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsOnlineStrategy {
    /// 并行查询用户勾选的在线接口，按用户排定的顺序发起。
    #[default]
    Parallel,
    /// 只查询当前正在播放的平台自己的在线接口，不使用其他平台兜底。
    CurrentPlayerOnly,
}

/// 解析步骤的来源标识。
///
/// 这里只输出稳定的机器键，展示文案由前端按语言组装：Rust 直接产出中文会让界面文案无法
/// 跟随语言设置，也让前端只能靠字符串相等来判断步骤含义，改动文案即静默失效。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsResolutionSite {
    /// 应用自身的解析结果缓存。
    ApplicationCache,
    /// 当前播放器的本地歌词。
    Local,
    /// 当前播放器的在线歌词。
    Online,
    /// 备用平台的在线歌词。
    OnlineFallback,
    /// 已有可展示缓存时在后台尝试的本地精度升级。
    LocalUpgrade,
}

/// 最近一次解析的有界步骤记录，仅保留诊断所需摘要。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionStep {
    pub site: LyricsResolutionSite,
    pub outcome: LyricsResolutionOutcome,
    pub detail: Option<String>,
    /// 该步骤属于并发查询多个来源的阶段：同阶段的步骤是同时执行的，不能显示成先后顺序。
    pub parallel: bool,
}

/// Muse Tune 规范化歌词缓存的磁盘状态。
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCacheDiagnostics {
    pub schema_version: String,
    pub entry_count: usize,
    pub total_bytes: u64,
    pub limit_bytes: u64,
    pub current_entry_exists: bool,
    pub current_entry_bytes: Option<u64>,
    pub current_entry_age_seconds: Option<u64>,
    pub current_entry_fresh: Option<bool>,
    pub current_refresh_remaining_seconds: Option<u64>,
}

/// 单个播放器歌词适配器的自动发现与监听状态。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsAdapterDiagnostics {
    pub player: MediaPlayer,
    pub cache_path: Option<String>,
    pub cache_path_available: bool,
    pub watcher_active: bool,
}

/// 可展示且可诊断的歌词来源。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSource {
    pub player: MediaPlayer,
    pub kind: LyricsSourceKind,
    pub song_id: Option<String>,
}

/// 单个逐字片段的绝对时间范围。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// 统一后的单行歌词。
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub words: Vec<LyricWord>,
}

/// 仅把绝大多数正文行都有完整时间轴的规范化歌词视为逐字结果。
pub fn has_word_timing(lines: &[LyricLine]) -> bool {
    let mut eligible_count = 0usize;
    let mut timed_count = 0usize;
    // 逐行拼接复用同一缓冲，避免为每一行单独分配字符串。
    let mut timed_text = String::new();
    for line in lines {
        let normalized = normalize_text(&line.text);
        if normalized.is_empty() {
            continue;
        }
        eligible_count += 1;
        timed_text.clear();
        for word in &line.words {
            if word.end_ms > word.start_ms && !word.text.trim().is_empty() {
                timed_text.push_str(&word.text);
            }
        }
        if !timed_text.is_empty() && normalize_text(&timed_text) == normalized {
            timed_count += 1;
        }
    }
    eligible_count > 0 && timed_count * 100 >= eligible_count * MIN_WORD_TIMING_COVERAGE_PERCENT
}

/// 独立于媒体快照广播的歌词状态。
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSnapshot {
    pub track_key: Option<String>,
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub lines: Vec<LyricLine>,
    pub error_reason: Option<String>,
}

/// 设置页只读展示的歌词运行状态，不开放路径覆盖能力。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDiagnostics {
    pub snapshot: LyricsSnapshotDiagnostics,
    pub current_player: Option<MediaPlayer>,
    pub enabled: bool,
    pub online_strategy: LyricsOnlineStrategy,
    pub resolution_method: LyricsResolutionMethod,
    pub local_cache_path: Option<String>,
    pub local_cache_available: bool,
    pub resolver_running: bool,
    pub pending_resolution: bool,
    pub resolution_duration_ms: Option<u64>,
    /// 本轮解析使用的曲目信息。切歌瞬间媒体会话可能给出混搭快照（标题已换、时长未换等），
    /// 匹配是按标题、艺术家、时长做的，三处同时未命中时只能靠这一项定位。
    pub resolution_track: Option<LyricsResolutionTrack>,
    pub resolution_steps: Vec<LyricsResolutionStep>,
    /// 上一轮已完成的解析；有新一轮开始时归档，只保留一条。缓存命中那轮会短路整条链路，
    /// 排障时需要它来对照上一条链路的变化。
    pub recent_resolutions: Vec<LyricsResolutionRecord>,
    pub cache: LyricsCacheDiagnostics,
    pub adapters: Vec<LyricsAdapterDiagnostics>,
}

/// 一次已结束解析的完整记录：尝试过哪些来源，以及最终结论。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionRecord {
    pub finished_at_seconds: Option<u64>,
    pub duration_ms: Option<u64>,
    pub track: Option<LyricsResolutionTrack>,
    pub steps: Vec<LyricsResolutionStep>,
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub error_reason: Option<String>,
}

/// 一轮解析所使用的曲目标识字段；与来源匹配依据保持一致。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResolutionTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

/// 歌词诊断只传递摘要，避免正文和逐字数组进入 IPC。
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSnapshotDiagnostics {
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub line_count: usize,
    pub error_reason: Option<String>,
}

impl LyricsSnapshot {
    /// 创建不携带旧歌词的加载状态。
    pub fn loading(track_key: String) -> Self {
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Loading,
            ..Self::default()
        }
    }

    /// 创建已解析状态；平台占位文案转换为不携带时间轴的语义状态。
    pub fn from_resolved(track_key: String, resolved: ResolvedLyrics) -> Self {
        let ResolvedLyrics { source, lines } = resolved;
        if let Some(notice) = platform_notice(&lines) {
            return Self {
                track_key: Some(track_key),
                status: notice.status(),
                source: Some(source),
                ..Self::default()
            };
        }
        let precision = if has_word_timing(&lines) {
            LyricsPrecision::Word
        } else {
            LyricsPrecision::Line
        };
        Self {
            track_key: Some(track_key),
            status: LyricsStatus::Ready,
            source: Some(source),
            precision: Some(precision),
            lines,
            error_reason: None,
        }
    }

    /// 创建当前歌曲没有可靠歌词的可恢复状态。
    pub fn unavailable(track_key: Option<String>, reason: impl Into<String>) -> Self {
        Self {
            track_key,
            error_reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

/// 播放器适配器返回的内部统一结果。
#[derive(Clone, Debug)]
pub struct ResolvedLyrics {
    pub source: LyricsSource,
    pub lines: Vec<LyricLine>,
}

/// 歌词适配器对协调器返回的统一业务结果；技术失败继续由 `LyricsError` 表达。
#[derive(Debug)]
pub enum LyricsLookupOutcome {
    /// 当前适配器不具备所请求的能力。
    Unsupported,
    /// 适配器具备能力，但本次没有找到可靠歌词。
    Miss(LyricsLookupMiss),
    /// 找到可进入统一质量校验的歌词候选。
    Hit(ResolvedLyrics),
}

/// 正常未命中的稳定分类，供编排和诊断使用，不承载平台私有细节。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LyricsLookupMiss {
    /// 解析所需的播放器缓存、索引或歌曲元数据尚不可用。
    DataUnavailable,
    /// 适配器已执行，但没有产出通过其内部规则的可靠歌词。
    NoReliableLyrics,
}

/// 平台占位文案的长度上限；超过这个长度不可能是否认歌词的固定说明。
const PLATFORM_NOTICE_MAX_CHARS: usize = 40;
/// 平台占位文案的行数上限；真实歌词不会只有寥寥数行。
const PLATFORM_NOTICE_MAX_LINES: usize = 3;
/// 一句完整说明的常见主语，如“此歌曲为没有填词的纯音乐，请您欣赏”。
const NOTICE_SUBJECTS: [&str; 4] = ["此歌曲", "该歌曲", "本歌曲", "这首歌"];

/// 平台占位文案的类型；两者都不是歌词正文，但结论与有效期不同。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformNotice {
    Instrumental,
    NoLyrics,
}

impl PlatformNotice {
    /// 占位文案对应的歌曲状态。
    pub(super) const fn status(self) -> LyricsStatus {
        match self {
            Self::Instrumental => LyricsStatus::Instrumental,
            Self::NoLyrics => LyricsStatus::NoLyrics,
        }
    }
}

/// 识别平台给出的占位文案，并区分“纯音乐”与“没有歌词”。
///
/// 逐个枚举历史文案（“此歌曲为没有填词的纯音乐请您欣赏”“纯音乐请欣赏”……）必然漏判：
/// 平台换一个说法就会被当成真实歌词并长期缓存。因此改为按形态与关键词识别，并同时限制
/// 行数与长度，避免把普通歌词里出现的“纯音乐”“没有歌词”误判成语义状态。
pub(super) fn platform_notice(lines: &[LyricLine]) -> Option<PlatformNotice> {
    if lines.is_empty() || lines.len() > PLATFORM_NOTICE_MAX_LINES {
        return None;
    }
    lines.iter().find_map(|line| notice_kind(&line.text))
}

/// 从没有时间轴的原始歌词文本中找出平台占位文案。
///
/// QQ 音乐给纯音乐曲目返回的整段歌词就是一句“此歌曲为没有填词的纯音乐，请您欣赏”，
/// 一个时间戳都没有；逐行解析只会把它整段丢掉，于是这首歌既不显示歌词、也判不出纯音乐。
/// 这里在原始文本上剥离方括号标记后按同一套形态规则再识别一次，行数与长度限制保持不变。
pub(super) fn notice_text_without_timeline(input: &str) -> Option<String> {
    let texts = input
        .lines()
        .map(strip_bracket_segments)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if texts.is_empty() || texts.len() > PLATFORM_NOTICE_MAX_LINES {
        return None;
    }
    texts.into_iter().find(|text| notice_kind(text).is_some())
}

/// 去掉方括号内的标记（时间轴、"纯音乐"以外的 `[ti:]` 等元数据），只留正文。
fn strip_bracket_segments(line: &str) -> String {
    let mut text = String::new();
    let mut inside_bracket = false;
    for character in line.chars() {
        match character {
            '[' => inside_bracket = true,
            ']' => inside_bracket = false,
            _ if !inside_bracket => text.push(character),
            _ => {}
        }
    }
    text.trim().to_owned()
}

fn notice_kind(text: &str) -> Option<PlatformNotice> {
    let normalized = normalize_text(text);
    if normalized.is_empty() || normalized.chars().count() > PLATFORM_NOTICE_MAX_CHARS {
        return None;
    }
    // 形态一：直接以“纯音乐”开头，如“纯音乐请欣赏”“纯音乐，请您欣赏”。
    if normalized.starts_with("纯音乐") {
        return Some(PlatformNotice::Instrumental);
    }
    // 形态二：一句完整说明，如“此歌曲为没有填词的纯音乐，请您欣赏”“该歌曲暂无歌词”。
    if !NOTICE_SUBJECTS
        .iter()
        .any(|subject| normalized.starts_with(subject))
    {
        return None;
    }
    if normalized.contains("纯音乐") {
        return Some(PlatformNotice::Instrumental);
    }
    ["无歌词", "没有歌词", "暂无歌词"]
        .iter()
        .any(|marker| normalized.contains(marker))
        .then_some(PlatformNotice::NoLyrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> LyricLine {
        LyricLine {
            start_ms: 0,
            end_ms: 1_000,
            text: text.to_owned(),
            translation: None,
            romanization: None,
            words: Vec::new(),
        }
    }

    fn word_line(text: &str) -> LyricLine {
        LyricLine {
            words: vec![LyricWord {
                start_ms: 0,
                end_ms: 1_000,
                text: text.to_owned(),
            }],
            ..line(text)
        }
    }

    /// 平台换一种说法就会被当成真实歌词并长期缓存，所以识别必须按形态而不是枚举历史文案。
    #[test]
    fn platform_notice_recognizes_both_conclusions() {
        assert_eq!(
            platform_notice(&[line("纯音乐请欣赏")]),
            Some(PlatformNotice::Instrumental)
        );
        assert_eq!(
            platform_notice(&[line("此歌曲为没有填词的纯音乐，请您欣赏")]),
            Some(PlatformNotice::Instrumental)
        );
        assert_eq!(
            platform_notice(&[line("该歌曲暂无歌词")]),
            Some(PlatformNotice::NoLyrics)
        );
        assert_eq!(
            platform_notice(&[line("这首歌没有歌词")]),
            Some(PlatformNotice::NoLyrics)
        );
    }

    #[test]
    fn platform_notice_maps_to_matching_status() {
        assert_eq!(
            PlatformNotice::Instrumental.status(),
            LyricsStatus::Instrumental
        );
        assert_eq!(PlatformNotice::NoLyrics.status(), LyricsStatus::NoLyrics);
    }

    /// 真实歌词里出现"纯音乐"三个字不应被误判：占位文案必须同时满足主语形态与长度限制。
    #[test]
    fn notice_detection_requires_notice_shape() {
        assert_eq!(platform_notice(&[line("我喜欢这首纯音乐作品")]), None);
        assert_eq!(platform_notice(&[line("此歌曲很好听")]), None);
    }

    #[test]
    fn notice_detection_rejects_long_and_numerous_lines() {
        let long = format!("纯音乐{}", "啊".repeat(PLATFORM_NOTICE_MAX_CHARS));
        assert_eq!(platform_notice(&[line(&long)]), None);

        let many = vec![line("纯音乐"); PLATFORM_NOTICE_MAX_LINES + 1];
        assert_eq!(platform_notice(&many), None);
    }

    #[test]
    fn notice_detection_ignores_empty_input() {
        assert_eq!(platform_notice(&[]), None);
    }

    /// 整段占位文案可能一个时间戳都没有（QQ 音乐纯音乐曲目就是如此），
    /// 必须在解析丢行之前先在原始文本上识别出来。
    #[test]
    fn notice_detection_works_without_timeline() {
        assert_eq!(
            notice_text_without_timeline("此歌曲为没有填词的纯音乐，请您欣赏"),
            Some("此歌曲为没有填词的纯音乐，请您欣赏".to_owned())
        );
        assert_eq!(
            notice_text_without_timeline("[00:00.00]纯音乐，请欣赏"),
            Some("纯音乐，请欣赏".to_owned())
        );
        assert_eq!(notice_text_without_timeline("让我们一起摇摆"), None);
        assert_eq!(notice_text_without_timeline(""), None);
    }

    /// 端到端：QQ 音乐那种整段没有时间戳的占位文案必须落成 instrumental，
    /// 而不是因为"解析不出歌词行"被判成不可用。
    #[test]
    fn untimed_notice_becomes_instrumental_snapshot() {
        let lines = crate::lyrics::parser::parse_lrc_lines("此歌曲为没有填词的纯音乐，请您欣赏")
            .expect("解析不应失败");
        let snapshot = LyricsSnapshot::from_resolved(
            "track-key".to_owned(),
            ResolvedLyrics {
                source: LyricsSource {
                    player: MediaPlayer::QqMusic,
                    kind: LyricsSourceKind::Online,
                    song_id: None,
                },
                lines,
            },
        );
        assert_eq!(snapshot.status, LyricsStatus::Instrumental);
        assert!(snapshot.lines.is_empty());
    }

    /// 只有绝大多数行都有完整逐字覆盖才算逐字歌词，否则前端高亮会大面积缺失。
    #[test]
    fn word_timing_requires_high_coverage() {
        assert!(has_word_timing(&[word_line("第一句"), word_line("第二句")]));

        // 5 行中 4 行有逐字 = 80%，正好达到阈值。
        assert!(has_word_timing(&[
            word_line("第一句"),
            word_line("第二句"),
            word_line("第三句"),
            word_line("第四句"),
            line("第五句"),
        ]));

        assert!(!has_word_timing(&[
            word_line("第一句"),
            word_line("第二句"),
            word_line("第三句"),
            line("第四句"),
            line("第五句"),
        ]));
    }

    #[test]
    fn word_timing_ignores_empty_lines() {
        // 空文本行不计入分母，否则间奏多的歌词永远达不到阈值。
        assert!(has_word_timing(&[word_line("第一句"), line("   ")]));
        assert!(!has_word_timing(&[]));
    }

    /// 单词文本拼起来与整行不一致时不算覆盖，避免错位的逐字数据被当成逐字歌词。
    #[test]
    fn word_timing_requires_matching_text() {
        let mismatched = LyricLine {
            words: vec![LyricWord {
                start_ms: 0,
                end_ms: 1_000,
                text: "别的".to_owned(),
            }],
            ..line("第一句")
        };
        assert!(!has_word_timing(&[mismatched]));
    }
}
