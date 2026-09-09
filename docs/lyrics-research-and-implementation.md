# 四播放器歌词调研、验证与实现方案

> 状态：技术调研完成 / 首版代码已实现 / 待实机回归
> 适用项目：Muse Tune  
> 调研日期：2026-09-09  
> 目标平台：Windows 11、Tauri 2、Rust、Vue 3

## 1. 文档范围

本文给出 Muse Tune 支持以下四家 Windows 桌面播放器歌词的调研结论、实机验证证据和可直接实施的技术方案：

- QQ 音乐；
- 网易云音乐；
- 汽水音乐；
- 酷狗音乐。

首版采用“本地优先、国内网络源兜底”的策略，明确排除：

- LRCLIB、Musixmatch、Spotify、Apple Music 等国外歌词源；
- 要求用户登录、读取 Cookie 或提取登录凭据的接口；
- 扫描播放器进程内存、注入播放器或修改播放器文件；
- 依赖 HTTP 明文传输的在线歌词链路；
- 将未经验证的新项目整体作为关键运行时依赖。

本文只记录方案，不修改生产代码。歌词功能实施时应继续遵守项目的事件驱动、关注点分离、性能和可维护性约定。

## 2. 结论摘要

### 2.1 推荐数据链路

统一解析顺序如下：

1. Muse Tune 已解析歌词缓存；
2. 当前播放器本地歌词缓存或可用于定位歌曲的本地元数据；
3. 当前播放器的国内官方域名 HTTPS 接口；
4. QQ 音乐和网易云音乐的国内 HTTPS 接口并行匹配；
5. 所有候选均不满足高置信度条件时返回“无可靠歌词”，不冒险展示最高分候选。

播放器切歌事件只触发一次解析任务。播放进度更新只用于前端定位当前行和逐字进度，不重复搜索、读取目录或发起网络请求。

### 2.2 官方接口结论

本次没有找到四家平台对第三方桌面应用公开、无需登录并承诺稳定性的歌词 API 文档。

QQ 音乐、网易云音乐和汽水音乐存在无需登录即可访问的官方域名内部接口，并已对部分真实响应完成验证。这些接口不能表述为“公开官方 API”，也不能假定响应结构长期稳定。实现时必须：

- 每个平台使用独立适配器；
- 限制请求超时、重试次数和响应体大小；
- 严格校验状态码、Content-Type 和 JSON 字段；
- 允许单个平台独立失败；
- 将接口路径和解析逻辑集中管理，便于替换。

没有发现值得作为 Muse Tune 运行时依赖的稳定国内第三方托管歌词 API。第三方开源项目适合作为协议、算法和匹配策略的交叉证据，不适合作为远程服务依赖。

### 2.3 本地缓存结论

| 播放器     | 本地数据价值                 | 自定义路径发现             | 实机状态                                        | 首版用途              |
| ---------- | ---------------------------- | -------------------------- | ----------------------------------------------- | --------------------- |
| QQ 音乐    | 原文、翻译、音译 QRC         | 注册表 `CACHEPATH`         | 已验证真实 QRC 可解密为逐字歌词                 | 首选歌词源            |
| 酷狗音乐   | KRC 原文、逐字时序、语言扩展 | `KuGou.ini` 的 `LyricPath` | 已验证真实 KRC 可解密解析                       | 首选歌词源            |
| 网易云音乐 | 旧版可能有 LRC/YRC/JSON      | 默认目录探测加用户覆盖     | 当前安装未发现独立歌词目录                      | 尽力读取，HTTPS 兜底  |
| 汽水音乐   | 队列中的歌曲 ID 和元数据     | `%APPDATA%\SodaMusic`      | 已验证 QueueCache 有曲目 ID，未发现独立歌词文件 | 为官方域名接口提供 ID |

“播放器缓存目录”不是统一概念。QQ 音乐和酷狗音乐可以直接定位歌词目录；网易云音乐新版不保证保留独立歌词缓存；汽水音乐目前只能可靠利用本地队列元数据，不能把媒体缓存目录误当作歌词目录。

