# MeetingLens 架构设计

## 1. 架构目标

MeetingLens 当前采用 Personal Edition / 自用版基线：“轻量桌面壳 + 本地转录工作区 + Rust Summary 执行边界 + 本地数据库 + 按需受控 Worker + 可选外部 AI”。Tauri/Rust 控制系统权限、转录导入、SQLite、SecretStore、持久 Summary run 和 Provider 调用，React 负责交互；Python 只用于现有认证健康 Sidecar 和按功能启动的隔离 Worker。

核心目标是明确权限边界、允许处理模块独立替换，并让原始会议数据默认不离开本机。

当前近期产品只推进总结模式：粘贴或导入转录，随后由用户明确确认的 Rust DeepSeek Provider 生成纪要，再在页面查看、修改、复制或保存 Markdown。Rust-only DeepSeek、最小确认和撤销底层已经实现；正式生成入口、结果页面和 Markdown 保存仍未实现。

总结、翻译和会议模式的长期愿景仍共享同一引擎，不建立三个应用或三套后端；但翻译模式、实时能力和独立会议模式工作区当前均为 Frozen / Deferred。现行持久化真相是 meeting、transcript source/segment、summary snapshot/evidence、最小 Provider Consent 当前偏好和 deletion pipeline；不得把未来 translation、checkpoint 或通用 Consent 审计设计描述为当前 schema 事实。

## 2. 系统上下文

```mermaid
flowchart LR
    U["用户"] --> UI["React WebView"]
    UI <--> CORE["Tauri / Rust Core"]
    CORE <--> DB["Local SQLite"]
    CORE <--> FS["Private App Data"]
    CORE <--> SIDE["现有认证健康 Sidecar"]
    CORE --> WORKERS["按请求启动的隔离 Worker"]
    CORE --> SUMMARY["Rust SummaryProvider"]
    SUMMARY -->|"固定 HTTPS；确认后的最小快照"| DS["DeepSeek API"]
```

### 2.1 Personal Edition 当前处理主线

```mermaid
flowchart LR
    HOME["总结模式"] --> SOURCE["粘贴或导入 TXT/SRT/VTT"]
    SOURCE --> SEGMENTS["统一 Transcript Segments"]
    SEGMENTS --> SNAPSHOT["不可变最终输入快照"]
    SNAPSHOT --> CONSENT["Rust Key / Consent 校验"]
    CONSENT --> DEEPSEEK["Rust DeepSeek Provider"]
    SNAPSHOT -. "仅测试/debug" .-> MOCK["Rust 零网络 Mock"]
    MOCK -.-> RUN["Summary run / evidence"]
    DEEPSEEK --> RUN
    RUN -. "随后阶段" .-> MINUTES["页面查看、修改、复制或保存 Markdown"]
```

翻译、实时字幕、checkpoint 和独立会议工作区属于 Deferred；若未来解冻，仍只能作为共享管线上的派生能力。最终纪要继续以原始 segment revision 和不可变输入快照为准，Provider 失败不得破坏已有本地转录。

## 3. 组件职责

### 3.1 React WebView

- 当前显示会议、转录工作区和 Summary run 开发状态；录音、实时翻译和独立会议工作区属于 Deferred。
- 只通过定义明确的 Tauri commands 访问系统能力。
- 不直接读取 `.env`、SQLite、任意文件路径或操作系统凭据。
- 不直接启动 sidecar，也不持有长期 API key。

界面国际化集中在 `src/i18n`：`i18next` 与 `react-i18next` 在应用入口初始化，`zh-CN`/`en-US` JSON 资源随 Vite bundle 打包，`en-US` 是可靠 fallback，不加载远程语言包。React 页面只使用稳定 translation key；集中 formatters 按当前 UI locale 和用户本地时区显示日期、数字与文件大小。设置页只把允许的 locale 枚举写入 `meetinglens.uiLocale`，不使用 SecretStore 或 SQLite，也不将 UI locale 复用为未来 Summary 输出语言。

### 3.2 Tauri / Rust Core

- 作为 WebView 与本机资源之间的权限边界。
- 管理窗口、文件选择和应用生命周期；未来如解冻设备采集，仍由 Rust 管理权限。
- 生成每次应用启动的会话令牌，并作为唯一授权调用方管理现有 Sidecar 健康通信。
- MVP 1A-3B-1 已取得安全传递令牌所需的最小开发 Sidecar 启停所有权；完整实时 Sidecar supervisor、监控和孤儿进程回收当前为 Deferred。
- 通过 Tauri App State 管理 `SecretStore`，并在 Windows 使用当前用户的 Credential Manager；WebView 只能写入、查询配置状态和删除，不能读回值。
- 对 IPC 输入做类型、路径和大小校验。
- 负责数据库迁移、原子写入和删除协调。
- 在 Rust DeepSeek Provider 内执行最小首次外发确认、remember/revoke、SecretStore 消费和请求取消；麦克风、系统音频、持续翻译和实时屏幕采集的独立会话同意仅作为 Deferred 安全边界保留，应用启动不得自动申请权限。

### 3.3 Python Sidecar

