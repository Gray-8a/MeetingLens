# MeetingLens 产品规格

## 1. 产品定义

MeetingLens 是一款面向 Windows 11 的本地优先 AI 会议助手，当前采用 **Personal Edition / 自用版基线**。第一核心目标是：让单个用户以最少步骤，将已有会议转录或粘贴文本发送给经明确确认的 DeepSeek Provider，获得详实、可编辑、可追溯的会议纪要，并通过 Markdown 复制或保存。

产品优先把“会上说了什么”转化成可核验的决定、行动项、风险和未决问题。转录是最重要的第一类来源；录音文件事后批量转录和文档证据是可复用同一管线的后续增强，窗口上下文只有在用户确认真实需要后才可选启动。实时音频、实时 ASR、说话人增强、实时翻译和独立会议工作区不属于当前自用版的近期完成标准。

长期三模式愿景与复用规则以 `SIMPLIFIED_MODE_SPEC.md` 为准；本文件是产品能力、行为规则和验收边界的权威规格；实际实施顺序以 `DEVELOPMENT_PLAN.md` 为准。当前只优先完成总结模式。翻译和独立会议模式工作区是 Frozen/Deferred 的长期愿景，此前已经合并的工程与安全基础继续保留，不删除代码或数据。

## 2. 目标用户与边界

- 需要整理高频会议的个人和学生
- 使用其他软件完成会议并在会后整理转录或录音文件的个人用户
- 需要提取决定、行动项、风险和未决问题的项目负责人
- 对会议隐私、数据驻留和删除能力有明确要求的用户

Personal Edition 是单人自用的单机桌面产品，不引入账号体系、团队后端或云端数据存储，也不以机器人身份加入 Zoom、Teams 或 Google Meet。当前最小外发确认不足以作为面向团队、客户或公众分发的企业合规方案。

## 3. 产品原则

1. **明确同意**：录音、屏幕上下文和外部 AI 处理必须由用户主动开启。
2. **本地优先**：录音、上传文件、数据库和处理中间产物默认留在本机。
3. **可追溯**：摘要、决定和行动项能够定位到时间戳、转录片段、文档或屏幕证据。
4. **最小发送**：调用外部模型前只发送用户本次明确选择并确认的规范化转录内容和必要元数据，不发送本地路径、原始文件或凭据。
5. **可恢复、可删除**：处理任务可独立失败和重试；用户可删除会议及全部本地派生数据。
6. **准确优先**：准确性优先于低延迟；实时结果可以在会后校正。
7. **依赖复用**：优先集成成熟组件，不自行重写转录、说话人分离、OCR 或文档解析算法。

## 4. 核心使用流程

### 4.1 首页快速入口

首页保留三个主要模式入口和历史记录，但当前 Personal Edition 只把总结模式作为近期可进入的产品主线。用户不需要先理解 Provider、Docling、设备协议或内部状态机。

- **总结模式（Current）**：当前可粘贴 UTF-8 文本或通过 Rust 原生选择器导入 TXT/SRT/VTT，并预览和修正规范化片段；底层 Rust DeepSeek Provider、最小外发确认与撤销已经实现，但正式生成入口和结果页仍未接线。
- **翻译模式（Frozen/Deferred）**：保留长期愿景和共享数据边界，当前不开发实时字幕、持续翻译外发、checkpoint 或浮窗。
- **会议模式（Frozen/Deferred）**：未来如解冻，优先复用录音/转录输入、现有 Transcript Workspace 和同一 Summary pipeline，不建立第二套数据库或转录/摘要模型。
- **历史记录（Current）**：当前查看本地会议和已实现的转录入口；未来摘要和导出结果仍复用同一历史管线。

音频文件批量转录是 `feat/summary-mode-minutes` 达到日常可用后计划的增强，当前尚未显示为可用入口；视频导入继续 Deferred。参考材料仍通过独立 Document UI 管理，不与转录来源混淆。

