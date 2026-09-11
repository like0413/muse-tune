use reqwest::{
    blocking::Client,
    header::{REFERER, USER_AGENT},
};
use serde::Deserialize;
use serde_json::json;

use super::super::super::{
    error::LyricsError,
    model::{LyricLine, has_word_timing},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    parser::{AuxiliaryKind, merge_auxiliary_lines, parse_lrc_lines, parse_qrc_lines},
};

const PLAY_LYRIC_ENDPOINT: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";

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
