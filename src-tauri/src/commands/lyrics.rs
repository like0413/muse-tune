use tauri::State;

use crate::{
    commands::run_blocking,
    error::Error,
    ipc::IpcError,
    lyrics::{
        LyricsChineseVariant, LyricsOnlineStrategy, LyricsService, LyricsSnapshot,
        convert_chinese_texts as convert_texts,
    },
    media::MediaPlayer,
};

const MAX_CONVERSION_TEXTS: usize = 8;
const MAX_CONVERSION_TEXT_BYTES: usize = 4 * 1024;

/// 返回最近一次歌词解析状态，供新创建的任务栏窗口补取初始值。
#[tauri::command]
pub fn get_current_lyrics(service: State<'_, LyricsService>) -> LyricsSnapshot {
    service.snapshot()
}

/// 批量转换歌曲展示字段；限制数量与长度，避免通用 IPC 被滥用于大段文本转换。
#[tauri::command]
pub async fn convert_chinese_texts(
    texts: Vec<String>,
    chinese_variant: LyricsChineseVariant,
) -> Result<Vec<String>, IpcError> {
    if texts.len() > MAX_CONVERSION_TEXTS
        || texts
            .iter()
            .any(|text| text.len() > MAX_CONVERSION_TEXT_BYTES)
    {
        return Err(IpcError::new(
            "lyrics.convert-chinese-texts",
            "待转换的媒体文本超过数量或长度限制",
            false,
        ));
    }
    run_blocking(
        "lyrics.convert-chinese-texts",
        "等待媒体文本简繁转换失败",
        move || Ok::<_, Error>(convert_texts(texts, chinese_variant)),
    )
    .await
}

/// 同步歌词总开关、联网能力、在线调度策略与在线接口集合到后端解析生命周期。
#[tauri::command]
pub fn set_lyrics_preferences(
    enabled: bool,
    chinese_variant: LyricsChineseVariant,
    allow_online: bool,
    online_strategy: LyricsOnlineStrategy,
    online_sources: Vec<MediaPlayer>,
    service: State<'_, LyricsService>,
) -> Result<(), IpcError> {
    service
        .set_preferences(
            enabled,
            chinese_variant,
            allow_online,
            online_strategy,
            online_sources,
        )
        .map_err(|error| IpcError::new("lyrics.set-preferences", error, false))
}