### 4.2 生成会议纪要（Planned）

1. 用户选择一个或多个可用来源；近期 transcript-only MVP 只消费转录来源。
2. Rust 先在本地创建 queued run 并原子冻结不可变输入快照；这一步不读取 Key，也不发起网络，后续编辑不改变本次 run。
3. 应用根据该冻结 run 显示 Provider/host/model、来源数、片段数、input bytes、模板、输出语言和外发数据类型。
4. 第一次准备把冻结内容发送给 DeepSeek 时，应用提供“本次同意”“记住我的选择”和“取消”；没有有效确认时 fail closed。
5. 执行时 Rust 再校验 Key、Consent、run 状态和冻结范围；DeepSeek 只接收用户确认的规范化文本和必要元数据，不接收本地路径、原始文件或凭据。
6. 结果按详细讨论、决定、行动项、未决问题、风险、争议和证据展示，并标记为可编辑草稿。
7. 用户可以修改正文、保存当前结果或整份重新生成；重新生成创建新 revision，不静默覆盖旧 revision 或用户编辑。

### 4.3 Frozen/Deferred 的实时能力

实时会议录音、WASAPI、实时 ASR、实时说话人分离、实时翻译、字幕浮窗、checkpoint、自动话题判断和独立会议工作区全部 Frozen/Deferred，当前不开发，也不永久删除。未来如解冻，真实音频必须由用户明确开始采集，界面持续显示麦克风、系统音频、保存原始音频和外部翻译的独立状态，并继续写入与导入转录相同的数据模型。

如果这些能力未来解冻，保存完整转录、保存原始音频和保存屏幕关键帧仍必须是独立选项；自动话题边界只能给出需要用户确认的建议，不得静默切段或写成最终决定。当前 Personal Edition 不以这些能力作为日常可用或发布门槛。

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

### 5.3 Personal Edition 近期主线：完成日常可用总结模式

PR #1–#23 已合并。文档安全存储、原生导入 UI、Docling Worker Foundation、zh-CN/en-US、Transcript Data Core、安全 Transcript Command API、Summary Mode Input UI、schema v4 Summary Run Core 和 Personal Edition 重定基线已完成。当前 `feat/summary-provider-deepseek` 在既有不可变快照、run/series/revision 和 evidence allowlist 上增加 schema v5、Rust-only DeepSeek、SecretStore 消费、最小外发确认/记住/撤销、严格 JSON、取消和稳定错误。Docling Worker 尚未接入 SQLite、Commands 或产品 UI；正式生成入口和结果 UI 仍禁用，因此普通用户仍无法在当前页面获得真实 AI 纪要。

当前最短路线如下。分支名和阶段能力范围是权威标识；未来 PR 编号只表示预期顺序：

1. **`docs/personal-edition-descope`（PR #23 已完成）**：重新定义 Personal Edition、冻结/简化范围和风险分级验收。
2. **`feat/summary-provider-deepseek`（当前交付，预计 PR #24）**：schema v5 最小 remembered Consent、Rust DeepSeek Provider、SecretStore 读取、固定真实请求、严格响应/evidence、安全错误/取消，以及本次同意/记住/撤销；不建立通用 Consent Data Core，不开发正式结果页面。
3. **`feat/summary-mode-minutes`（唯一下一阶段，预计 PR #25，日常可用里程碑）**：模板、中文/英文/双语输出、正式生成入口、纪要结果页、Markdown 查看/复制/保存、正文修改和整份重新生成；继续使用现有 series/revision，不覆盖旧 revision，不开发章节级重生成或 DOCX/PDF。
4. **`feat/audio-file-transcription`（预计 PR #26）**：导入会议录音文件，通过本地 faster-whisper CPU int8 一次性受控 Worker 批量转写，并写入现有 transcript 数据模型；不做实时音频、WASAPI、实时字幕或实时 diarization。
5. **`feat/document-parsing-orchestration`（预计 PR #27）**：将已有 Docling Worker 接入正式数据流，生成 document chunks/evidence，失败自动重试一次，再失败提示手动重新导入；不建立重型通用调度平台。
6. **`feat/window-context`（可选，预计 PR #28 或更后）**：只有在用户确认经常需要屏幕分享上下文后才启动；必须明确选择具体窗口，身份变化时暂停或重新确认，默认不保存关键帧。

