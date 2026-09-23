use base64::Engine;
use reqwest::{
    blocking::Client,
    header::{REFERER, USER_AGENT},
};
use serde::Deserialize;
use serde_json::json;

use super::super::super::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricLine, LyricsSource, LyricsSourceKind, ResolvedLyrics, has_word_timing},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_qrc_lines},
    track::TrackDescriptor,
};
use crate::media::MediaPlayer;

const PLAY_LYRIC_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";

/// 匿名搜索 QQ 曲目，并按匹配分数依次尝试逐字与逐行歌词。
pub(in crate::lyrics::players) fn resolve(
    track: &TrackDescriptor,
    client: &Client,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    let query = format!("{} {}", track.title, track.artists.join(" "));
    let response = parse_json::<SearchResponse>(
        deadline
            .apply(
                client
                    .get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://y.qq.com/")
                    .query(&[("format", "json"), ("p", "1"), ("n", "10"), ("w", &query)]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let mut candidates = response
        .data
        .song
        .list
        .into_iter()
        .filter_map(|song| {
            let artists = song
                .singer
                .iter()
                .map(|artist| artist.name.clone())
                .collect::<Vec<_>>();
            let score = accepted_score(
                track,
                SongCandidate {
                    title: &song.song_name,
                    artists: &artists,
                    duration_ms: Some(u64::from(song.interval) * 1_000),
                },
            )?;
            Some((score, song))
        })
        .collect::<Vec<_>>();
    // 最高分候选取不到歌词时继续试次优候选，避免有词原版被无词版本遮挡。
    candidates.sort_by_key(|(score, _)| std::cmp::Reverse(*score));

    for (_, song) in candidates {
        if let Some(lyrics) = fetch_song_lyrics(client, &song, deadline)? {
            return Ok(Some(lyrics));
        }
    }
    Ok(None)
}

/// 读取单个候选曲目的歌词：优先逐字 QRC，接口缺失或格式变化时回退行级 LRC。
fn fetch_song_lyrics(
    client: &Client,
    song: &Song,
    deadline: &ResolutionDeadline,
) -> Result<Option<ResolvedLyrics>, LyricsError> {
    match fetch_word_lyrics(client, &song.song_mid, deadline) {
        Ok(Some(lines)) => {
            return Ok(Some(ResolvedLyrics {
                source: LyricsSource {
                    player: MediaPlayer::QqMusic,
                    kind: LyricsSourceKind::Online,
                    song_id: Some(song.song_mid.clone()),
                },
                lines,
            }));
        }
        Ok(None) => {}
        Err(error) => log::debug!("QQ 音乐在线 QRC 不可用，回退行级歌词: {error}"),
    }

    let response = parse_json::<LyricsResponse>(
        deadline
            .apply(
                client
                    .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://y.qq.com/")
                    .query(&[
                        ("songmid", song.song_mid.as_str()),
                        ("format", "json"),
                        ("nobase64", "0"),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(original) = decode_base64_text(response.lyric.as_deref()) else {
        return Ok(None);
    };
    let mut lines = parse_lrc_lines(&original)?;
    if let Some(translation) = decode_base64_text(response.trans.as_deref()) {
        let translation = parse_lrc_lines(&translation)?;
        merge_auxiliary_lines(&mut lines, &translation, AuxiliaryKind::Translation);
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::QqMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song.song_mid.clone()),
        },
        lines,
    }))
}

/// 从 QQ 音乐匿名 PlayLyricInfo 接口读取并解密在线 QRC 逐字歌词。
pub(super) fn fetch_word_lyrics(
    client: &Client,
    song_mid: &str,
    deadline: &ResolutionDeadline,
) -> Result<Option<Vec<LyricLine>>, LyricsError> {
    let request = json!({
        "comm": {
            "ct": 19,
            "cv": 1859,
            "uin": "0"
        },
        "req": {
            "module": "music.musichallSong.PlayLyricInfo",
            "method": "GetPlayLyricInfo",
            "param": {
                "songMID": song_mid,
                "songID": 0,
                "crypt": 1,
                "qrc": 1,
                "roma": 1,
                "trans": 1,
                "lrc_t": 0,
                "qrc_t": 0,
                "roma_t": 0,
                "trans_t": 0,
                "interval": 0,
                "type": 0,
                "format": "json",
                "ct": 19,
                "cv": 1859
            }
        }
    });
    let response = parse_json::<PlayLyricResponse>(
        deadline
            .apply(
                client
                    .post(PLAY_LYRIC_ENDPOINT)
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://y.qq.com/")
                    .json(&request),
            )?
            .send()?
            .error_for_status()?,
    )?;
    if response.code != 0 || response.req.code != 0 {
        return Ok(None);
    }
    let Some(data) = response.req.data else {
        return Ok(None);
    };
    if data.qrc != Some(1) || data.crypt != Some(1) {
        return Ok(None);
    }
    let Some(original) = decrypt_field(data.lyric.as_deref()) else {
        return Ok(None);
    };
    let mut lines = parse_qrc_lines(&original)?;
    // 有些曲目虽返回 lyric 字段，正文仍只有逐行数据；这种情况交给原 LRC
    // 链路处理，避免把“在线 QRC 可用”误报为逐字。
    if !has_word_timing(&lines) {
        return Ok(None);
    }

    merge_auxiliary_field(
        &mut lines,
        data.trans.as_deref(),
        AuxiliaryKind::Translation,
    );
    merge_auxiliary_field(
        &mut lines,
        data.roma.as_deref(),
        AuxiliaryKind::Romanization,
    );
    Ok(Some(lines))
}

/// 解密单条在线 QRC 密文字段；空字段或解密失败由调用方按能力缺失降级。
fn decrypt_field(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    lyrics_crypto::decrypter::qrc::decrypter::decrypt_lyrics(value)
}

/// QQ 的辅助轨可能是行级 LRC（翻译）或 QRC XML（罗马音），失败不影响主歌词。
fn merge_auxiliary_field(original: &mut [LyricLine], encrypted: Option<&str>, kind: AuxiliaryKind) {
    let Some(text) = decrypt_field(encrypted) else {
        return;
    };
    let parsed = if text.trim_start().starts_with("<?xml") {
        parse_qrc_lines(&text)
    } else {
        parse_lrc_lines(&text)
    };
    let Ok(auxiliary) = parsed else {
        return;
    };
    merge_auxiliary_lines(original, &auxiliary, kind);
}

/// 解码旧歌词接口返回的 Base64 文本。
fn decode_base64_text(value: Option<&str>) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value?)
        .ok()?;
    String::from_utf8(bytes).ok()
}

#[derive(Deserialize)]
struct SearchResponse {
    data: SearchData,
}

#[derive(Deserialize)]
struct SearchData {
    song: SongList,
}

#[derive(Deserialize)]
struct SongList {
    #[serde(default)]
    list: Vec<Song>,
}

#[derive(Deserialize)]
struct Song {
    #[serde(rename = "songmid")]
    song_mid: String,
    #[serde(rename = "songname")]
    song_name: String,
    #[serde(default)]
    singer: Vec<Artist>,
    interval: u32,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}

#[derive(Deserialize)]
struct LyricsResponse {
    lyric: Option<String>,
    trans: Option<String>,
}

#[derive(Deserialize)]
struct PlayLyricResponse {
    #[serde(default)]
    code: i32,
    req: PlayLyricRequest,
}

#[derive(Deserialize)]
struct PlayLyricRequest {
    #[serde(default)]
    code: i32,
    data: Option<PlayLyricData>,
}

#[derive(Deserialize)]
struct PlayLyricData {
    qrc: Option<i32>,
    crypt: Option<i32>,
    lyric: Option<String>,
    trans: Option<String>,
    roma: Option<String>,
}