- 提供版本化 loopback 健康 API；当前只提供认证健康契约，不接收用户文件、凭据或会议正文。
- Sidecar 保持无运行时第三方依赖的窄 HTTP 服务；MVP 1A-3B-1 已为所有路由增加会话认证。
- 近期录音文件转录不扩展为实时 Sidecar 栈，而是复用 Docling 的受控模式，由 Rust 为每次任务启动一次性 faster-whisper Worker。
- 完整 supervisor、实时 ASR、翻译和说话人分离当前为 Deferred；若未来解冻，不兼容模型仍必须使用独立环境和版本化消息协议隔离。

### 3.4 SQLite 与本地文件

- SQLite 当前保存 meeting、document、transcript、Summary run/snapshot/evidence、删除状态，以及 schema v5 的最小 `provider_consent_preferences` remembered Consent 当前偏好。
- 当前上传文档保存在应用私有数据目录，数据库只保存受控相对路径和哈希。计划中的录音文件转录必须复用同一 Rust 受控存储边界，不得被描述为已经实现。
- 凭据不进入 SQLite、`.env`、环境变量、日志、Python 或命令行参数。DeepSeek key 保存在 Windows Credential Manager，只能由受信任 Rust 代码通过 `SecretStore` 读取。
- 用户源文件保持不变；MeetingLens 使用 document UUID 生成内部文件名，不把源绝对路径或原始文件名用于存储路径拼接。
- 文件导入通过应用私有 staging 目录流式复制并计算 SHA-256，校验成功后在同一卷原子 rename；失败时清理临时文件。
- SQLite schema v2 已新增 `documents` 与统一 `deletion_jobs`；应用持有 `meeting-data/.meetinglens-storage.lock` 的生命周期独占锁，第二个写入实例 fail closed，进程内 mutation mutex 串行化导入、删除与恢复。
- schema v3 已由 PR #18 引入 `transcript_sources` 与 `transcript_segments`。粘贴文本和 TXT/SRT/VTT 使用轻量同步解析器进入 SQLite，不经过 DocumentService、Docling 或 Python Sidecar；原始转录文件首版不长期复制。
- PR #21 已通过 `feat/summary-run-core` 追加 schema v4：`summaries` 保存 run/series/revision 与状态，`summary_sources` 保存真实复制的 transcript snapshot 正文和元数据，`evidence_items` 只保存当前 run 实际引用的 transcript evidence。snapshot source/segment UUID 是历史值而非 live FK，因此终态 source 删除不会级联破坏已有结果。
- schema v5 只追加 `provider_consent_preferences(provider, policy_version, policy_fingerprint, consented_at, updated_at)`；一次性 Consent 不写数据库，按 run 在 Rust 内存中保留 10 分钟并单次消费。撤销删除 remembered 行并清空全部未消费一次性授权。

### 3.5 外部 AI 适配器

- DeepSeek 是首个外部 Summary 适配器；实时翻译 Provider 为 Deferred。production identity 固定为 `deepseek/external/deepseek-v4-pro/summary-deepseek-v1`，WebView 不能选择 Provider、model、host 或 URL。
- 适配器由 Rust `SummaryProvider` 实现，只接收用户确认的不可变最小快照范围；API key 只在执行前由 Rust 从 SecretStore 读回，WebView 只能 set/status/delete，key 不进入 SQLite、日志、Python 或命令行。
- Rust transport 固定 `https://api.deepseek.com/chat/completions`，使用 rustls，禁止重定向和环境 proxy，连接/总超时分别为 10/180 秒，请求体上限 2 MiB、响应体上限 1 MiB；非流式、thinking disabled、JSON object、单次请求且不自动重试。
- Provider 严格解析固定 model/finish reason/JSON schema、大小/数量和 evidence allowlist；失败与取消不提交部分结果。真实付费 API smoke 未执行时，只能报告 fake transport/协议路径已验证。

### 3.6 Provider 契约与 Runtime

MVP 1A-3A 在 React/TypeScript 应用边界定义五类能力契约：Capture、Transcription、Translation、Summary 和 DocumentParser。每个契约明确输入、输出、统一状态、稳定错误、取消和清理行为；页面只通过应用拥有的 `ProviderRegistry` 获取能力，不直接实例化第三方实现。

TypeScript Provider Registry 继续只有确定性的纯内存 Mock，用于页面能力预览，不是持久 Summary run 的执行者。Rust `SummaryProvider` trait 同时承载 production `DeepSeekSummaryProvider` 和仅测试/debug 的 `DeterministicMockSummaryProvider`；二者只接收从 schema v4 snapshot 重建的有界 DTO，不自行改变 run 状态。Summary service 独占快照、状态转换、输出/evidence 校验和原子持久化；WebView 不能提交 Provider 结果。

`TranscriptionSegment` 契约和 schema v3 已统一使用可空 `startOffsetMs/endOffsetMs`，并在 source 上使用可空 `baseStartedAt`；SRT/VTT 与计划中的录音文件批量转录都归一化为相对毫秒，没有时间戳时以稳定 ordinal 定位证据，不保存两套竞争的时间真相。现有内存 Mock 与持久化 DTO 使用同一时间语义。

