# MeetingLens 产品规格

## 1. 产品定义

MeetingLens 是一款面向 Windows 11 的本地优先 AI 会议助手。第一核心目标是：让用户以最少步骤，将已有会议转录、粘贴文本、实时转写和相关会议材料转化为详实、可编辑、可追溯的会议纪要。

产品优先把“会上说了什么”转化成可核验的决定、行动项、风险和未决问题。转录是最重要的第一类来源；文档提供背景和证据增强，真实音频、屏幕上下文、说话人增强与实时翻译按价值顺序逐步接入，同时尽量减少会议数据离开本机。

当前三模式入口与用户体验以 `SIMPLIFIED_MODE_SPEC.md` 为准；本文件是产品能力、行为规则和验收边界的权威规格；实际实施顺序以 `DEVELOPMENT_PLAN.md` 为准。同一套会话引擎提供总结、翻译和会议三种模式，此前已经合并的工程与安全基础继续复用，旧版尚未开始的路线暂停。

## 2. 目标用户与边界

- 需要整理高频会议的个人、学生和小型团队
- 参加跨语言会议、需要实时或会后翻译的用户
- 需要提取决定、行动项、风险和未决问题的项目负责人
- 对会议隐私、数据驻留和删除能力有明确要求的用户

首版是单机桌面产品，不引入账号体系、团队后端或云端数据存储，也不以机器人身份加入 Zoom、Teams 或 Google Meet。

## 3. 产品原则

1. **明确同意**：录音、屏幕上下文和外部 AI 处理必须由用户主动开启。
2. **本地优先**：录音、上传文件、数据库和处理中间产物默认留在本机。
3. **可追溯**：摘要、决定和行动项能够定位到时间戳、转录片段、文档或屏幕证据。
4. **最小发送**：调用外部模型前只发送完成任务所需的最小文本，不一次发送完整会议数据。
5. **可恢复、可删除**：处理任务可独立失败和重试；用户可删除会议及全部本地派生数据。
6. **准确优先**：准确性优先于低延迟；实时结果可以在会后校正。
7. **依赖复用**：优先集成成熟组件，不自行重写转录、说话人分离、OCR 或文档解析算法。

## 4. 核心使用流程

### 4.1 首页快速入口

首页只提供三个主要模式入口和历史记录。用户不需要先理解 Provider、Docling、设备协议或内部状态机。

- **总结模式**：粘贴 UTF-8 文本或通过 Rust 原生选择器导入 TXT/SRT/VTT，预览规范化片段后生成纪要。
- **翻译模式**：明确开启音频后显示原文/译文字幕、匿名说话人标签和阶段摘要；真实能力在共享实时转录底座完成后开放。
- **会议模式**：在已经完成的实时转录、用户确认的屏幕/窗口上下文和可选翻译能力上，集成阶段记录与最终纪要；不重新实现这些共享底座。
- **历史记录**：统一查看三种模式产生的会议、逐字稿、摘要和导出结果。

“导入会议录制视频”和原始音频文件离线转写只作为未来增强，当前不显示为可用入口。参考材料仍通过独立 Document UI 管理，不与转录来源混淆。

### 4.2 生成会议纪要

1. 用户选择一个或多个可用来源；近期 transcript-only MVP 只消费转录来源。
2. 应用显示来源、片段数、时间范围、说话人、模板、输出语言和外部发送内容类型。
3. 用户确认后创建不可变输入快照；生成过程中后续编辑不改变本次 run。
4. DeepSeek 只接收用户确认的规范化文本和必要元数据，不接收本地路径、原始文件或凭据。
5. 结果按详细讨论、决定、行动项、未决问题、风险、争议和证据展示，并标记为可编辑草稿。
6. 重新生成创建新 revision，不静默覆盖用户编辑；输入变化可把已有纪要标记为 stale。

### 4.3 实时会议与后续增强

真实音频阶段由用户明确开始采集，界面持续显示麦克风、系统音频、保存原始音频和外部翻译的独立状态；实时 ASR 将片段写入与导入转录相同的数据模型。保留设置必须把“保存完整听录文本”（默认开启）、“保存原始会议音频”（默认关闭）和“保存屏幕关键帧”（默认关闭、仅在屏幕增强可用时显示）作为三个独立选项。屏幕、说话人和翻译均为可选增强，失败不得阻塞本地转录、人工记录或会后纪要。