Frozen/Deferred：实时会议录音、实时 ASR、实时说话人分离、实时翻译、字幕浮窗、checkpoint、自动话题判断、独立翻译/会议工作区、Structured Visuals、安装包完善、签名、SBOM、托盘、自动更新、商业/企业分发，以及 DOCX/PDF 导出。它们当前不开发，但不删除已有基础或未来恢复可能。

界面语言与纪要输出语言是两项独立设置：切换 zh-CN/en-US 只改变应用界面，不得自动改变当前或未来 Summary run 的中文、英文或双语输出选择。

Docling 是参考文档解析增强，不是 transcript-only 纪要的强制前置。转录证明会上说了什么；文档提供材料与背景，文档中写着某项计划不等于会议已经确认该计划。

#### 5.3.1 第一版转录输入与限制

近期只实现粘贴严格 UTF-8 文本，以及 TXT、SRT、VTT。普通文本按非空段落或明确发言行保序转换；仅识别明确的 `姓名: 内容`，不确定时 speaker 为空，不用大模型猜测。SRT/VTT 保留 cue 顺序、相对时间、原始语言和明确说话人标签，不自动翻译。

限制固定为：粘贴内容最多 2 MiB、转录文件最多 10 MiB、每个 source 最多 20,000 个 segments、单 segment 正文最多 64 KiB。输入必须为严格 UTF-8，拒绝 NUL 和除 TAB/LF/CR 外的危险 C0 控制字符。文本中的 URL、代码和指令只作为数据，不执行、不访问网络、不写入普通日志。

Summary run 的多来源聚合限制独立固定为：最多 20,000 个 final segments、规范化正文合计最多 10 MiB UTF-8 bytes。来源按用户选择顺序、片段按稳定 ordinal 冻结并产生全局 ordinal；空输入、重复/跨 meeting source、只有 partial segments 或任何超限都整体拒绝，不截断、不抽样。

普通用户检查页面不得一次读取或渲染 20,000 个 segments。列表由后端按稳定 `ordinal` 顺序分页，生产单页上限为 200，并返回总数和后续页信息；不得在前端先取得全量再切片。同一 source 内相同 `speaker_label` 可由用户统一设置或清空 `speaker_name`，每条受影响 segment 的 revision 必须递增且 `corrected_at` 更新；原始 label 不变，映射不跨 source/meeting，也不推断真实身份。

`platform_export` 是保留来源类型；Teams、Zoom、飞书、腾讯会议等平台专用 adapter 只有获得脱敏真实样例后才增加。当前不承诺 DOCX/CSV/JSON 平台模板或视频转录。音频文件转录尚未实现，但已计划由后续 `feat/audio-file-transcription` 通过本地批量 Worker 接入现有 transcript 数据模型。

解析后持久化规范化 segments、原始显示文件名、来源类型和内容 SHA-256，但不长期复制原始转录文件；源文件不被修改或删除。重新解析需要用户再次选择源文件。该哈希用于来源识别和同会议重复检测，不代表可信或防病毒。

当前 meeting 模型没有独立、可编辑的会议发生日期。总结模式只能将 `created_at` 明确展示为“创建时间”，不得把它冒充会议发生日期，也不得在没有数据模型与 migration 先行批准时提供无法持久化的日期输入。

#### 5.3.2 纪要资格、模板与证据

