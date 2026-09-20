use reqwest::{blocking::Client, header::USER_AGENT};
use serde::Deserialize;

use crate::media::MediaPlayer;

use super::super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    parser::parse_krc_lines,
    track::{TrackDescriptor, split_artists},
};

/// 歌词候选搜索；实测只要 `keyword` + `duration` 即可，无需 hash 或 album_audio_id。
const SEARCH_ENDPOINT: &str = "https://krcs.kugou.com/search";
/// KRC 下载；返回内容为 base64 编码的 KRC 文件。
const DOWNLOAD_ENDPOINT: &str = "https://lyrics2.kugou.com/download";

#[derive(Deserialize)]
struct KugouSearchResponse {
    #[serde(default)]
    candidates: Vec<KugouCandidate>,
}

#[derive(Deserialize)]
struct KugouCandidate {
    id: String,
    accesskey: String,
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    singer: Option<String>,
    #[serde(default)]
    duration: Option<u64>,
    /// 酷狗自己的候选权重，仅在我们打分相同时用于决定先后。
    #[serde(default)]
    score: u32,
}

#[derive(Deserialize)]
struct KugouDownloadResponse {
    #[serde(default)]
    status: Option<u32>,
    #[serde(default)]
    content: Option<String>,
}

/// 搜索酷狗歌词候选，按匹配度逐个下载解密，返回第一个可用结果。
pub(super) fn resolve(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    // 时长是酷狗搜索的必需参数，同时也是我们判别候选的主要依据之一。
    let Some(duration_ms) = track.duration_ms else {
        return Ok(None);
    };
    let keyword = format!("{} {}", track.title, track.artists.join(" "));
    let duration = duration_ms.to_string();
    let response = parse_json::<KugouSearchResponse>(
        deadline
            .apply(
                client
                    .get(SEARCH_ENDPOINT)
                    .header(USER_AGENT, API_USER_AGENT)
                    .query(&[
                        ("ver", "1"),
                        ("man", "yes"),
                        ("client", "pc"),
                        ("keyword", keyword.as_str()),
                        ("duration", duration.as_str()),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let mut candidates = response
        .candidates
        .into_iter()
        .filter_map(|candidate| {
            let artists = split_artists(candidate.singer.as_deref().unwrap_or_default());
            let score = accepted_score(
                track,
                SongCandidate {
                    title: candidate.song.as_deref().unwrap_or_default(),
                    artists: &artists,
                    duration_ms: candidate.duration,
                },
            )?;
            Some((score, candidate))
        })
        .collect::<Vec<_>>();
    // 与 QQ 一致：按匹配分从高到低逐个尝试，最高分候选取不到歌词时继续试次优候选。
    candidates.sort_by_key(|(score, candidate)| {
        (
            std::cmp::Reverse(*score),
            std::cmp::Reverse(candidate.score),
        )
    });

    for (_, candidate) in candidates {
        if let Some(lyrics) = fetch_word_lyrics(client, &candidate, deadline)? {
            return Ok(Some(lyrics));
        }
    }
    Ok(None)
}

/// 下载并解密单个候选的 KRC 逐字歌词。
fn fetch_word_lyrics(
    client: &Client,
    candidate: &KugouCandidate,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let response = parse_json::<KugouDownloadResponse>(
        deadline
            .apply(
                client
                    .get(DOWNLOAD_ENDPOINT)
                    .header(USER_AGENT, API_USER_AGENT)
                    .query(&[
                        ("ver", "1"),
                        ("client", "pc"),
                        ("id", candidate.id.as_str()),
                        ("accesskey", candidate.accesskey.as_str()),
                        ("fmt", "krc"),
                        ("charset", "utf8"),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    if response.status != Some(200) {
        return Ok(None);
    }
    let Some(content) = response.content else {
        return Ok(None);
    };
    // 真实 KRC 载荷只有几十 KB。这里先卡住体量：`decrypt_lyrics` 内部按 zlib 最大压缩比
    // 可膨胀上千倍，而且没有输出上限（真正的上限需要上游支持），只能先把爆炸半径压到最小。
    const MAX_ENCRYPTED_BYTES: usize = 768 * 1024;
    if content.len() > MAX_ENCRYPTED_BYTES {
        log::warn!("酷狗在线歌词体量异常，已忽略: {} 字节", content.len());
        return Ok(None);
    }
    // `decrypt_lyrics` 自带 base64 解码步骤，与播放器本地 KRC 共用同一套密钥与解析器。
    let Some(text) = lyrics_crypto::decrypter::krc::decrypter::decrypt_lyrics(&content) else {
        return Ok(None);
    };
    let lines = parse_krc_lines(&text)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::KugouMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(candidate.id.clone()),
        },
        lines,
    }))
}