阶段摘要首版只允许用户手动触发，或每 5 分钟在存在新最终片段时触发。最终纪要必须读取原始转录片段的不可变快照，不能只对阶段摘要再次总结。后续自动话题边界只能给出需要用户确认的建议，不得静默切段或写成最终决定；透明桌面浮窗和屏幕变化判断同样属于后续独立增强。

## 5. 功能范围

### 5.1 Phase 0：规格与技术基线（已完成）

- 完成产品、架构、数据、安全、开发计划与 Agent 工作流文档
- 验证关键依赖、授权和 Python 兼容性，不安装重型 AI 组件
- Tauri 2 + React + TypeScript + Vite 最小脚手架
- Python 3.12 最小 sidecar 与 `/health`
- React 经 Tauri command 调用 Rust，由 Rust 请求 sidecar `/health`
- 单元测试、lint、formatter、type check、build 和 GitHub Actions

Phase 0 已通过 PR #1 合并，并在 Windows 本机实际启动 Tauri 窗口。该阶段没有实现录音、转录、上传解析、DeepSeek 调用、数据库业务表或屏幕采集。

### 5.2 MVP 1A：数据、模拟 UI 与安全基础

- **MVP 1A-1（已完成）**：SQLite、迁移、会议 CRUD、删除语义和持久化，不做 UI 或 Provider；由 PR #3 交付。
- **MVP 1A-2-1（已完成）**：Meeting Tauri Command API、稳定 DTO/错误和可 mock 的 TypeScript 客户端，由 PR #6 交付。
- **MVP 1A-2-2（已完成）**：桌面页面、模拟转写和完整模拟会议流程，由 PR #7 交付。
- **MVP 1A-3A（已完成）**：Capture、Transcription、Translation、Summary、DocumentParser Provider 契约、Registry 与纯内存 Mock，由 PR #8 交付。
- **MVP 1A-3B-1（已完成）**：本地 Sidecar 会话令牌、Loopback 认证、稳定错误和最小日志脱敏，由 PR #9 交付。
- **MVP 1A-3B-2（已完成）**：Windows Credential Manager `SecretStore`、凭据配置状态和跨层安全日志基础，由 PR #10 交付。

合成会议的产品状态固定为：创建后 `draft`，开始后 `recording`，结束后 `ready`。开始/结束时间由后端生成；`processing` 保留给未来真实异步处理，用户界面不能任意写入状态。

当前阶段只允许删除尚未开始的 `draft` 或已正常结束的 `ready` 会议。`recording` 必须先结束再删除；其他保留状态在后续阶段定义清理语义前同样不可删除。

### 5.3 简约版近期主线：先完成总结模式

PR #1–#21 已合并。文档安全存储、原生导入 UI、Docling Worker Foundation、转录/纪要规格、zh-CN/en-US 界面、Help 页面、双语开发预览手册、Transcript Data Core、安全 Transcript Command API、Summary Mode Input UI 和 Summary Run Core 已完成；PR #21 已交付 schema v4、不可变正文快照、run/series/revision、Rust zero-network Mock 与 evidence allowlist。Docling 具备独立锁定环境、显式离线模型、stdio 协议、Windows Job Object、超时/取消和多格式本机 Smoke，但尚未接入 SQLite、Provider、Commands 或 UI。下一阶段是 `feat/consent-data-core`。DeepSeek key 可以安全保存但没有真实 Provider 消费，普通用户生成入口仍禁用，真实 AI 纪要和正式结果 UI 仍不可用。阶段名和分支名是未来计划的权威标识；PR 编号只记录实际历史。

当前顺序不再使用未来 PR 编号作为计划契约：