纪要资格取决于能否冻结至少一个用户选择的有效输入，而不是只允许 `ready`。当前 schema v4 与 Summary service 只实现 `draft/recording/ready` 搭配 transcript snapshot；manual notes、documents、混合来源、documents-only 和 manual-notes-only 只是长期架构方向，必须等对应数据与 evidence 阶段实际实现后才能启用。`deleting`、`delete_failed`、没有输入或输入仍无法安全冻结时禁止。纪要任务拥有独立状态，不新增 `content_ready/source_ready/summarized` 等 meeting 状态。

schema v4 为每个新纪要系列生成 `series_id` 和 revision 1；显式重新生成在事务中创建下一 revision、重新冻结当前 live segment revisions，不覆盖旧 run。快照实际复制正文、source/segment IDs、revision、source/segment/global ordinal、speaker、offset、language、confidence 与时间，Provider 不得回读 live segment。当前状态机是 `queued → running → completed/failed/cancelled` 和 `queued → cancelled`；启动后遗留 running 变为 `failed + summary_interrupted`。

production run 固定记录 `deepseek/external/deepseek-v4-pro/summary-deepseek-v1`，只由 Rust 从不可变 snapshot 构造请求并在执行前读取 SecretStore。历史 `deterministic_mock` run 继续可读，但 Mock 只允许测试/debug 隔离，release WebView 不可选择或执行。每个 snapshot row 的 UUID 形成当前 run evidence allowlist；Rust 在原子写入结果前验证 output run ID、结构版本、大小/数量和全部 evidence，未知、重复或跨 run 引用使 run 安全失败。只有结果实际引用的 transcript evidence 被持久化；document/screen/manual-note evidence 不属于本阶段。

集中式 `summaryApi` 固定包含十二个方法：既有 policy/create/revision/execute/get/list/cancel/evidence 八个 commands，以及 `get_deepseek_provider_status`、`get_summary_external_disclosure`、`confirm_summary_external_disclosure`、`revoke_deepseek_consent`。DTO（含嵌套结果）统一 camelCase 并严格拒绝扩展；run/evidence 列表采用默认 50、最大 100 的 keyset 分页。WebView 只能提交 ID、模板/语言、分页和 `remember` 布尔值，不能提交 Provider/model/endpoint、结果、evidence、正文、路径、凭据或时间戳。这些底层 API、Consent Dialog 和 Settings 状态不等于正式生成页面已经开放。

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

默认配额为：单文件最大 100 MiB、单次选择最多 10 个文件、单场会议最多 20 个文件、单场会议累计最大 500 MiB、原始文件名 UTF-8 表示最大 255 bytes。Rust 是最终执行者，必须防止通过多次调用绕过累计配额；前端校验只改善体验。全局磁盘配额管理器属于后续实现；PR #14 已为隔离 Docling Worker 实现 4 GiB Job Object 内存上限和 150 秒处理时限。

### 5.4 总结模式参考材料增强（Planned）

- `feat/document-parsing-orchestration` 在音频文件批量转写之后接入既有 Docling Worker，并在同一清晰用户能力内生成 document chunks/evidence、页码/章节/表格定位并接入既有 Summary pipeline
- 单次解析失败自动重试一次；再次失败时提示用户手动重新导入，不实现通用 processing-job 调度器、多级恢复、长期 checkpoint、指数退避或企业任务监控面板
- Docling 不参与粘贴/TXT/SRT/VTT 的轻量解析，也不阻塞第一份 transcript-only 纪要
- 继续保留独立进程、资源限制、Rust 路径边界、不向 React 暴露路径和不静默外发文件的安全约束

### 5.5 音频文件批量转录（Planned）

- 用户主动导入已有会议录音文件，本地 faster-whisper 以 CPU int8 和一次性受控 Worker 运行
- 转写结果写入现有 `transcript_sources/transcript_segments`，进入现有 Transcript Workspace 和 Summary pipeline
- 当前尚未实现，不得描述为可用，也不增加实时音频、WASAPI、持续 Sidecar、实时字幕、实时 ASR 或实时 diarization