## 3. 调研与验证方法

### 3.1 证据等级

文中使用以下状态区分结论强度：

- **已实机验证**：在当前 Windows 环境对已安装客户端、真实缓存文件或实时接口执行了只读验证；
- **已交叉验证**：至少有两个独立开源实现或一个实现加本地文件结构支持该结论；
- **待实施验证**：方案可行但尚未用本项目 Rust 代码和固定夹具验证，不能直接视为完成。

### 3.2 隐私边界

验证只读取本地配置、缓存头部和结构化统计。文档不记录完整用户名、用户目录、真实歌词正文、完整播放历史或登录信息。

### 3.3 已完成的实机验证

| 验证项                     | 结果   | 证据摘要                                                           |
| -------------------------- | ------ | ------------------------------------------------------------------ |
| QQ 音乐自定义缓存根目录    | 通过   | 注册表中的缓存根目录指向非系统盘，证明不能硬编码默认路径           |
| QQ 音乐本地 QRC            | 通过   | 真实 `_qm.qrc` 解密后得到 110 行，并检测到逐字时序                 |
| 酷狗音乐自定义歌词目录     | 通过   | UTF-16LE INI 中的 `LyricPath` 指向非系统盘                         |
| 酷狗音乐本地 KRC           | 通过   | 文件具有 `krc1` 头，解密后得到 28 行、逐字时序和语言扩展块         |
| 网易云音乐旧歌词目录       | 未命中 | 当前安装只有音频缓存等目录，未发现可依赖的独立 `webdata\lyric`     |
| 汽水音乐队列元数据         | 通过   | `LUNA` 容器内的 gzip JSON 包含歌曲 ID、标题、歌手、专辑和时长      |
| 汽水音乐独立歌词缓存       | 未发现 | `LunaCacheV2` 更符合媒体缓存索引与数据块，不应假设其中稳定保存歌词 |
| QQ 音乐无登录歌词接口      | 通过   | 官方域名响应有效 Base64 行歌词                                     |
| 网易云音乐无登录歌词接口   | 通过   | 官方域名响应有效逐行歌词和翻译字段                                 |
| 汽水音乐按歌曲 ID 获取歌词 | 通过   | 官方域名 SEO 响应包含曲目详情和歌词                                |
| 酷狗在线歌词 HTTPS         | 不通过 | 当前公开链路 HTTPS 握手失败，HTTP 可用但不满足安全要求             |