1. **`docs/transcript-and-summary-spec`（已完成，PR #15）**：冻结转录、输入快照和可追溯纪要规格。
2. **`feat/i18n-user-guide-foundation`（已完成，PR #16）**：打包 zh-CN/en-US、界面语言偏好、Help 与双语开发预览手册；后续产品文案必须同步维护。
3. **`docs/simplified-three-mode-spec`（已完成，PR #17）**：冻结三个入口、共享管线、隐私边界和新顺序，不实现功能。
4. **`feat/transcript-data-core`（已完成，PR #18）**：schema v3、`transcript_sources`/`transcript_segments`、粘贴/TXT/SRT/VTT 同步解析、去重、修正和删除语义；不做 UI 或外部 Provider。
5. **`feat/transcript-command-api`（已完成，PR #19）**：安全 Transcript Commands/API、Rust 原生选择器和原子 meeting + transcript 工作流；不实现正式页面。
6. **`feat/summary-mode-input-ui`（已完成，PR #20）**：总结模式入口、粘贴与原生导入、来源和分页片段预览、基础说话人/正文修正；只消费已合并 API，不调用 DeepSeek。
7. **`feat/summary-run-core`（已完成，PR #21）**：schema v4、Summary run/series/revision、完整 transcript snapshot、evidence allowlist、稳定状态/恢复与 Rust zero-network Mock；不接外部 Provider 或结果 UI。
8. **`feat/consent-data-core`（下一阶段）**：通用 `consent_events`、会话失效和审计 API；不调用 Provider、不访问设备。
9. **`feat/summary-provider-deepseek`**：Rust SecretStore 消费、外发同意和受限 DeepSeek adapter；不新增 schema 或正式页面。
10. **`feat/summary-mode-minutes`**：选择输入/模板、外发预览、生成状态、结果、证据和 Markdown 复制/下载；不新增数据库或 Provider。
11. **`feat/summary-editing-export`**：用户编辑、summary revision、stale、章节重生成、DOCX 和历史版本，完成首个 transcript-only 总结闭环。
12. **`feat/document-parsing-orchestration`**：接入已完成的 Docling Worker、parse jobs、文档状态、artifact 原子提交、取消/重试/恢复；不实现音频。
13. **`feat/document-evidence-index`**：建立 document chunks 与页码/章节/表格定位，并把参考材料 evidence 接入既有 Summary pipeline；不重写 transcript pipeline。
14. **实时采集基础**：依次完成 `feat/live-session-data-core`、`feat/sidecar-supervisor` 和 `feat/audio-capture`；保存原始音频仍须独立同意且默认关闭。
15. **实时 ASR**：`feat/realtime-transcription` 直接写入既有 `transcript_sources/transcript_segments`，不建立第二套转录模型。
16. **屏幕与窗口上下文**：依次完成 `feat/window-context` 和 `feat/screen-ocr`；只读取用户确认的目标，屏幕关键帧默认不保存。
17. **说话人分离与命名**：先由 `feat/live-language-data-core` 建立会话级别名等共享数据，再由 `feat/speaker-diarization` 提供聚类、校准和人工修正；不承诺跨会议声纹复用。
18. **实时翻译**：依次完成 `feat/realtime-translation-provider`、`feat/checkpoint-summary-runtime`、`feat/topic-checkpoints` 和 `feat/translation-overlay`，覆盖可关闭翻译、阶段摘要建议和透明字幕交互。
19. **翻译/会议模式集成**：依次完成 `feat/translation-mode-workspace`、`feat/meeting-notes-core` 和 `feat/meeting-mode-workspace`；三种模式共享纪要、证据、历史和导出流程。

界面语言与纪要输出语言是两项独立设置：切换 zh-CN/en-US 只改变应用界面，不得自动改变当前或未来 Summary run 的中文、英文或双语输出选择。

Docling 是参考文档解析增强，不是 transcript-only 纪要的强制前置。转录证明会上说了什么；文档提供材料与背景，文档中写着某项计划不等于会议已经确认该计划。

#### 5.3.1 第一版转录输入与限制

近期只实现粘贴严格 UTF-8 文本，以及 TXT、SRT、VTT。普通文本按非空段落或明确发言行保序转换；仅识别明确的 `姓名: 内容`，不确定时 speaker 为空，不用大模型猜测。SRT/VTT 保留 cue 顺序、相对时间、原始语言和明确说话人标签，不自动翻译。