### 5.6 可选屏幕与窗口上下文

- 用户显式选择目标会议窗口，界面持续显示正在读取的应用；窗口关闭或身份变化时暂停并重新确认
- 只有在用户确认实际经常需要该能力后才启动；首个可选阶段不自动监控全部桌面
- 首个可选阶段优先读取受控 accessibility text，只保存用户确认的结构化文字或 screen evidence，默认不保存屏幕关键帧
- screen OCR、区域/全屏捕获、截图缓存和自动关键事件检测为 Frozen/Deferred，不属于 `feat/window-context`
- Screenpipe 仅保留为未来解冻时可重新评估的 Capture Provider 候选，不得成为当前阶段或核心依赖

### 5.7 说话人分离与命名（Frozen/Deferred）

- 使用会话级匿名标签和 speaker alias 作为数据真相，再增加实时/会后聚类、开场姓名校准及人工修改/合并
- 不默认保存 voice sample，不承诺跨会议声纹自动复用
- Diart、WhisperX 等重型模型继续使用隔离 worker、独立环境和明确模型条款

### 5.8 实时翻译与模式集成（Frozen/Deferred）

- 实时翻译只消费已确定的转录片段，可独立开启、暂停和关闭；关闭后停止持续外发
- 字幕浮窗的透明度、收起、展开、滚动、置顶、点击穿透和多显示器行为由独立 PR 和 Windows 本机验收覆盖
- 阶段总结支持手动、固定时间和需用户确认的自动话题建议，不把阶段摘要直接视为最终会议决定
- 翻译模式工作区复用实时转录、说话人和 checkpoint；会议模式工作区再复用屏幕上下文、重点笔记和同一最终纪要管线
- 自动说话人身份识别不属于首版；默认只使用匿名标签，不跨会议复用身份

### 5.9 Structured Visuals（Frozen/Deferred）

- 当前不开发自动图表、自动流程图、结构化可视化生成或依据不充分的 AI 图表建议
- 现有 evidence 和确定性行动项/决定数据继续保留，未来如解冻仍必须反向链接来源

### 5.10 商业分发与复杂导出（Frozen/Deferred）

- 安装包完善、代码签名、SBOM、托盘、自动更新、商业发布和企业部署不属于当前 Personal Edition 路线
- DOCX/PDF 导出、复杂版式模板和 Office 兼容性矩阵不属于当前近期输出目标；当前输出终点是页面查看、Markdown、一键复制和 Markdown 文件保存

各阶段的唯一分支名称、范围和风险分级验收条件见 `DEVELOPMENT_PLAN.md`。当数据模型和边界清晰时，一个完整用户能力可以在一个独立 PR 中包含必要的数据层、Rust Commands、TypeScript API 和 UI；涉及 Windows 桌面行为时仍须完成与风险匹配的本机验收，CI 不能替代。

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
- 当前阶段声称已支持音频文件转录，或把视频转录、实时音频、视频关键帧和跨会议声纹复用纳入近期交付
- 自动判断话题结束、把屏幕变化直接当作会议结论，或默认持续捕获整个桌面
- 执行模型生成的 HTML、JavaScript 或任意图表代码

## 7. 主要界面

1. **首页（Current）**：总结、翻译、会议三个入口和历史记录；当前只有总结模式进入实际工作流，翻译和会议入口保持禁用并标为 Frozen/Deferred。
2. **总结输入工作区（Current）**：粘贴/导入、来源和分页片段预览、正文与说话人显示名修正；生成按钮仍禁用。
3. **总结结果工作区（Planned）**：模板和输出语言、生成状态、会议纪要、证据、Markdown 查看/复制/保存、正文修改和整份重新生成。
4. **会议结果增强（Planned/Optional）**：音频文件转录、文档 evidence 和可选窗口上下文只在对应阶段完成后显示。
5. **实时设置和会议中工作区（Frozen/Deferred）**：实时音频、翻译、字幕、checkpoint、说话人分离和独立会议布局当前不开发。
6. **设置（Current）**：DeepSeek Key 配置状态、固定 Provider/host/model、remembered Consent 状态与时间、撤销入口、界面语言和本地诊断；不显示 Key、不允许编辑 model/base URL，也不允许脱离具体 run 提前授权。

