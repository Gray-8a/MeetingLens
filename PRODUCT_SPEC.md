# MeetingLens 产品规格

## 1. 产品定义

MeetingLens 是一款面向 Windows 11 的本地优先 AI 会议助手。第一核心目标是：让用户以最少步骤，将已有会议转录、粘贴文本、实时转写和相关会议材料转化为详实、可编辑、可追溯的会议纪要。

产品优先把“会上说了什么”转化成可核验的决定、行动项、风险和未决问题。转录是最重要的第一类来源；文档提供背景和证据增强，真实音频、屏幕上下文、说话人增强与实时翻译按价值顺序逐步接入，同时尽量减少会议数据离开本机。

当前产品入口与首轮边界以 `SIMPLIFIED_MODE_SPEC.md` 为准：同一套会话引擎提供总结、翻译和会议三种模式。此前已经合并的工程与安全基础继续复用，旧版尚未开始的路线暂停。

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
- **会议模式**：在实时转录基础上提供阶段记录、最终纪要和可选翻译；屏幕上下文后续独立加入。
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

真实音频阶段由用户明确开始采集，界面持续显示麦克风、系统音频、保存原始音频和外部翻译的独立状态；实时 ASR 将片段写入与导入转录相同的数据模型。屏幕、说话人和翻译均为可选增强，失败不得阻塞本地转录、人工记录或会后纪要。

阶段摘要首版只允许用户手动触发，或每 5 分钟在存在新最终片段时触发。最终纪要必须读取原始转录片段的不可变快照，不能只对阶段摘要再次总结。自动话题边界、透明桌面浮窗和屏幕变化判断属于后续独立增强。

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

PR #1–#15 已合并。文档安全存储、原生导入 UI、Docling Worker Foundation 和转录/纪要规格已完成；Docling 具备独立锁定环境、显式离线模型、stdio 协议、Windows Job Object、超时/取消和多格式本机 Smoke，但尚未接入 SQLite、Provider、Commands 或 UI。当前 SQLite schema 为 v2，模拟字幕仍只在前端内存中，DeepSeek key 可以安全保存但没有真实 Provider 消费，产品当前不能生成真实会议纪要。

当前顺序不再使用未来 PR 编号作为计划契约：

1. **`docs/simplified-three-mode-spec`**：冻结三个入口、共享管线、隐私边界和新顺序，不实现功能。
2. **i18n 决策门**：暂停中的 `feat/i18n-user-guide-foundation` 只由用户决定去留；若保留，必须先基于新 `main` 删除旧路线冲突并重新验收，不能原样合并。
3. **`feat/transcript-data-core`**：schema v3、`transcript_sources`/`transcript_segments`、粘贴/TXT/SRT/VTT 同步解析、去重、修正和删除语义；不做 UI 或外部 Provider。
4. **`feat/summary-mode-import`**：Transcript Commands/API、总结模式入口、粘贴与原生导入、片段预览和基础说话人修正；不调用 DeepSeek。
5. **`feat/summary-run-core`**：Summary run/revision、不可变输入快照、evidence 与稳定错误；只使用 Mock，不接外部 Provider 或结果 UI。
6. **`feat/consent-data-core`**：通用 `consent_events`、会话失效和审计 API；不调用 Provider、不访问设备。
7. **`feat/summary-provider-deepseek`**：Rust SecretStore 消费、外发同意和受限 DeepSeek adapter；不新增 schema 或正式页面。
8. **`feat/summary-mode-minutes`**：选择输入/模板、外发预览、生成状态、结果、证据和 Markdown 复制/下载；不新增数据库或 Provider。
9. **`feat/summary-editing-export`**：用户编辑、summary revision、stale、章节重生成和 DOCX。
10. **共享实时底座**：依次完成 `feat/live-session-data-core`、`feat/sidecar-supervisor`、`feat/audio-capture` 和 `feat/realtime-transcription`。
11. **翻译模式**：依次完成 `feat/live-language-data-core`、`feat/realtime-translation-provider`、`feat/checkpoint-summary-runtime` 和 `feat/translation-mode-workspace`。
12. **会议模式**：先完成 `feat/meeting-notes-core`，再完成 `feat/meeting-mode-workspace`；屏幕上下文、自动话题边界、说话人增强、透明浮窗和固定类型图表继续拆分。
13. **参考材料增强**：`feat/document-parsing-orchestration` 和 `feat/document-evidence-index` 保留，但不阻塞 transcript-only 总结模式。