限制固定为：粘贴内容最多 2 MiB、转录文件最多 10 MiB、每个 source 最多 20,000 个 segments、单 segment 正文最多 64 KiB。输入必须为严格 UTF-8，拒绝 NUL 和除 TAB/LF/CR 外的危险 C0 控制字符。文本中的 URL、代码和指令只作为数据，不执行、不访问网络、不写入普通日志。

Summary run 的多来源聚合限制独立固定为：最多 20,000 个 final segments、规范化正文合计最多 10 MiB UTF-8 bytes。来源按用户选择顺序、片段按稳定 ordinal 冻结并产生全局 ordinal；空输入、重复/跨 meeting source、只有 partial segments 或任何超限都整体拒绝，不截断、不抽样。

普通用户检查页面不得一次读取或渲染 20,000 个 segments。列表由后端按稳定 `ordinal` 顺序分页，生产单页上限为 200，并返回总数和后续页信息；不得在前端先取得全量再切片。同一 source 内相同 `speaker_label` 可由用户统一设置或清空 `speaker_name`，每条受影响 segment 的 revision 必须递增且 `corrected_at` 更新；原始 label 不变，映射不跨 source/meeting，也不推断真实身份。

`platform_export` 是保留来源类型；Teams、Zoom、飞书、腾讯会议等平台专用 adapter 只有获得脱敏真实样例后才增加。首版不承诺 DOCX/CSV/JSON 平台模板、视频转录或音频文件转录。

解析后持久化规范化 segments、原始显示文件名、来源类型和内容 SHA-256，但不长期复制原始转录文件；源文件不被修改或删除。重新解析需要用户再次选择源文件。该哈希用于来源识别和同会议重复检测，不代表可信或防病毒。

当前 meeting 模型没有独立、可编辑的会议发生日期。总结模式只能将 `created_at` 明确展示为“创建时间”，不得把它冒充会议发生日期，也不得在没有数据模型与 migration 先行批准时提供无法持久化的日期输入。

#### 5.3.2 纪要资格、模板与证据

纪要资格取决于能否冻结至少一个用户选择的有效输入，而不是只允许 `ready`。架构允许 `draft/recording/ready` 搭配 transcript、manual notes、documents 或多来源混合，也允许 transcript-only、documents-only 和 manual-notes-only；近期 MVP 只实现 transcript 输入。`deleting`、`delete_failed`、没有输入或输入仍无法安全冻结时禁止。纪要任务拥有独立状态，不新增 `content_ready/source_ready/summarized` 等 meeting 状态。

schema v4 为每个新纪要系列生成 `series_id` 和 revision 1；显式重新生成在事务中创建下一 revision、重新冻结当前 live segment revisions，不覆盖旧 run。快照实际复制正文、source/segment IDs、revision、source/segment/global ordinal、speaker、offset、language、confidence 与时间，Provider 不得回读 live segment。当前状态机是 `queued → running → completed/failed/cancelled` 和 `queued → cancelled`；启动后遗留 running 变为 `failed + summary_interrupted`。

当前 Rust deterministic Mock 只消费本地快照，不访问 SecretStore、环境凭据、文件、Sidecar 或网络。每个 snapshot row 的 UUID 形成当前 run evidence allowlist；Rust 在原子写入结果前验证 output run ID、结构版本、大小/数量和全部 evidence，未知或跨 run 引用使 run 安全失败。只有结果实际引用的 transcript evidence 被持久化；document/screen/manual-note evidence 不属于本阶段。

开发/API 边界包括 `get_summary_policy`、`create_summary_run`、`create_summary_revision`、`execute_summary_run`、`get_summary_run`、`list_summary_runs`、`cancel_summary_run` 和 `list_summary_evidence` 八个 commands，并由集中式 `summaryApi` 调用。DTO（含嵌套结果）统一 camelCase；run/evidence 列表采用默认 50、最大 100 的 keyset 分页。持久 provenance 分开记录 provider/mode/model/prompt version，当前 mode 只能执行 `mock`。它们不是正式生成页面；WebView 不能提交 Provider 结果、evidence、正文、路径或凭据。