QRC 验证使用 [LRC-GET](https://github.com/valenbine/LRC-GET) 的 PC 本地 QRC 动态掩码、DES/3DES 和 zlib 流程作为独立实现证据。KRC 结构和解析结果同时与 [Lyricify Lyrics Helper](https://github.com/WXRIW/Lyricify-Lyrics-Helper) 及 [Rust Lyrics Helper](https://github.com/ChouChiu/Lyrics-Helper) 对照。

## 4. QQ 音乐

### 4.1 路径发现

自动模式按以下顺序定位：

1. 读取 `HKCU\Software\Tencent\QQMusic\LogConfig` 的 `CACHEPATH`；
2. 在该根目录下拼接 `QQMusicLyricNew`；
3. 注册表缺失或目录无效时尝试已知默认位置；
4. 用户已设置 Muse Tune 手动覆盖目录时，覆盖值优先于自动发现；
5. 覆盖目录失效时返回明确诊断，但不静默改写用户设置。

注册表值和目录都属于客户端实现细节。应使用 Windows 注册表通知监听键值变化，目录内文件变化使用文件系统事件，不轮询整个目录。

### 4.2 文件与匹配

已观察到的文件族包括：

- `*_qm.qrc`：原文；
- `*_qmts.qrc`：翻译；
- `*_qmRoma.qrc`：音译。

文件名通常含歌手、标题、时长和专辑，可用于廉价筛选。实现不得仅依赖文件名格式，因为客户端版本可能改变命名规则。推荐流程：

1. 初次扫描只建立文件名、大小、修改时间和路径索引；
2. 用规范化标题、歌手和时长筛出少量候选；
3. 只读取和解密候选文件；
4. 将同一基础名的原文、翻译、音译合并；
5. 解密或结构校验失败时跳过单个文件，不中止整个解析任务。

### 4.3 QRC 解包边界

PC 本地 QRC 与接口返回的 QRC 不是同一外层格式：

1. PC 文件先进行 QMC dynamic mask 解掩码；
2. 识别解掩码后的头部和偏移；
3. 对后续密文执行 QQ QRC 的 DES/3DES 兼容解密；
4. zlib 解压；
5. 解析 QRC XML 或文本中的行与逐字时序。

当前 `lyrics-crypto` 的 QRC 实现只接受接口型十六进制密文，没有覆盖本机 PC 动态掩码封装。因此不能宣称直接引入该 crate 就能读取本地 QQ 缓存。首版应把“PC 外层解包”和“QRC 密文解密”作为独立步骤，并用真实匿名化夹具锁定行为。

### 4.4 在线兜底

已验证的歌词示例接口：

```text
GET https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg
    ?songmid={song_mid}&format=json&nobase64=0
Referer: https://y.qq.com/
```

[已验证示例响应](https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid=0039MnYb0qxYhV&format=json&nobase64=0)

歌词获取本身已实机验证；匿名搜索和更完整的逐字 QRC `musicu.fcg` 请求仍需在实施阶段用固定歌曲验证。若搜索接口失效，QQ 适配器应独立降级，不影响网易云或汽水适配器。

## 5. 酷狗音乐

### 5.1 路径发现

自动模式读取：

```text
%APPDATA%\KuGou8\KuGou.ini
[LyricConfigSection]
LyricPath=...
```

该 INI 在当前客户端为 UTF-16LE，解析时应支持 BOM，并容忍大小写、空行和尾部路径分隔符。不能用 `DownloadPath` 或 `TempPath` 代替 `LyricPath`。

路径优先级为：Muse Tune 手动覆盖 → `LyricPath` → 已知默认目录。INI 文件变更由文件系统事件触发重新读取。

### 5.2 KRC 解密与解析

真实文件已验证具有 ASCII `krc1` 四字节头。后续数据按公开实现执行循环 XOR 和 zlib 解压，明文中包含：

- 行起始时间和时长；
- 逐字起始时间、时长和文本；
- 标签信息；
- 可选语言扩展块，可承载翻译或音译。

可选择性使用 `lyrics-crypto` 的 KRC 字节解密和 `lyrics-parsers` 的 KRC 解析，但必须先通过本机匿名化夹具。任何 panic、越界、非法 UTF-8 或异常大解压结果都应转换为受控错误。

### 5.3 在线策略

调研到的酷狗歌词搜索和下载链路在本机通过 HTTP 可以返回数据，但 HTTPS 当前握手失败。根据已确定的安全边界：

- 不发起酷狗 HTTP 请求；
- 不为了酷狗接口关闭 TLS 校验；
- 不引入公共代理绕过 TLS；
- 本地 KRC 未命中时，转用 QQ 音乐和网易云音乐 HTTPS 匹配。

如果未来酷狗提供可验证的 HTTPS 链路，只需增加或恢复其在线适配器，不改变上层协调器。

## 6. 网易云音乐

### 6.1 本地缓存

旧版本和部分开源方案使用：

```text
%LOCALAPPDATA%\NetEase\CloudMusic\webdata\lyric
```

当前实机安装未发现该目录，因此它只能作为机会式数据源，不能作为网易云支持的前提。实现应：

- 探测旧目录和用户手动覆盖目录；
- 按内容识别 LRC、YRC 或 JSON，不只按扩展名判断；
- 不遍历音频 `Cache\Cache` 试图猜测歌词；
- 不解析未知或加密配置来强行寻找路径；
- 没有本地歌词时立即进入 HTTPS 适配器。

### 6.2 在线兜底

已验证的歌词请求：

```text
GET https://music.163.com/api/song/lyric
    ?id={song_id}&lv=1&kv=1&tv=-1
Referer: https://music.163.com/
Cookie: os=pc; appver=...
```

这里的 Cookie 只包含匿名客户端环境字段，不读取或传递用户登录 Cookie。

[已验证示例响应](https://music.163.com/api/song/lyric?id=186016&lv=1&kv=1&tv=-1)

匿名搜索可隔离使用 `https://music.163.com/api/search/get`。适配器应同时识别普通 LRC、翻译歌词和响应中存在的 YRC 逐字歌词；单个字段缺失不能导致整个响应失败。

## 7. 汽水音乐

### 7.1 本地数据定位

当前 Windows 客户端在 `%APPDATA%\SodaMusic` 下保存 `LunaStorage` 和 `LunaCacheV2`。实机结果表明：

- `LunaStorage\QueueCache` 使用 `LUNA` 外层和 gzip JSON，可取得当前或近期曲目的稳定 ID、标题、歌手、专辑和时长；
- `LunaCacheV2` 更符合消息打包索引和媒体 `.bin` 数据块，没有证据证明它是稳定歌词缓存；
- 客户端渲染状态中可以出现歌词，但不能据此推断磁盘上必然存在可稳定读取的独立歌词文件。

该判断可与 [SodaMusic Cache Export 的本地缓存协议说明](https://github.com/YancyWei/sodamusic-cache-export/blob/main/docs/local-cache-protocol.md) 交叉检查。

首版只把 QueueCache 当作“歌曲 ID 辅助索引”，不得修改文件，也不得把整个播放队列写入日志或 Muse Tune 缓存。

### 7.2 歌曲 ID 关联

GSMTC 当前只提供标题、歌手和时长，不提供汽水歌曲 ID。汽水本地适配器应：

1. 在 QueueCache 中按规范化标题和歌手筛选；
2. 用时长消除同名版本歧义；
3. 只有唯一高置信度结果时取出歌曲 ID；
4. 多个候选无法消歧时不调用按 ID 接口；
5. QueueCache 结构变化时返回“本地元数据格式不支持”，再走跨平台兜底。

### 7.3 官方域名 SEO 接口

已验证无需登录的按 ID 请求：

```text
GET https://beta-luna.douyin.com/luna/h5/seo_track
    ?track_id={track_id}&device_platform=web
```

[已验证示例响应](https://beta-luna.douyin.com/luna/h5/seo_track?track_id=7395494043440072705&device_platform=web)

该接口能返回歌曲详情和歌词，但仍属于内部接口。曾被开源项目使用的旧 `api.qishui.com/luna/pc/track_v2` 请求在当前验证中返回空响应，不应采用。汽水匿名搜索没有找到同等可靠的链路，因此没有本地歌曲 ID 时直接转用 QQ/网易云匹配。

## 8. 第三方实现与许可证评估

| 项目                                                                         | 许可证     | 可借鉴内容                                   | 采用边界                                               |
| ---------------------------------------------------------------------------- | ---------- | -------------------------------------------- | ------------------------------------------------------ |
| [Lyricify Lyrics Helper](https://github.com/WXRIW/Lyricify-Lyrics-Helper)    | Apache-2.0 | QRC/KRC/YRC 模型、解密、解析和多平台适配经验 | .NET 实现，不引入 sidecar；作为成熟参考                |
| [Rust Lyrics Helper](https://github.com/ChouChiu/Lyrics-Helper)              | Apache-2.0 | `lyrics-parsers`、KRC 解密、Rust 数据结构    | 项目较新；只选择通过真实夹具的子库，不启用完整搜索功能 |
| [TaskbarLyrics](https://github.com/ANYNC/TaskbarLyrics)                      | MIT        | GSMTC 触发、多源匹配、分数阈值和持久映射     | 借鉴架构与行为，不引入 .NET 运行时                     |
| [LRC-GET](https://github.com/valenbine/LRC-GET)                              | MIT        | PC 本地 QRC dynamic mask 与解密流程          | 维护历史较短；只作算法交叉验证并保留许可证要求         |
| [LDDC](https://github.com/chenmozhijin/LDDC)                                 | GPL-3.0    | QRC/KRC/YRC 行为、格式和异常样本参考         | 只做黑盒结果和协议交叉验证，不复制 GPL 代码            |
| [SodaMusic Cache Export](https://github.com/YancyWei/sodamusic-cache-export) | MIT        | 汽水媒体缓存协议与索引结构                   | 只用于确认 `LunaCacheV2` 职责，不把它当歌词提供方      |

依赖决策：

- 可以评估直接依赖 `lyrics-parsers`；
- `lyrics-crypto` 只考虑 KRC 路径，且必须通过真实夹具门禁；
- 不启用或依赖 `lyrics-helper` 的 `search` 默认功能；
- QQ PC QRC 外层解包由 Muse Tune 的 QQ 本地适配模块负责；
- 在线请求由 Muse Tune 自有的窄接口适配器实现，避免失效平台拖累全部歌词功能。

## 9. 目标架构

```text
MediaSessionChanged
        │
        ▼
LyricsCoordinator ── generation/cancellation ──► LyricsSnapshot event
        │
        ├── ParsedLyricsCache
        │
        ├── LocalSource
        │   ├── QqMusicLocalSource
        │   ├── KugouLocalSource
        │   ├── NeteaseLocalSource
        │   └── SodaMetadataSource
        │
        ├── OnlineSource
        │   ├── QqMusicOnlineSource
        │   ├── NeteaseOnlineSource
        │   └── SodaOnlineSource
        │
        ├── CandidateMatcher
        └── LyricsNormalizer
            ├── QRC
            ├── KRC
            ├── YRC
            └── LRC/JSON
```

### 9.1 协调器

`LyricsCoordinator` 负责数据源顺序、并发、取消和最终状态，不包含任何平台协议代码：

- 媒体歌曲身份变化时增加 generation，并取消上一首的本地读取和网络请求；
- 相同 `trackKey` 的播放、暂停和时间线更新不重新解析；
- 缓存命中立即发布 `ready`；
- 本地源串行按当前播放器查询，跨平台在线源并行查询；
- 发布结果前再次比较 generation 和 `trackKey`，杜绝上一首歌词覆盖当前歌曲；
- 单个数据源错误只写入诊断，不直接把总体状态设为失败。

### 9.2 模块职责

- `LocalSource`：发现目录、建立轻量索引、读取候选原始数据；
- `OnlineSource`：搜索歌曲和获取歌词，不参与最终匹配决策；
- `CandidateMatcher`：统一规范化和评分，处理版本冲突；
- `LyricsNormalizer`：把各平台格式变为统一逐行/逐字模型；
- `ParsedLyricsCache`：保存最终规范化结果，不缓存登录信息或完整搜索响应；
- Vue composable：订阅歌词事件，结合现有媒体时间线计算展示状态。

## 10. 公共接口与数据模型

歌词不能加入高频变化的 `MediaSessionSnapshot`。媒体元数据事件和歌词结果事件生命周期不同，混合会造成无意义的大对象序列化。

### 10.1 Rust/IPC 模型

```rust
pub struct LyricsSnapshot {
    pub track_key: String,
    pub status: LyricsStatus,
    pub source: Option<LyricsSource>,
    pub precision: Option<LyricsPrecision>,
    pub lines: Vec<LyricLine>,
    pub error_reason: Option<LyricsErrorReason>,
}

pub struct LyricLine {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub translation: Option<String>,
    pub romanization: Option<String>,
    pub words: Vec<LyricWord>,
}

pub struct LyricWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}
```

枚举至少包含：

- `LyricsStatus`：`loading`、`ready`、`unavailable`、`error`；
- `LyricsPrecision`：`word`、`line`；
- `LyricsSource`：平台、`local`/`online`、可选平台歌曲 ID；
- `LyricsErrorReason`：目录无效、格式不支持、无可靠匹配、网络不可用、响应无效。

`error_reason` 用于设置页诊断，不应直接在 48px 任务栏中显示技术错误。

### 10.2 命令与事件

| 名称                             | 方向        | 用途                                 |
| -------------------------------- | ----------- | ------------------------------------ |
| `get_current_lyrics`             | 前端 → Rust | 新窗口初始化时取得当前快照           |
| `get_lyrics_cache_paths`         | 前端 → Rust | 读取自动发现路径、覆盖路径和有效状态 |
| `set_lyrics_cache_path_override` | 前端 → Rust | 为指定播放器设置或清除手动覆盖       |
| `lyrics://changed`               | Rust → 前端 | 歌词状态或最终结果变化               |
| `lyrics://cache-paths-changed`   | Rust → 前端 | 注册表、INI、目录或手动设置变化      |

设置写入由 Rust 负责，避免前端 store 与后端实际读取路径形成两个事实源。手动覆盖值使用应用设置持久化；清除覆盖即恢复自动发现。

### 10.3 Track Key

`trackKey` 使用以下字段规范化后计算版本化哈希：

```text
schema version + player + title + sorted artists + rounded duration
```

- 标题和歌手执行 Unicode 规范化、大小写折叠、空白与常见全半角标点归一化；
- 时长按秒取整，避免 GSMTC 毫秒抖动；
- 不使用封面、播放状态或会话实例 ID；
- 缺少时长时允许生成键，但在线自动匹配要求更严格。

## 11. 匹配与冲突规则

候选总分固定为 100：

| 项目 | 分值 | 规则                                                       |
| ---- | ---- | ---------------------------------------------------------- |
| 标题 | 50   | 规范化完全相同得 50；相似度按比例降分                      |
| 歌手 | 30   | 规范化集合完全相同得 30；有交集但不完整最多 20             |
| 时长 | 20   | 差值不超过 2 秒得 20，5 秒得 15，10 秒得 8，超过 10 秒得 0 |

自动采用必须同时满足：

- 总分不低于 90；
- 标题相似度不低于 0.9；
- 至少一个规范化歌手匹配；
- 有时长时差值不超过 5 秒；
- 不存在版本标记硬冲突。

版本标记至少识别：`live`、`现场`、`remix`、`dj`、`伴奏`、`纯音乐`、`翻唱`、`cover`、`demo`、`片段`。当前曲目和候选一方有标记、另一方没有，或标记集合不同，直接拒绝，不允许分数补偿。

缺少时长时，标题和完整歌手集合必须完全一致，且搜索结果中只能有一个无冲突候选。多个平台返回同分结果时，优先级为当前播放器同平台 → 本地源 → QQ 音乐 → 网易云音乐；优先级只用于高置信度候选之间，不用于放宽阈值。

可参考 [TaskbarLyrics](https://github.com/ANYNC/TaskbarLyrics) 的多源评分和提前接受思路，但 Muse Tune 首版采用更保守的 90 分阈值。

## 12. 缓存、目录监听与性能

### 12.1 Muse Tune 解析后缓存

使用 Tauri `app_cache_dir` 下的专用版本目录，不与播放器缓存混放：

```text
{app_cache_dir}/lyrics/v1/
├── entries/{track_key}.json
└── index.json
```

每个条目保存统一模型、来源、平台歌曲 ID、创建时间和源文件指纹。写入流程为“同目录临时文件 → flush → 原子替换”，防止崩溃产生半文件。

缓存失效条件：

- schema 版本变化；
- 本地源文件大小或修改时间变化；
- 用户改变缓存目录覆盖；
- 在线响应解析版本变化；
- 用户主动清理应用缓存。

内部接口没有稳定性承诺，因此在线歌词不应因短 TTL 被频繁重取；解析成功条目长期复用，明确损坏或版本迁移时再失效。

### 12.2 目录索引

- 初次只枚举文件名和文件元数据，不解密全部文件；
- 索引工作放到阻塞 I/O 线程，不占用 Tauri 主线程；
- 大目录分批构建，并允许当前歌曲的精确文件名候选优先；
- 文件系统事件只增量更新变化项；
- 不递归监听无关的整个播放器缓存根目录；
- 解析和解压设置输入、输出大小上限，防止损坏文件造成内存放大。

### 12.3 路径变化事件

- QQ 音乐：`RegNotifyChangeKeyValue` 监听对应注册表键；
- 酷狗音乐：成熟文件监听库监听具体 INI 和歌词目录；
- 网易云、汽水和手动覆盖目录：只监听最终选定目录及必要的配置文件；
- 监听失败时在下一次歌曲身份变化时进行一次廉价路径重检，不启动固定周期轮询。

## 13. 网络安全与容错

在线适配器统一执行：

- 只允许 `https` 和明确的官方域名白名单；
- 连接与总请求超时；
- 最多一次带退避的瞬时错误重试；
- 限制重定向次数，重定向目标仍需通过域名白名单；
- 限制响应体大小和解码后歌词大小；
- 不关闭证书验证；
- 不发送用户登录 Cookie、设备唯一标识或播放历史；
- User-Agent 使用固定的 Muse Tune 标识，不伪装已登录用户；
- 日志只记录平台、阶段、状态码和错误类别，不记录完整歌词或完整请求查询。

接口响应结构改变时返回 `response_invalid`，保留其他平台继续工作。网络失败不能阻塞播放器控制、媒体监控、频谱或任务栏布局线程。

## 14. 任务栏交互方案

### 14.1 常态与悬停

- 常态：显示封面和歌词；歌曲信息区与控制按钮隐藏；
- 悬停任务栏内容区：歌词隐藏，恢复现有歌曲信息和控制按钮；
- 鼠标离开：恢复歌词；
- 封面始终遵守现有可见性设置；
- 频谱始终按现有设置渲染，不随歌词/悬停切换；
- 现有进度条、右键唤醒播放器和音量交互保持不变。

切换采用同一布局容器内的短透明度过渡，避免改变任务栏宽度和触发布局抖动。控制按钮原本被用户关闭时，悬停也不强制显示。

### 14.2 两行歌词

48px 高度内：

- 第一行显示当前原文；有逐字时序时按词推进高亮，无逐字时序时整行高亮；
- 第二行优先显示当前行翻译；没有翻译时显示下一行原文；
- 音译保留在数据模型中，首版不占用默认第二行；
- 长文本使用单行裁切或与现有标题滚动能力一致的受控滚动，不改变窗口高度；
- 空白时间段保留上一有效行，直到下一行开始；歌曲结束后保持最后一行。

### 14.3 回退状态

以下状态完整回退现有“封面 + 歌曲信息 + 控制按钮”界面：

- 歌词仍在加载；
- 无本地缓存且网络不可用；
- 只有低置信度候选；
- 纯音乐或无时间轴文本；
- 解析器不支持当前格式；
- 当前媒体会话不是四家支持的播放器。

一旦当前 `trackKey` 的歌词变为 `ready`，无需用户操作即可切回歌词常态。

## 15. 实施顺序

1. 建立统一歌词模型、错误类型、`trackKey` 和格式解析边界；
2. 用匿名化真实 QRC/KRC 夹具验证选择性依赖，完成 QQ/Kugou 本地源；
3. 完成路径自动发现、手动覆盖、目录监听和轻量索引；
4. 完成解析后缓存与 `LyricsCoordinator`，接入媒体歌曲变化事件；
5. 实现并隔离 QQ、网易云、汽水 HTTPS 适配器和统一匹配器；
6. 增加 Tauri 命令/事件和 Vue composable；
7. 实现任务栏常态歌词、悬停切换和无歌词回退；
8. 完成设置页目录状态、手动覆盖及诊断信息；
9. 执行性能、错误隔离和真实播放器验收。

每一阶段都必须能独立失败并回退到现有任务栏功能，不能让歌词成为媒体控制和任务栏窗口初始化的硬依赖。

## 16. 验证与验收清单

### 16.1 本地文件

- QQ 默认目录、自定义目录、注册表值改变、目录不存在；
- QQ 原文、翻译、音译组合，以及损坏、截断和未知 QRC 变体；
- 酷狗 UTF-16LE INI、自定义 `LyricPath`、KRC 语言扩展和损坏文件；
- 网易云旧目录存在、不存在、空目录、LRC/YRC/JSON 混合；
- 汽水 QueueCache 有 ID、无 ID、多个同名 ID、结构版本改变；
- 大目录首次索引期间切歌，不能阻塞媒体事件或窗口渲染。

### 16.2 匹配与网络

- 标题和歌手完全一致、轻微标点差异、多个歌手顺序变化；
- Live、Remix、DJ、伴奏、纯音乐、翻唱和同名不同歌曲；
- 时长缺失、差 2 秒、5 秒、10 秒和超过 10 秒；
- QQ/网易云/汽水分别超时、空响应、非 JSON、字段缺失和超大响应；
- 断网时本地缓存仍可用，所有网络源失败时快速回退；
- 验证不会发出酷狗 HTTP 请求，不会访问国外歌词域名。

### 16.3 并发与界面

- 快速连续切歌，旧 generation 结果不能覆盖当前歌曲；
- 暂停、继续、拖动进度后当前行和逐字高亮立即同步；
- 多任务栏窗口取得同一首歌的相同歌词快照；
- 常态歌词、悬停信息/控制、鼠标离开恢复歌词；
- 无歌词时完整回退，歌词稍后成功时自动切换；
- 封面、控制、频谱现有可见性设置继续生效；
- 歌词功能失败不影响播放控制、音量、频谱、任务栏定位和右键唤醒播放器。

### 16.4 验证命令边界

按项目约定，实施阶段只运行：

```powershell
vp check
cargo check --manifest-path src-tauri/Cargo.toml
```

不运行 `test` 或 `build` 命令。真实缓存验证使用只读验证入口或临时工具，只输出格式、行数、时序范围和错误分类，不输出歌词正文；验证完成后不把用户缓存复制进仓库。

## 17. 已确定的默认约束

- 首版仅支持 Windows 11 上的 QQ 音乐、网易云音乐、汽水音乐和酷狗音乐；
- 逐字优先，逐行兜底；
- 第二行优先翻译，无翻译时显示下一行；
- 本地路径自动发现，并允许用户手动覆盖和恢复自动；
- 酷狗只读取本地 KRC，禁用其 HTTP 在线链路；
- 不使用国外歌词服务、需登录接口或用户登录凭据；
- 低置信度结果不自动采用，也不写入缓存；
- 不增加候选选择弹窗；
- 无可靠歌词时回退现有任务栏界面；
- 所有官方域名内部接口均按不稳定依赖设计，可独立替换或禁用。

## 18. 参考资料

- [Lyricify Lyrics Helper](https://github.com/WXRIW/Lyricify-Lyrics-Helper)
- [Rust Lyrics Helper](https://github.com/ChouChiu/Lyrics-Helper)
- [TaskbarLyrics](https://github.com/ANYNC/TaskbarLyrics)
- [LRC-GET](https://github.com/valenbine/LRC-GET)
- [LDDC](https://github.com/chenmozhijin/LDDC)
- [SodaMusic Cache Export](https://github.com/YancyWei/sodamusic-cache-export)
- [QQ Music lyric endpoint validation sample](https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid=0039MnYb0qxYhV&format=json&nobase64=0)
- [NetEase lyric endpoint validation sample](https://music.163.com/api/song/lyric?id=186016&lv=1&kv=1&tv=-1)
- [Soda SEO track endpoint validation sample](https://beta-luna.douyin.com/luna/h5/seo_track?track_id=7395494043440072705&device_platform=web)