Docling 是参考文档解析增强，不是 transcript-only 纪要的强制前置。转录证明会上说了什么；文档提供材料与背景，文档中写着某项计划不等于会议已经确认该计划。

#### 5.3.1 第一版转录输入与限制

近期只实现粘贴严格 UTF-8 文本，以及 TXT、SRT、VTT。普通文本按非空段落或明确发言行保序转换；仅识别明确的 `姓名: 内容`，不确定时 speaker 为空，不用大模型猜测。SRT/VTT 保留 cue 顺序、相对时间、原始语言和明确说话人标签，不自动翻译。

限制固定为：粘贴内容最多 2 MiB、转录文件最多 10 MiB、每个 source 最多 20,000 个 segments、单 segment 正文最多 64 KiB。输入必须为严格 UTF-8，拒绝 NUL 和除 TAB/LF/CR 外的危险 C0 控制字符。文本中的 URL、代码和指令只作为数据，不执行、不访问网络、不写入普通日志。

`platform_export` 是保留来源类型；Teams、Zoom、飞书、腾讯会议等平台专用 adapter 只有获得脱敏真实样例后才增加。首版不承诺 DOCX/CSV/JSON 平台模板、视频转录或音频文件转录。

解析后持久化规范化 segments、原始显示文件名、来源类型和内容 SHA-256，但不长期复制原始转录文件；源文件不被修改或删除。重新解析需要用户再次选择源文件。该哈希用于来源识别和同会议重复检测，不代表可信或防病毒。

#### 5.3.2 纪要资格、模板与证据

纪要资格取决于能否冻结至少一个用户选择的有效输入，而不是只允许 `ready`。架构允许 `draft/recording/ready` 搭配 transcript、manual notes、documents 或多来源混合，也允许 transcript-only、documents-only 和 manual-notes-only；近期 MVP 只实现 transcript 输入。`deleting`、`delete_failed`、没有输入或输入仍无法安全冻结时禁止。纪要任务拥有独立状态，不新增 `content_ready/source_ready/summarized` 等 meeting 状态。

第一版模板固定为：详实会议纪要、简洁会议总结、行动项与决定清单；输出语言为中文、英文或双语。模型不得发明参会人、负责人、截止日期或会议结论；决定区分 `proposed/confirmed/superseded`，行动项区分明确分配、建议任务和未确认任务，冲突和“无法确定”必须保留。关键决定、行动项和争议必须引用当前 run 快照中的有效 evidence。

详实会议纪要至少包含：会议信息、会议目标、输入来源、执行摘要、按主题整理的详细讨论、已确认决定、提议但未确认事项、行动项、负责人、截止日期、未解决问题、风险和注意事项、争议或冲突、关键时间线、证据引用和无法确定的信息。没有负责人时显示“未分配”，没有截止日期时保持为空；“有人提出”不得自动写成“会议确认”。

#### 5.3.3 第一版文件来源与安全边界

第一版只接受用户通过 MeetingLens 原生文件选择器主动选择的本地文件。Teams、Zoom、Google Meet、邮件或聊天中的附件必须先由用户自行下载，再手动导入。首版不自动读取聊天/云盘附件，不监控 Downloads 或桌面，不读取剪贴板文件，不接受任意路径拖放、远程 URL 或网络共享自动导入。