segment 修正把依赖旧 revision 的 run 标为 `segment_revised` stale，但旧快照/结果不变。queued/running run 使用的 source 删除稳定拒绝；terminal source 删除保留已引用、最多 1,024 UTF-8 bytes 的 evidence，清理完整正文快照并标 `source_deleted` stale。会议删除先取消 active runs，再由既有协调器清理 summaries、snapshots 和 evidence；任何路径都不得静默重生成。

第一版模板固定为：详实会议纪要、简洁会议总结、行动项与决定清单；输出语言为中文、英文或双语。模型不得发明参会人、负责人、截止日期或会议结论；决定区分 `proposed/confirmed/superseded`，行动项区分明确分配、建议任务和未确认任务，冲突和“无法确定”必须保留。关键决定、行动项和争议必须引用当前 run 快照中的有效 evidence。

详实会议纪要至少包含：会议信息、会议目标、输入来源、执行摘要、按主题整理的详细讨论、已确认决定、提议但未确认事项、行动项、负责人、截止日期、未解决问题、风险和注意事项、争议或冲突、关键时间线、证据引用和无法确定的信息。没有负责人时显示“未分配”，没有截止日期时保持为空；“有人提出”不得自动写成“会议确认”。

#### 5.3.3 第一版文件来源与安全边界

第一版只接受用户通过 MeetingLens 原生文件选择器主动选择的本地文件。Teams、Zoom、Google Meet、邮件或聊天中的附件必须先由用户自行下载，再手动导入。首版不自动读取聊天/云盘附件，不监控 Downloads 或桌面，不读取剪贴板文件，不接受任意路径拖放、远程 URL 或网络共享自动导入。

所有导入文件均视为不可信数据。MeetingLens 只复制和保存参考文档，不启动文件、不调用系统文件关联或 Microsoft Office 自动化、不执行宏/脚本/嵌入对象、不自动访问外部链接，也不在 WebView 中直接渲染未经处理的文件。参考文档内容解析只能由隔离、离线、受 Rust 限制的 Docling Worker 执行；粘贴文本与 TXT/SRT/VTT 转录由 Rust 的有界轻量 parser 处理，不经过 Docling。当前 Python HTTP Sidecar 和 DeepSeek 均不接收文件内容。已有 Worker Foundation 不持久化解析结果，只有后续 `feat/document-parsing-orchestration` 才能接入参考文档生命周期。

React 只接收 document ID、原始显示名称、安全类型、大小、状态和导入时间。Rust 负责原生选择、最终校验、私有存储和路径边界，源文件绝对路径不得写入 SQLite 或返回前端。

#### 5.3.4 格式与配额

第一版 allowlist 为：

- 文档：`.pdf`、`.docx`、`.pptx`、`.xlsx`
- 文本：`.txt`、`.md`、`.csv`
- 图片：`.png`、`.jpg`、`.jpeg`

明确拒绝旧 Office 二进制格式、宏文档、普通归档、可执行文件、脚本、HTML、双重扩展伪装、空文件和无法确定安全类型的文件。扩展名不能作为唯一依据；Rust 必须结合文件签名或安全类型识别，类型不一致或未知时 fail closed。OOXML 虽是 ZIP 容器，但不因此允许普通 ZIP；导入阶段不解压，完整结构验证留给隔离 Worker。未知内容不得自动当作纯文本。

默认配额为：单文件最大 100 MiB、单次选择最多 10 个文件、单场会议最多 20 个文件、单场会议累计最大 500 MiB、原始文件名 UTF-8 表示最大 255 bytes。Rust 是最终执行者，必须防止通过多次调用绕过累计配额；前端校验只改善体验。全局磁盘配额管理器和 Docling 独立内存/处理时限属于后续实现。

### 5.4 总结模式参考材料增强