Capture、Transcription、Translation 和 DocumentParser 的预览契约仍主要位于 TypeScript；持久 Summary run 已使用独立 Rust `SummaryProvider` trait。Rust 继续负责系统权限、Tauri command、数据库、进程、SecretStore 和外部 Summary 调用，Python 继续只提供现有 Sidecar 健康契约或受控的一次性 Worker。真实实现出现时再在所属层增加版本化 adapter，不为形式对称提前复制无调用方的接口。

替换关系继续保持清晰：DeepSeek 只实现 Rust Summary Provider；Docling 只实现 DocumentParser Worker；faster-whisper 在录音文件阶段只实现批量 Transcription Worker。Screenpipe、实时 Translation、Diart/WhisperX 和完整 Sidecar supervisor 均为 Deferred。会话认证由 MVP 1A-3B-1 建立，MVP 1A-3B-2 建立的 SecretStore 现在只由受信任 Rust DeepSeek 执行路径消费。

## 4. 信任边界

| 边界                     | 高信任侧           | 低信任侧         | 主要控制                                                                        |
| ------------------------ | ------------------ | ---------------- | ------------------------------------------------------------------------------- |
| WebView → Rust           | Rust core          | UI 输入          | Tauri capabilities、command allowlist、结构校验                                 |
| WebView → SecretStore    | Rust core          | API key 输入     | `main` 窗口 permission set、SecretKind 白名单、无读回 command                   |
| Native picker → Rust     | Rust core          | 不可信源文件     | allowlist、签名/类型、配额、UUID 路径和流式复制                                 |
| Transcript input → Rust  | Rust core          | 粘贴/TXT/SRT/VTT | 严格 UTF-8、大小/片段上限、控制字符拒绝、同步解析、无执行/网络                  |
| Rust → Sidecar           | Rust session owner | 本地 HTTP 进程   | loopback、临时 bearer token、集中 Header、超时                                  |
| Rust → Document Worker   | Rust coordinator   | 不可信文档       | 隔离进程、受控相对引用、取消、资源和输出限制                                    |
| Audio device → Rust      | Rust core          | 麦克风/系统音频  | Deferred；未来仍须手动启停、来源指示、句柄回收、可选保存                        |
| Window/screen → Rust     | Rust core          | 屏幕像素/结构树  | 可选阶段；单应用优先、目标身份复核、本地变化检测、临时帧删除                    |
| Rust Provider → DeepSeek | Rust core          | 外部服务         | 固定 host/model、明确确认、最小快照、SecretStore、TLS、无重定向/代理、超时/取消 |

## 5. Sidecar API

### 5.1 Phase 0 健康检查

`GET /health`

```json
{
  "status": "ok",
  "service": "meetinglens-sidecar",
  "version": "0.1.0",
  "api_version": "v1"
}
```

约束：

- 默认绑定 `127.0.0.1`。
- 返回值不包含主机名、路径、环境变量、模型列表或凭据信息。
- 非法路径返回 JSON 404。
- 所有端点（包括 `/health`）都要求每次启动生成的 bearer token。

Phase 0 调用链：

```mermaid
sequenceDiagram
    participant UI as React
    participant Rust as Tauri command
    participant Sidecar as Python 3.12 sidecar
    Rust->>Sidecar: 启动子进程，环境传递临时 token
    UI->>Rust: check_sidecar_health()
    Rust->>Sidecar: GET /health + Authorization: Bearer [REDACTED]
    Sidecar-->>Rust: versioned JSON
    Rust->>Rust: status/service/api_version validation
    Rust-->>UI: typed health or stable error code
```

Rust 使用 1 秒连接超时、2 秒总超时、禁止重定向，并且不把底层网络错误、路径、Header 或响应正文回显给 WebView。MVP 1A-3B-1 保留固定开发端口，但锁死 `127.0.0.1` 并启用每次启动轮换的高熵 token；随机端口、打包二进制和完整 supervisor 当前为 Deferred。

### 5.2 Deferred 端点

原规划的通用 transcribe/translate/diarize/summarize HTTP job 栈不属于当前 Personal Edition 路线。近期录音文件转录和 Docling 解析均由 Rust 启动一次性隔离 Worker，真实 Summary 则由 Rust Provider 直接调用 DeepSeek；任何未来本地任务协议仍只能传受控 artifact ID 或应用私有目录中的相对引用，不接受任意绝对路径。

## 6. 关键数据流

### 6.1 录音文件批量转录（计划）

```mermaid
sequenceDiagram
    participant User
    participant UI
    participant Rust
    participant Worker as One-shot faster-whisper Worker
    participant DB as Existing transcript model
    User->>UI: 主动导入会议录音文件
    UI->>Rust: 请求原生选择和批量转录
    Rust->>Rust: 类型、大小、路径和受控存储校验
    Rust->>Worker: 单次 CPU int8 转录任务
    Worker-->>Rust: 有界、版本化 segment 输出
    Rust->>DB: 写入 transcript_sources/transcript_segments
    Rust-->>UI: 进入现有 Transcript Workspace
```

该阶段不实现实时音频、WASAPI、实时字幕或实时 diarization。Worker 不监听端口、不接收凭据，并沿用 Docling Worker 的进程、资源、路径和输出校验边界。

### 6.2 Summary run 与外部摘要