所有导入文件均视为不可信数据。MeetingLens 只复制和保存文件，不启动文件、不调用系统文件关联或 Microsoft Office 自动化、不执行宏/脚本/嵌入对象、不自动访问外部链接，也不在 WebView 中直接渲染未经处理的文件。解析只能由隔离、离线、受 Rust 限制的 Docling Worker 执行；当前 Python HTTP Sidecar 和 DeepSeek 均不接收文件内容。已有 Worker Foundation 不持久化解析结果，只有后续 `feat/document-parsing-orchestration` 才能接入文档生命周期。

React 只接收 document ID、原始显示名称、安全类型、大小、状态和导入时间。Rust 负责原生选择、最终校验、私有存储和路径边界，源文件绝对路径不得写入 SQLite 或返回前端。

#### 5.3.4 格式与配额

第一版 allowlist 为：

- 文档：`.pdf`、`.docx`、`.pptx`、`.xlsx`
- 文本：`.txt`、`.md`、`.csv`
- 图片：`.png`、`.jpg`、`.jpeg`

明确拒绝旧 Office 二进制格式、宏文档、普通归档、可执行文件、脚本、HTML、双重扩展伪装、空文件和无法确定安全类型的文件。扩展名不能作为唯一依据；Rust 必须结合文件签名或安全类型识别，类型不一致或未知时 fail closed。OOXML 虽是 ZIP 容器，但不因此允许普通 ZIP；导入阶段不解压，完整结构验证留给隔离 Worker。未知内容不得自动当作纯文本。

默认配额为：单文件最大 100 MiB、单次选择最多 10 个文件、单场会议最多 20 个文件、单场会议累计最大 500 MiB、原始文件名 UTF-8 表示最大 255 bytes。Rust 是最终执行者，必须防止通过多次调用绕过累计配额；前端校验只改善体验。全局磁盘配额管理器和 Docling 独立内存/处理时限属于后续实现。

### 5.4 共享实时底座：Sidecar 生命周期、真实音频与转写

- live-session 数据基础：模式提示、capture session、可选音频 artifact 与删除协调，并复用已有 consent service，不访问真实设备
- Sidecar supervisor、随机端口/打包二进制接线、崩溃恢复、孤儿进程回收和生命周期安全日志
- Windows WASAPI 系统音频与麦克风、可见录音状态和异常恢复
- 实时 ASR、完整时间戳转录、CPU 回退和会后校正入口

以上内容必须继续拆成 live-session data core、sidecar supervisor、音频采集和实时转写四个小型 PR。音频采集 PR 不得顺带新增数据库/保留体系。

### 5.5 翻译与会议模式工作区

- 翻译模式先分别完成译文/speaker/checkpoint 数据、实时翻译 Provider 和 checkpoint runtime，再由主窗口工作区集成可收起、展开和滚动的原文/译文区域
- 会议模式先建立可追溯的转录书签/用户文字笔记，再由工作区复用实时转录与翻译，增加手动重点记录、阶段摘要和最终纪要
- Windows 透明、置顶、点击穿透和多显示器字幕浮窗必须作为独立 PR 和本机验收项
- 自动说话人身份识别不属于首版；默认只使用匿名标签，不跨会议复用身份

### 5.6 后续：会议窗口、OCR 与屏幕上下文

- 用户显式选择目标会议窗口
- accessibility tree / OCR 提取与关键事件检测
- 只保存与会议相关的结构化事件和证据，截图短期缓存后默认删除
- Screenpipe 只能作为可替换的可选 Capture Provider，不得成为核心依赖

### 5.7 后续：自动化与说话人增强

- 自动话题边界与阶段摘要建议，必须允许用户修正
- 匿名实时说话人分离、人工重命名和会后 WhisperX 校正
- 保存用户主动提供的短 voice sample，允许重录和删除
- 固定类型的时间线、行动项表和决定表；输入不足时不生成图表
- 决策和行动项反向链接到来源片段

### 5.8 产品化：安装包与加固

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