## 8. 关键数据对象

- 当前已实现对象以 schema v5 和 `DATA_MODEL.md` 为准，包括 Meeting、Document、TranscriptSource、TranscriptSegment、Summary、SummarySource、EvidenceItem 和最小 ProviderConsentPreference
- DocumentChunk、ScreenEvent、Translation、SpeakerAlias 等仅在相应 Planned 或 Frozen/Deferred 阶段恢复后增加，不得描述为当前已实现
- schema v5 仅追加 `provider_consent_preferences` 当前偏好表，保存固定 provider/policy fingerprint 与 consented/updated UTC 时间；没有独立 `consent_events`、session/audit log、正文、run ID、Prompt、响应、路径或 Key

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

底层 DeepSeek Provider 已由 Rust 实现；普通用户生成入口与正式结果 UI 仍不可用。production 身份固定为 Provider `deepseek`、mode `external`、host `api.deepseek.com`、model `deepseek-v4-pro`、prompt version `summary-deepseek-v1`，endpoint 固定为 `https://api.deepseek.com/chat/completions`，不能由 WebView 或用户修改。

- DeepSeek 请求只能由 Rust `SummaryProvider` 发起；React、Python Sidecar 和 Docling Worker 均不得直接调用 Provider。
- Provider 响应必须严格解析并限制结构、大小和 evidence 引用；未知、重复、跨 run 或不匹配的 evidence 继续由 Rust fail closed。
- 外发 disclosure 只显示固定 Provider/host/model、source/segment/input byte 数、模板、输出语言和固定六类数据；不返回 transcript、snapshot rows、evidence excerpt、路径、Prompt 或数据库细节。
- 一次性确认按 Summary run 绑定，在 Rust 内存中最多保留 10 分钟、只能消费一次且重启失效；remembered Consent 只以 schema v5 的 policy version/fingerprint 和时间戳持久化。没有有效确认时不得发起请求。
- 撤销会删除 remembered Consent 并清除全部尚未消费的一次性确认，只影响未来发送，不假装召回已经送达 Provider 的内容。这套最小 Consent 仅适用于单人自用版，不足以作为团队、客户或公众分发的企业合规方案。
- Rust 从 SecretStore 获取 key；React、Docling Worker、日志和 SQLite 均不能读取或保存 key。
- transcript-only Provider 只发送不可变 snapshot 的规范化文本、speaker 标签/用户确认名称、source/segment 顺序、时间 offset、run-local evidence IDs、模板和输出语言；除这些 run-local evidence IDs 外，不发送其他数据库标识，也不发送会议标题、source 显示名/路径、语言/置信度元数据、原始参考文件、API Key 或完整 Prompt 日志。
- transport 固定 HTTPS，禁止重定向和环境代理，设置 10 秒连接/180 秒总超时、请求/响应大小上限、非流式 JSON-only、thinking disabled、单次请求且不自动重试；运行中取消会中止 transport future，失败不保留部分结果。
- Summary run 使用不可变输入快照；输入后续变化不影响当前 run，整份重新生成创建 revision，用户编辑不得被静默覆盖。章节/段落级重生成、复杂 stale UI、编辑历史时间线和协作冲突合并当前冻结。
- 未知 evidence ID 必须拒绝或标记，模型自然语言不得被当作不存在的证据引用。
- 自动测试只使用明显合成数据与本地 fake transport；真实付费 DeepSeek API smoke 未执行时不得宣称协议已通过真实账户联网验收。

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