1. Rust 验证 meeting、按用户给定顺序排列的 transcript sources、模板和输出语言；资格允许 `draft/recording/ready`，但拒绝删除态、空输入和无法安全冻结的输入。
2. 单个事务创建 series/revision 和 queued run，并实际复制最多 20,000 个 final segments、合计最多 10 MiB UTF-8 正文及 source/segment/global ordinal、speaker、offset、language、confidence 和快照时间。Provider 运行期间不再读取 live segment。
3. production run 固定为 DeepSeek policy。Rust 先返回不含正文的 disclosure；用户可选择本次同意、记住选择或取消。本次同意在内存中按 run 保留 10 分钟并单次消费，记住选择只保存 policy fingerprint/时间，撤销同时清除两类未使用授权。没有有效确认、没有 key、发送范围为空或超过 1 MiB DeepSeek 上限时均在网络前 fail closed。
4. `DeepSeekSummaryProvider` 只在执行前从 SecretStore 取 key，并从上述 snapshot 构造固定 JSON request；运行中取消通过 active-provider registry 唤醒并丢弃 HTTP future。历史 Mock 结果仍可读取，确定性 Mock 只通过测试/debug 注入运行，release WebView 不可选择。
5. 每条 snapshot row 的 UUID 构成 run-local evidence allowlist。Rust 拒绝未知、重复、跨 run ID 及超出版本化 schema/大小/数量边界的输出；WebView 不能提交 Provider 输出或 evidence。
6. 校验通过后，结构化 `content_json` 与实际引用的 evidence 在同一事务写入并转为 completed；失败写稳定安全错误，取消丢弃未提交结果，不产生部分 evidence。
7. 重新生成创建同一 `series_id` 的下一 revision 并重新冻结当前输入，不覆盖旧 run。segment revision 漂移标 stale；启动恢复把遗留 running 标为 `failed + summary_interrupted`，不自动重放 Provider。

上述最小 Consent 只适用于单人自用的 Personal Edition，不足以作为向团队、客户或公众分发时的企业合规方案。

正式 Summary 页面仍未实现；集中式 TypeScript `summaryApi` 是 WebView 的唯一调用入口，并固定为十二个方法：既有八个 policy/run/evidence commands，加上 `get_deepseek_provider_status`、`get_summary_external_disclosure`、`confirm_summary_external_disclosure` 和 `revoke_deepseek_consent`。所有 wire DTO（含嵌套结果）统一 camelCase 并严格解析；run/evidence 列表使用默认 50、最大 100 的 keyset 分页。权限集只把四个新 command 授予本地 `main` 窗口，不增加通用 HTTP、dialog、文件系统、shell 或 process 能力。

### 6.3 Meeting command 边界

MVP 1A-2-1 先建立独立的 Meeting Tauri Command API，再由 MVP 1A-2-2 的页面使用。WebView 只调用集中封装的 TypeScript `meetingApi`；Tauri command 负责 DTO 校验和稳定错误映射，再调用 Rust service，command 内不得包含 SQL。

创建会议后状态为 `draft`。`start_meeting` 执行 `draft → recording`，`end_meeting` 在合成会议阶段执行 `recording → ready`；两个生命周期时间戳均由 Rust 以 UTC ISO 8601 写入。`processing` 保留给后续受信任的异步处理流程，普通更新 command 不得修改状态或时间戳。非法转换返回 `meeting_invalid_state`，不得泄漏 SQL、路径或底层数据库错误。

`delete_meeting` 复用 service/deletion coordinator，不在 command 层访问文件系统。`draft`/`ready` 接受请求并持久化 `deleting` tombstone；`recording`、`processing`、`failed`、`archived` 拒绝；`deleting` 返回 `meeting_delete_pending`；`delete_failed` 可重试。文件和删除任务清理完成后才删除数据库记录，失败返回 `meeting_delete_failed`；不存在的会议返回 `meeting_not_found`。

### 6.4 SecretStore command 边界

`SecretKind` 当前只允许 `deepseek_api_key`，并由 Rust 固定映射为 `com.gray.meetinglens/secret/deepseek-api-key`。Windows 后端直接使用 Credential Manager Generic Credential；测试使用随机隔离 target，非 Windows 平台返回 unsupported/unavailable，绝不回退到文件、SQLite 或浏览器存储。

前端只暴露 `get_secret_status`、`set_secret` 和 `delete_secret`。`get_for_backend` 只供受信任 Rust DeepSeek Provider 在执行前使用，不注册为 Tauri command，也不把 key 传给 Python Sidecar。Secret response 只包含配置状态、可用性和固定后端标识，不包含值、target、路径或原生错误。

### 6.5 安全文档存储核心与原生导入边界

MVP 1B-1A 提供受信任 Rust 调用栈复用的 `DocumentService`；MVP 1B-1B 在其上注册六个最小 Document Tauri Commands，并由集中、可 mock 的 TypeScript `documentApi` 使用。文件入口只有 Rust 打开的 MeetingLens 原生文件选择器：React 只发送 meeting/document ID，不传入路径，也不取得对话框或任意文件系统权限；选择结果和源绝对路径只在 Rust 导入调用期间使用，不写入数据库，也不返回 WebView。

