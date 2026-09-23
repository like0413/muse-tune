use reqwest::{
    blocking::Client,
    header::{COOKIE, REFERER, USER_AGENT},
};
use serde::Deserialize;

use crate::media::MediaPlayer;

use super::{LyricsResponse, parse_response};
use crate::lyrics::{
    error::LyricsError,
    matcher::{SongCandidate, accepted_score},
    model::{LyricsSource, LyricsSourceKind, ResolvedLyrics},
    network::{API_USER_AGENT, ResolutionDeadline, parse_json},
    track::TrackDescriptor,
};

const NETEASE_COOKIE: &str = "os=pc; appver=3.1.39; channel=netease;";

/// 匿名搜索网易云曲目，并优先读取响应中的 YRC 逐字歌词。
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
                    .post("https://music.163.com/api/search/get")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://music.163.com/")
                    .header(COOKIE, NETEASE_COOKIE)
                    .form(&[
                        ("s", query.as_str()),
                        ("type", "1"),
                        ("limit", "10"),
                        ("offset", "0"),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let Some(song) = response.result.and_then(|result| {
        result
            .songs
            .into_iter()
            .filter_map(|song| {
                let artists = song
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<_>>();
                let score = accepted_score(
                    track,
                    SongCandidate {
                        title: &song.name,
                        artists: &artists,
                        duration_ms: Some(song.duration),
                    },
                )?;
                Some((score, song))
            })
            .max_by_key(|(score, _)| *score)
            .map(|(_, song)| song)
    }) else {
        return Ok(None);
    };

    let response = parse_json::<LyricsResponse>(
        deadline
            .apply(
                client
                    .get("https://music.163.com/api/song/lyric")
                    .header(USER_AGENT, API_USER_AGENT)
                    .header(REFERER, "https://music.163.com/")
                    .header(COOKIE, NETEASE_COOKIE)
                    .query(&[
                        ("id", song.id.to_string()),
                        ("lv", "1".to_owned()),
                        ("kv", "1".to_owned()),
                        ("tv", "-1".to_owned()),
                        ("yv", "1".to_owned()),
                    ]),
            )?
            .send()?
            .error_for_status()?,
    )?;
    let lines = parse_response(&response)?;
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedLyrics {
        source: LyricsSource {
            player: MediaPlayer::NeteaseCloudMusic,
            kind: LyricsSourceKind::Online,
            song_id: Some(song.id.to_string()),
        },
        lines,
    }))
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Option<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(default)]
    songs: Vec<Song>,
}

#[derive(Deserialize)]
struct Song {
    id: u64,
    name: String,
    #[serde(default)]
    artists: Vec<Artist>,
    #[serde(rename = "duration", alias = "dt")]
    duration: u64,
}

#[derive(Deserialize)]
struct Artist {
    name: String,
}
