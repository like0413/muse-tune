/// 当前歌词缓存磁盘格式与歌曲键规则的共同版本。
pub const LYRICS_CACHE_SCHEMA_VERSION: u32 = 6;

/// 返回版本目录和诊断界面共用的稳定标签。
pub fn lyrics_cache_schema_label() -> String {
    format!("v{LYRICS_CACHE_SCHEMA_VERSION}")
}