`select_and_import_documents` 使用有类型的 Tauri `Channel` 返回每个文件的安全阶段、已处理字节与总字节；终态 response DTO 只包含 document ID、清理后的显示名、检测类型、大小、状态、UTC 时间和稳定错误码。其他 commands 提供 list/get/delete/retry 和固定导入策略。Command 层不包含 SQL、复制或路径拼接，所有最终状态、类型、配额、去重和删除规则仍由 `DocumentService` 执行。

现有 TypeScript `DocumentParserProvider`/`DocumentReference` 和 `MockDocumentParserProvider` 只是 MVP 1A-3A 的纯内存契约，不是文件导入 API，也不授权 WebView 读取 `artifactId` 对应的文件。Mock 中 10,000,000 bytes、255 characters 和少量 MIME 的合成校验不是生产 allowlist/配额；生产边界以 `PRODUCT_SPEC.md` 的 100 MiB、UTF-8 byte 限制和完整 allowlist 为准。PR #14 的 Rust Worker client 只接受受控 staging 根目录内且 size/SHA-256 已注册的私有输入；`feat/document-parsing-orchestration` 再由 document ID 编排该边界。

### 6.6 隔离 Docling Worker 基础

PR #14 将文档解析器放在 `workers/docling` 的独立 Python 3.12/uv lock 环境，而不是现有 HTTP Sidecar。它不监听端口、不读取 SQLite、不注册 Tauri command，也不向 WebView 暴露文件或进程能力。Rust 每次请求启动一个 `python -I -m meetinglens_docling_worker` 子进程，清空继承环境后只加入离线运行所需 allowlist，并通过有界 JSON Lines stdin/stdout 完成一次 health、capabilities 或 parse 操作。

Windows 上子进程在写入请求前加入启用 `KILL_ON_JOB_CLOSE` 与内存上限的 Job Object；成功、失败、取消、150 秒超时或协议异常都会回收完整进程树。Worker 校验输入处于批准 staging 根、是普通文件且匹配已注册 size/SHA-256；固定输出 `document.json`、`content.md`、`content.txt` 和 `manifest.json` 到批准输出根，再由 Rust 重新校验相对路径、大小、总量与 SHA-256。任何绝对路径、UNC、ADS、遍历、未知产物或不匹配都 fail closed。

Docling 运行模式固定为 CPU-only、无远程服务、无插件、无隐式模型下载。模型只能通过显式脚本写入仓库外的版本化应用目录，manifest 固定 layout、TableFormer 和 RapidOCR 来源 revision 并列出每个文件的哈希。缺少、版本不兼容或被修改的模型会返回稳定错误；无模型时 TXT/Markdown/CSV/Office 仍可解析，PDF/图片/OCR fail closed。Worker Foundation 已由 PR #14 合并，但只是后续参考文档增强；简化的 `feat/document-parsing-orchestration` 将由 document ID 调用现有 Worker，成功后生成 document chunks/evidence，失败自动重试一次，再失败提示用户手动重新导入。它不建立通用任务调度器、多级恢复、长期 checkpoint 或企业监控面板，也不阻塞 transcript-only Summary。

### 6.7 转录输入与纪要快照边界

粘贴文本由 WebView 作为有界 UTF-8 DTO 交给 Rust；TXT/SRT/VTT 只能由 Rust 原生选择器选择，源绝对路径不返回 WebView、不写 SQLite。解析器只识别文本、明确 `姓名: 内容` 和标准 SRT/VTT cue，不执行 URL/代码/指令，不调用网络或 Docling，不猜测说话人或时间。

PR #19 已把上述边界暴露为集中式 `transcriptApi`：原子创建 draft meeting + transcript、向既有 meeting 添加、source/segment 查询、revision 修正和 source 删除。`feat/summary-mode-input-ui` 在该 API 上增加的最小分页仍由 Rust/SQLite 按稳定 ordinal 执行，production 单页不超过 200；React 不先读取全部 20,000 条。source-level 说话人命名只原子更新匹配 segment 的 `speaker_name`、revision 与 `corrected_at`，保留 `speaker_label`，不建立身份档案。

```mermaid
sequenceDiagram
    participant User as 用户
    participant UI as React
    participant Rust as Tauri/Rust
    participant DB as SQLite schema v5
    participant Summary as Summary Provider
    User->>UI: 粘贴文本或请求原生导入
    UI->>Rust: bounded transcript request
    Rust->>Rust: UTF-8/控制字符/大小/格式校验与同步解析
    Rust->>DB: transcript source + ordered segments
    Rust-->>UI: 无路径的解析预览
    User->>UI: 选择来源、模板和输出语言
    UI->>Rust: 仅提交 meeting/source IDs 与安全选项
    Rust->>DB: 原子创建 run 并复制完整正文与显示元数据快照
    Rust-->>UI: 固定 Provider/host/model 与发送类别/计数
    User->>UI: 本次同意、记住选择或取消
    UI->>Rust: 仅 summaryId + remember
    Rust->>Summary: 已授权、有界、不可变的 transcript snapshot DTO
    Summary->>Summary: Rust 从 SecretStore 取 Key 并请求固定 DeepSeek endpoint
    Summary-->>Rust: 带 allowlisted evidence IDs 的严格 JSON 结果
    Rust->>DB: 原子提交 content_json 与实际引用的 evidence
```