- `feat/document-parsing-orchestration` 在 transcript-only 纪要与编辑导出闭环之后接入既有 Docling Worker，增加 parse jobs、状态转换、artifact 原子提交、取消、重试和恢复
- `feat/document-evidence-index` 在下一独立 PR 建立 document chunks、页码/章节/表格定位，并把材料 evidence 接入既有 Summary pipeline
- Docling 不参与粘贴/TXT/SRT/VTT 的轻量解析，也不阻塞第一份 transcript-only 纪要
- 两个文档阶段完成后才进入真实音频，不得把文档解析与音频采集塞入同一个 PR

### 5.5 共享实时采集与 ASR

- live-session 数据基础：模式提示、capture session、可选音频 artifact 与删除协调，并复用已有 consent service，不访问真实设备
- Sidecar supervisor、随机端口/打包二进制接线、崩溃恢复、孤儿进程回收和生命周期安全日志
- Windows WASAPI 系统音频与麦克风、可见录音状态和异常恢复
- 实时 ASR、完整时间戳转录、CPU 回退和会后校正入口

以上内容必须继续拆成 live-session data core、sidecar supervisor、音频采集和实时转写四个小型 PR。音频采集 PR 不得顺带新增数据库/保留体系。

### 5.6 屏幕与窗口上下文

- 用户显式选择目标会议窗口，界面持续显示正在读取的应用；窗口关闭或身份变化时暂停并重新确认
- accessibility tree / OCR 提取与关键事件检测
- 只保存与会议相关的结构化事件和证据，截图短期缓存后默认删除
- Screenpipe 只能作为可替换的可选 Capture Provider，不得成为核心依赖

### 5.7 说话人分离与命名

- 使用会话级匿名标签和 speaker alias 作为数据真相，再增加实时/会后聚类、开场姓名校准及人工修改/合并
- 不默认保存 voice sample，不承诺跨会议声纹自动复用
- Diart、WhisperX 等重型模型继续使用隔离 worker、独立环境和明确模型条款

### 5.8 实时翻译与模式集成

- 实时翻译只消费已确定的转录片段，可独立开启、暂停和关闭；关闭后停止持续外发
- 字幕浮窗的透明度、收起、展开、滚动、置顶、点击穿透和多显示器行为由独立 PR 和 Windows 本机验收覆盖
- 阶段总结支持手动、固定时间和需用户确认的自动话题建议，不把阶段摘要直接视为最终会议决定
- 翻译模式工作区复用实时转录、说话人和 checkpoint；会议模式工作区再复用屏幕上下文、重点笔记和同一最终纪要管线
- 自动说话人身份识别不属于首版；默认只使用匿名标签，不跨会议复用身份

### 5.9 后续结构化视觉

- 图表默认不生成；仅在数据可追溯时提供固定类型的时间线、行动项表和决定表，输入不足时不生成
- 决策和行动项反向链接到来源片段

### 5.10 产品化：安装包与加固

- 托盘、自动更新、崩溃恢复、诊断与隐私设置
- PDF 导出、性能优化、长期稳定性与安装包签名

各阶段的唯一分支名称、拆分顺序和验收条件见 `DEVELOPMENT_PLAN.md`。涉及 Windows 桌面行为时，必须由用户本机实际运行验收，CI 不能替代。

## 6. 明确不做

- 未经提示的后台录音或 24/7 屏幕监控
- 默认上传完整录音、完整文档或截图到第三方
- 未经用户选择捕获所有窗口
- 在首版自动识别真实身份
- 将全部会议数据一次发送给外部模型
- 将 MeetingLens 视作遵守录音许可法律的替代品
- 未经用户确认自动发送摘要、邮件或任务
- 当前 MVP 的 SaaS 后端、账号体系、多人协作和跨平台优先开发
- 自动读取 Teams/Zoom/飞书/腾讯会议云端数据或聊天附件
- 自动下载会议录制，或声称支持所有平台专有转录模板
- 自动推断缺失的负责人、截止日期或会议共识
- 近期路线中的视频/音频文件离线转录、视频关键帧和跨会议声纹复用
- 自动判断话题结束、把屏幕变化直接当作会议结论，或默认持续捕获整个桌面
- 执行模型生成的 HTML、JavaScript 或任意图表代码

## 7. 主要界面