上图展示当前底层实现边界；现有“生成会议纪要”按钮仍保持禁用，因此普通页面不会执行外发。可复用 Consent Dialog、Settings 状态/撤销和 Rust DeepSeek 管线已完成；正式入口、结果页面、正文修改、整份重新生成和 Markdown 复制/保存由下一阶段统一接线。

转录 source 删除会处理其 segments，但不影响用户原始文件。queued/running Summary run 使用该 source 时删除稳定拒绝 `transcript_source_in_use`；terminal run 不阻塞删除。删除事务保留已实际引用的 evidence excerpt、speaker、offset/ordinal 和 revision并标 `source_deleted`，清理 `summary_sources` 中完整正文与非必要显示元数据，再删除 live source/segments；结果不会被静默改写。segment 修正增加 revision，并把依赖旧输入的 run 标为 `segment_revised` stale；运行中的 Mock 仍完成旧快照，后台不得静默重新生成。

```mermaid
sequenceDiagram
    participant User as 用户
    participant UI as React
    participant Rust as Tauri/Rust
    participant Stage as Private staging
    participant Store as Meeting documents
    participant DB as SQLite
    User->>UI: 主动选择文件
    UI->>Rust: 请求原生文件选择
    Rust->>Rust: 来源、数量、文件名、扩展和初步大小检查
    Rust->>DB: 创建 importing 元数据
    Rust->>Stage: 随机临时文件 + 流式复制 + SHA-256
    Rust->>Rust: 签名/安全类型、最终大小和会议累计配额
    Rust->>Store: 同卷原子 rename 到 UUID 文件名
    Rust->>DB: 转换为 staged；失败则进入 import_failed
    Rust-->>UI: 安全文档 DTO（无绝对路径）
```

允许格式和配额以 `PRODUCT_SPEC.md` 为准。扩展名不是唯一判断依据；PDF/PNG/JPEG 使用签名识别，文本格式需拒绝二进制内容，OOXML 只做容器/类型基础识别且在导入阶段不解压。完整 OOXML 结构校验和内容解析留给隔离 Docling Worker。类型不一致、空文件、双重扩展、未知格式或超额输入全部 fail closed。

同一会议内以 SHA-256 识别完全相同的内容：重复导入不创建第二条记录或物理副本，而是返回已有 document ID 和稳定结果。不同会议首版不做物理去重；哈希不代表恶意软件检测。

### 6.8 文件删除协调

MVP 1B-1A 已将同步硬删除演进为持久、幂等、可恢复的协调流程：

```mermaid
stateDiagram-v2
    [*] --> deleting: 接受删除请求并持久化 tombstone
    deleting --> deletionTrash: 同卷移动受控文件
    deletionTrash --> [*]: 清理文件、派生数据和数据库记录
    deleting --> delete_failed: 移动或清理失败
    delete_failed --> deleting: 用户或启动恢复重试
```

接受会议删除后先把该会议 queued/running Summary runs 原子转为 cancelled，再进入 `deleting`，冻结编辑、开始、文件集合和新纪要任务。协调器枚举原始文档、schema v4 summaries/snapshots/evidence 及未来解析、音频、截图、转写和导出产物，将文件安全移动到应用私有 `deletion-trash`（或实现等价的可恢复边界），成功后才最终删除数据库记录。失败进入 `delete_failed`、保留安全错误码并允许重试；重复请求返回稳定 pending 状态。不得先删数据库再尽力删文件，也不得先删文件再尝试删除数据库。

单文档删除使用同样模式：先进入 `deleting`，成功清理文件和 artifacts 后才移除元数据，失败进入 `delete_failed`。处于 `importing`/`parsing` 的文档必须先取消或等待安全终止。删除永远不影响用户源文件。

## 7. 仓库结构

```text
MeetingLens/
├── src/                       # React + TypeScript UI
│   └── providers/             # Provider 契约、Registry 与纯内存 Mock
├── src-tauri/                 # Tauri/Rust core、data/document service 与 capabilities
│   └── tests/                 # Windows 原生存储 smoke harness（合成文件）
├── sidecar/
│   ├── meetinglens_sidecar/   # Python 服务
│   └── tests/                 # Sidecar 自动化测试
├── workers/docling/           # 独立离线解析 Worker、锁文件与测试
├── scripts/                   # Worker 环境/模型显式准备脚本
├── .github/workflows/         # CI
├── PRODUCT_SPEC.md
├── ARCHITECTURE.md
├── DATA_MODEL.md
├── SECURITY.md
└── DEVELOPMENT_PLAN.md
```

## 8. 进程与打包策略

1. MVP 1A-3B-1 开始，`npm run tauri dev` 由 Rust 启动 Python 3.12 Sidecar，并通过子进程环境传递当前会话 token；WebView 不启动进程也不持有 token。
2. 独立启动仅用于受控协议测试，缺少父进程提供的 token 时必须 fail closed。
3. Docling 已使用 Rust 启动的一次性隔离 Worker；计划中的录音文件转录复用同一受控进程模式，不扩展常驻实时 Sidecar。
4. Rust 使用固定程序/module allowlist 启动进程，不允许 WebView 获得任意 shell 执行能力。
5. 当前只在正常窗口/应用退出时终止所拥有的 Sidecar 子进程；完整实时 supervisor、崩溃重启和孤儿进程回收为 Deferred。
6. 安装包完善、代码签名、SBOM、自动更新、托盘和商业分发为 Frozen / Deferred；已有构建能力保留，但不是 Personal Edition 日常可用的前置条件。

Phase 0 未写入 `externalBin`，因为仓库尚无可验证的平台二进制。未来若解冻分发能力，必须先产生并验证 Windows 二进制，再接入打包配置。

## 9. 可观测性

- 结构化日志只记录事件类型、任务 ID、耗时、状态码和匿名错误分类。
- 默认不记录转录正文、提示词正文、文件名、参会者、密钥或外部响应全文。
- UI 展示面向用户的错误；开发日志使用 request/correlation ID 串联 Rust 和 sidecar。
- 健康指标包括队列长度、任务耗时、重试次数和 sidecar 重启次数。

## 10. 架构决策

- **Tauri 2 + React + TypeScript**：小型桌面壳、明确 IPC 边界、前端生态成熟。
- **Python sidecar**：便于接入语音和文档处理生态，与 UI/Rust 生命周期解耦。
- **SQLite**：单机事务和迁移简单，适合本地优先元数据。
- **本地 HTTP + 会话令牌**：便于 Python 服务化测试，同时限制未授权本机进程访问。
- **Provider adapter**：避免业务逻辑与 DeepSeek 单点绑定。

### 10.1 实施顺序与模块所有权

| 状态             | 阶段                                      | 架构交付                                                                                                    |
| ---------------- | ----------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Implemented      | 已完成基础与 PR #16                       | meeting、Sidecar auth、SecretStore、安全文件导入、隔离 Docling Worker、zh-CN/en-US                          |
| Implemented      | PR #18–#20                                | schema v3、Transcript commands、粘贴/TXT/SRT/VTT、转录工作区、分页与 speaker 修正                           |
| Implemented      | `feat/summary-run-core`（PR #21）         | schema v4、run/series/revision、不可变正文快照、evidence allowlist、stale/cancel/recovery、Rust 零网络 Mock |
| Implemented      | `docs/personal-edition-descope`（PR #23） | Personal Edition 定位、冻结项、简化流程和风险分级验收                                                       |
| Current delivery | `feat/summary-provider-deepseek`          | schema v5、Rust DeepSeek、SecretStore、一次性/记住/撤销、严格 JSON/evidence、安全错误与取消                 |
| Next             | `feat/summary-mode-minutes`               | 正式生成/Consent 接线、结果页、模板/语言、正文修改、整份重新生成、Markdown 查看/复制/保存                   |
| Planned          | `feat/audio-file-transcription`           | 录音文件导入、本地 faster-whisper CPU int8 一次性 Worker、写入既有 Transcript Workspace                     |
| Planned          | `feat/document-parsing-orchestration`     | 现有 Docling Worker 接线、document chunks/evidence、失败重试一次，再失败提示重新导入                        |
| Optional         | `feat/window-context`                     | 仅在用户确认需要时实现明确窗口选择、身份复核和默认不保存帧                                                  |
| Frozen/Deferred  | 实时与商业产品路线                        | 实时采集/ASR/翻译/diarization、checkpoint、翻译浮窗、独立会议工作区、structured visuals、DOCX/PDF、商业分发 |

一个 PR 仍只完成一个明确用户能力；当数据模型和边界已经清晰时，可以在同一功能 PR 中交付必要的数据层、Rust Commands、TypeScript API 和 UI。schema、安全或真实网络变更必须重点审查，上游阶段不得夹带无关下游能力。

## 11. 依赖兼容性与采用门槛

以下结论基于 2026-07-31 的官方文档、包元数据和锁定依赖。标为已采用的依赖已经通过 Windows 本机编译与测试；其余候选仍须在所属阶段复核。

| 组件            | 候选基线                               | 结论与门槛                                                                                             |
| --------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Tauri / Rust    | Tauri `2.11.5`，Tauri build `2.6.3`    | Phase 0 已精确锁定；Windows CI 执行 fmt/test/check。                                                   |
| WinCred binding | `windows 0.61.3`                       | 已采用；复用 Tauri lock graph 版本，MIT/Apache-2.0，MSRV 1.74，只启用 Foundation/Credentials。         |
| Secret memory   | `zeroize 1.9.0`                        | 已采用；无运行时依赖，MIT/Apache-2.0，MSRV 1.85，当前 Rust 1.97.1 满足。                               |
| Document hash   | `sha2 0.11.0`                          | 已采用；纯 RustCrypto 实现，MIT/Apache-2.0；只用于完整性与同会议去重。                                 |
| OOXML container | `zip 8.6.0`，关闭默认 features         | 已采用；MIT、MSRV 1.88；仅有界读取中央目录元数据，不解压 payload，也不启用压缩后端。                   |
| Storage lock    | Rust `std::fs::File::try_lock`         | 已采用；Rust 1.89 稳定 API，Windows 使用原生文件锁，无新增依赖；第二写入实例 fail closed。             |
| Native dialog   | `tauri-plugin-dialog 2.7.2`            | 已采用并精确锁定；MIT/Apache-2.0。只从 Rust 打开选择器，WebView 未获得 dialog/fs permission。          |
| Python          | `3.12.x`                               | 项目统一基线；业务 sidecar 后续不得静默退回其他版本。                                                  |
| DeepSeek        | `deepseek-v4-pro` / `api.deepseek.com` | Rust Provider 已固定官方 host/model；真实付费 smoke 未执行，价格和数据条款仍须由用户在实际使用前确认。 |
| HTTP transport  | `reqwest 0.13.4` + rustls              | 已精确锁定；无重定向/环境代理、固定 HTTPS、流式有界读取、超时/取消、无自动重试。                       |
| docling-slim    | `2.117.0`                              | Python 3.12/Windows 独立环境；只启用所需格式、本地模型和 RapidOCR ONNX extras。                        |
| Diart           | `0.9.2`                                | 依赖 `numpy<2`，其参考环境偏向 Python 3.10，并建议旧版 pyannote；不能直接进入主 sidecar。              |
| WhisperX        | `3.8.6`                                | Python `>=3.10,<3.14`，但要求 `numpy>=2.1`、`pyannote-audio>=4`；与 Diart 当前依赖冲突。               |
| faster-whisper  | `1.2.1`                                | 可用 CPU int8；GPU 路径要求 CUDA 12/cuDNN 9 并受 CTranslate2 兼容性约束。                              |
| Screenpipe      | app `2.5.139`                          | 上游变化快；商业用途需要单独核验/取得许可。采集接入前必须完成授权与隐私复审。                          |