1. **首页**：会议模式、翻译模式、总结模式和历史记录四个清晰入口。
2. **总结工作区**：粘贴/导入、片段预览、来源选择、纪要生成和 Markdown 结果；持久编辑在后续独立阶段开放。
3. **实时设置**：在对应能力完成后选择麦克风/系统音频、翻译与逐字稿导出；会议窗口、屏幕上下文和 voice sample 均显示为后续可选能力，不能提前宣称可用。
4. **会议中**：顶部持续状态；左侧原始转录，中间派生译文/阶段摘要，右侧说话人映射、重点书签和已确认上下文。
5. **会议结果**：完整记录、摘要、决策、行动项、未决问题、风险和证据；文档/屏幕标签只在对应增强完成后显示。
6. **设置**：DeepSeek 连接、模型、隐私、保留/删除和诊断设置。

## 8. 关键数据对象

- Meeting、Participant、SpeakerAlias、TranscriptSource、TranscriptSegment、ManualNote
- ScreenEvent、Document、DocumentChunk、EvidenceItem
- Decision、ActionItem、Translation、Summary
- ProcessingJob、ConsentEvent、Artifact

详细字段见 `DATA_MODEL.md`。

## 9. 非功能要求

- Windows 11 是当前唯一交付平台；接口保持模块化，但本轮不投入 macOS/Linux 适配。
- 应用打开后 3 秒内呈现本地会议列表（不含首次迁移）。
- Sidecar 健康检查在本机环境中 1 秒内返回。
- 处理任务崩溃不得破坏已有会议数据，状态必须可恢复。
- UI、Rust command 和 sidecar API 使用版本化数据结构。
- Sidecar 默认只监听 loopback，不向局域网暴露。
- 任何外部请求都能在 UI 中辨认和取消。
- 外部依赖使用精确版本和 lockfile；模型、授权和系统要求在实现分支开始前复核。

## 10. DeepSeek 约定

已完成的 Summary Run Core 不调用 DeepSeek：`deterministic_mock` 是零网络开发 Provider，普通用户生成入口与正式结果 UI 仍不可用。以下约定只对后续获批的真实 Provider 阶段生效。

- 基础地址可配置，默认使用官方 `https://api.deepseek.com`。
- 实时翻译模型为 `deepseek-v4-flash`。
- 最终摘要模型为 `deepseek-v4-pro`。
- 禁止继续使用已弃用的 `deepseek-chat` 或 `deepseek-reasoner` 名称。
- 长会议采用分段事实提取、主题归并、决策/行动项/未决问题提取和最终合并。
- 调用前显示 transcript sources、segment 数量、时间范围、speaker、模板、输出语言、文档来源和将外发的数据类型；用户确认后才能发送。
- Rust 从 SecretStore 获取 key；React、Docling Worker、日志和 SQLite 均不能读取或保存 key。
- transcript-only MVP 只发送用户确认的规范化文本和必要元数据，不发送原始参考文件、本地路径或完整 prompt 日志。
- Summary run 使用不可变输入快照；输入后续变化不影响当前 run，重新生成创建 revision，用户编辑不得被静默覆盖。
- 未知 evidence ID 必须拒绝或标记，模型自然语言不得被当作不存在的证据引用。

## 11. Phase 0 验收标准

- 五份核心规格文档相互一致。
- Tauri 2 + React + TypeScript 脚手架通过前端类型检查和生产构建。
- 首页展示核心模块、隐私边界和 sidecar 健康入口。
- Python sidecar 的 `GET /health` 返回稳定、版本化 JSON。
- Tauri/Rust command 以短超时请求 `/health`、验证契约并向 React 返回脱敏状态。
- `/health` 成功和 404 行为有自动化测试，Rust 响应校验有单元测试。
- GitHub CI 使用 Python 3.12，并执行 formatter、前端检查、Python 测试和 Tauri 核心测试。
- 仓库中不存在真实凭据或会议数据。

## 12. 成功指标

- 从会议结束到获得第一版可核验摘要的中位时间
- 有来源链接的行动项占比
- 转录/说话人标签的人工修正率
- Sidecar 任务成功率和重试后成功率
- 用户主动删除会议时的完整删除成功率
- 发送到外部 AI 的文本量相对原始会议数据的比例