### 11.1 Worker 隔离决策

当前 Personal Edition 的近期 Worker 边界为：

```text
Tauri/Rust
├── audio-file transcription worker (planned faster-whisper CPU int8, one request per process)
└── document worker (Docling locked environment, PR #14 complete; product wiring planned)
```

Diart 的 `numpy<2` / 较旧 pyannote 约束与 WhisperX 的 `numpy>=2.1` / pyannote 4 约束不能在同一 Python 环境满足；该兼容性结论保留给未来可能解冻的实时 diarization/alignment，但不再驱动当前路线。Worker 之间只交换受控文件引用和版本化 JSON，不共享 site-packages；每个已采用 Worker 必须有独立 lockfile、许可清单、资源上限和合成数据测试。

### 11.2 延后决策

- Screenpipe 只能作为可替换的 Capture Provider；窗口上下文仅是可选阶段，核心窗口选择、权限、事件模型和删除语义不得依赖它。采用前需用户决定商业授权、采集范围和 Windows 资源预算。
- WhisperX/Diart 与实时 diarization 均为 Deferred；若未来解冻，模型条款、token 和相互隔离环境仍须单独复核，token 不得写入配置或仓库。
- Docling 进程基础已由 PR #14 引入；简化的解析持久化、document chunks 和 evidence 在 `feat/audio-file-transcription` 之后由一个 `feat/document-parsing-orchestration` 功能 PR 接入。第一份 transcript-only 纪要由既有 Summary Run Core、已实现的 Rust DeepSeek/最小 Consent 和下一阶段 Minutes 页面完成，不依赖 Docling。不得在同一 PR 混入无关的转录持久化、外部 AI 和文档解析信任边界。

### 11.3 Windows 本地验收门槛

CI 负责自动化构建与测试，但不能证明 WebView2 窗口、凭据库、文件对话框、权限或进程回收在用户机器上可用。验收按风险分级：文档 PR 运行 Markdown/格式、链接、路线一致性和 CI；不修改 schema 的普通功能 PR 默认一次本机启动、主要流程和相关自动测试；修改 schema、migration、安全边界或真实网络的高风险 PR 继续执行升级、integrity/foreign-key、必要的隔离或双启动、故障恢复和敏感信息/网络边界验证。

PR #21 的 Summary Run Core 已使用 development-only 双启动隔离 harness，设置 `VITE_SUMMARY_RUN_CORE_SMOKE=1`、`MEETINGLENS_SUMMARY_RUN_CORE_SMOKE_ROOT`、`VITE_SUMMARY_RUN_CORE_SMOKE_ISOLATED=1` 和匹配的合成 identifier，验证了零应用外发、浏览器存储无正文、SQLite 完整性、恢复与最终清理。这是已完成高风险阶段的历史验收证据，不是以后所有普通 UI PR 的默认矩阵。

## 12. 参考

- [Tauri 2 创建项目](https://v2.tauri.app/start/create-project/)
- [Tauri sidecar 指南](https://v2.tauri.app/develop/sidecar/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri security](https://v2.tauri.app/security/)
- [Windows Credential Manager APIs](https://learn.microsoft.com/en-us/windows/win32/api/wincred/)
- [Rust `windows` bindings](https://github.com/microsoft/windows-rs)
- [RustCrypto `zeroize`](https://docs.rs/zeroize/1.9.0/zeroize/)
- [DeepSeek 模型与定价](https://api-docs.deepseek.com/quick_start/pricing/)
- [DeepSeek 模型列表](https://api-docs.deepseek.com/api/list-models)
- [Docling](https://github.com/docling-project/docling)
- [Diart](https://github.com/juanmc2005/diart)
- [WhisperX](https://github.com/m-bain/whisperX)
- [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
- [Screenpipe](https://github.com/screenpipe/screenpipe)
