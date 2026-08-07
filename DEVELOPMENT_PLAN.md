# MeetingLens 开发计划

## 1. 工作流与阶段规则

`main` 始终保持可构建、可演示。Codex 可以自主开发、测试、修复 CI、提交、推送和创建/更新 PR，但不得启用 auto-merge，也不得自动合并；最终 Merge 由用户决定。

每个实现 PR 必须：

1. 只覆盖一个可验收范围，并使用本计划指定的分支名。
2. 有独立的正常、失败和安全验收条件。
3. 通过 formatter、lint、类型检查、测试、构建和敏感信息门禁。
4. 涉及 Windows 桌面行为时，由用户本机实际运行验收，不能只凭 CI 声称完成。
5. 合并后再从最新 `main` 创建下一个分支，不堆叠未审查的大型 PR。

MVP 1A-2 实现前已使用 `docs/define-meeting-lifecycle` 固定会议状态转换、删除演进策略和 PR 边界。Provider 与安全基础随后通过 1A-3A、1A-3B-1 和 1A-3B-2 三个顺序 PR 完成，避免把能力契约、Sidecar 会话认证与操作系统凭据存储放入同一审查单元。MVP 1B 在任何文件实现前先使用纯文档 PR #11 固定不可信输入、私有存储和可恢复删除语义。

本文件是 MeetingLens 实施顺序、分支名、停止线和验收条件的权威文档。PR #16 合并后，产品方向继续以 `SIMPLIFIED_MODE_SPEC.md` 定义的“一个共享引擎、三个简约入口”收敛：先完成 transcript-only 总结、真实纪要和编辑导出闭环；随后接入 Docling 编排与文档 evidence，但 Docling 不阻塞第一份纪要；之后依次建设实时采集、实时 ASR、屏幕/窗口上下文、说话人分离与命名、实时翻译，最后集成翻译模式和会议模式。旧版未开始的路线冻结但不删除已完成基础，PR #16 交付的 zh-CN/en-US、Help 和双语开发预览手册继续作为共享横向基础。

## 2. 统一路线图

| 阶段       | 目标                                                | 计划分支                                    | 状态           |
| ---------- | --------------------------------------------------- | ------------------------------------------- | -------------- |
| 工程基础   | Phase 0、会议 CRUD/API/UI、Provider、安全边界       | PR #1–#10                                   | 已完成并合并   |
| 文件基础   | 安全文件导入、可恢复删除、隔离 Docling Worker       | PR #11–#14                                  | 已完成并合并   |
| 转录规格   | 转录、输入快照和可追溯纪要规格                      | `docs/transcript-and-summary-spec` / PR #15 | 已完成并合并   |
| 国际化基础 | zh-CN/en-US、locale 偏好、Help 和双语预览手册       | `feat/i18n-user-guide-foundation` / PR #16  | 已完成并合并   |
| 简约版对齐 | 三种入口、共享管线、隐私边界和新顺序                | `docs/simplified-three-mode-spec` / PR #17  | 已完成并合并   |
| 总结数据   | schema v3、转录来源/片段、TXT/SRT/VTT/粘贴解析      | `feat/transcript-data-core` / PR #18        | 已完成并合并   |
| 转录接口   | Commands/API、原生选择器和原子创建边界              | `feat/transcript-command-api` / PR #19      | 已完成并合并   |
| 总结入口   | 首页入口、粘贴/导入、分页预览和说话人修正           | `feat/summary-mode-input-ui` / PR #20       | 已完成并合并   |
| 纪要数据   | schema v4、run/series/revision、正文快照和 evidence | `feat/summary-run-core`                     | 当前阶段       |
| 同意数据   | 通用 consent events、会话失效与审计 API             | `feat/consent-data-core`                    | 待开始         |
| 纪要服务   | SecretStore 消费、外发同意和受限 DeepSeek adapter   | `feat/summary-provider-deepseek`            | 待开始、需批准 |
| 纪要工作区 | 输入/模板选择、外发预览、结果和 Markdown            | `feat/summary-mode-minutes`                 | 待开始         |
| 编辑导出   | revision/stale、章节重生成和 DOCX                   | `feat/summary-editing-export`               | 待开始         |
| 文档编排   | Docling parse jobs、状态、artifact 原子提交和恢复   | `feat/document-parsing-orchestration`       | 待开始         |
| 文档证据   | chunks、页码/章节/表格定位和既有 Summary evidence   | `feat/document-evidence-index`              | 待开始         |
| 实时数据   | 模式提示、capture session、音频 artifact 与删除协调 | `feat/live-session-data-core`               | 待开始         |
| 实时进程   | Sidecar supervisor 与进程回收                       | `feat/sidecar-supervisor`                   | 待开始         |
| 音频采集   | 麦克风/系统音频、明确同意与恢复                     | `feat/audio-capture`                        | 待开始         |
| 实时转录   | 实时 ASR 写入统一 transcript segments               | `feat/realtime-transcription`               | 待开始         |
| 窗口上下文 | 用户确认目标、窗口身份、accessibility text          | `feat/window-context`                       | 待开始         |
| 屏幕 OCR   | 变化触发 OCR、screen evidence 和临时帧删除          | `feat/screen-ocr`                           | 待开始         |
| 语言数据   | translations、speaker aliases 和 checkpoint 快照    | `feat/live-language-data-core`              | 待开始         |
| 说话人增强 | 聚类、姓名校准、人工修改/合并和隔离 worker          | `feat/speaker-diarization`                  | 待开始         |
| 翻译服务   | 连续片段翻译、独立同意和取消                        | `feat/realtime-translation-provider`        | 待开始、需批准 |
| 阶段摘要   | 手动/固定时间调度、增量快照与重试                   | `feat/checkpoint-summary-runtime`           | 待开始         |
| 话题建议   | 可确认、忽略或修正的话题边界建议                    | `feat/topic-checkpoints`                    | 待开始         |
| 翻译浮窗   | 透明度、收起/展开、滚动、置顶和多显示器             | `feat/translation-overlay`                  | 待开始         |
| 翻译工作区 | 主窗口字幕、手动 speaker 与 checkpoint UI           | `feat/translation-mode-workspace`           | 待开始         |
| 重点数据   | 转录书签与用户文字笔记                              | `feat/meeting-notes-core`                   | 待开始         |
| 会议工作区 | 重点记录、阶段摘要、最终纪要和翻译开关              | `feat/meeting-mode-workspace`               | 待开始         |
| 图表增强   | 只基于可追溯结构化数据的固定图表                    | `feat/structured-visuals`                   | 后续           |
| 产品化     | Windows 安装包与加固                                | 见 §15                                      | 后续           |

除上表和各阶段明确列出的子分支外，不再使用表外的历史任务名或笼统阶段称呼。

## 3. Phase 0：规格与技术基线（已完成）

交付：核心规格文档、Tauri 2 + React + TypeScript 基线、Python 3.12 `/health`、React → Rust → Python 健康调用、精确依赖版本、lockfile 和 CI。

完成证据：PR #1 已通过前端/Python 和 Windows Rust CI；Windows 本机已实际运行 `npm run tauri dev` 并显示 MeetingLens 窗口。

## 4. MVP 1A-1：会议数据核心（已完成）

分支：`feat/meeting-data-core`

范围仅限：

- SQLite 连接与事务边界
- `schema_migrations` 与首批迁移
- meeting repository/service 的创建、读取、更新、列表和删除语义
- 应用私有数据目录和规范化路径基础
- 重启后的持久化验证、失败回滚和合成数据测试

不得包含：页面 UI、模拟转写、Provider、SecretStore、文档处理、音频或屏幕能力。

验收：迁移可从空库执行且可重复检测；CRUD 与删除语义有 Rust 测试；数据库文件保持本地并被 Git 忽略；应用重启后数据仍存在；本机运行最小持久化验收。

完成证据：PR #3 已 squash merge；Rust 数据层测试覆盖迁移、CRUD、回滚、路径守卫和重启持久化，Windows 本机已生成并重启验证应用私有 SQLite 数据库。

## 5. MVP 1A-2：Command API、桌面 UI 与模拟会议

MVP 1A-2 固定拆成以下两个顺序 PR。前一个 PR 合并后，后一个 PR 才能从最新 `main` 开始。

### 5.1 MVP 1A-2-1：Meeting Tauri Command API

分支：`feat/meeting-command-api`

范围：

- 将现有 meeting service 暴露为 `create_meeting`、`get_meeting`、`list_meetings`、`update_meeting`、`start_meeting`、`end_meeting` 和 `delete_meeting`
- 使用明确的 command/response DTO；command 不写 SQL，也不接受数据库路径
- 在 React/TypeScript 侧提供集中、严格类型且可 mock 的 `meetingApi`
- 使用稳定、脱敏的错误码，不向 WebView 泄漏 SQL、数据库路径或底层错误
- 通过专用 command 执行状态转换和后端 UTC 时间戳写入

生命周期契约：

- 创建会议得到 `draft`，`started_at` 与 `ended_at` 均为空
- `start_meeting` 只允许 `draft → recording`，并由 Rust 后端写入 `started_at`
- 在合成会议阶段，`end_meeting` 只允许 `recording → ready`，并由 Rust 后端写入 `ended_at`
- `processing` 保留给后续真实异步处理，只能由受信任后端进入；普通前端更新不能修改状态或生命周期时间戳
- 重复开始、重复结束、`draft → ready` 和 `ready → recording` 等转换使用稳定错误码 `meeting_invalid_state` 拒绝
- `delete_meeting` 只允许硬删除 `draft` 或 `ready`；`recording` 及当前不可进入的保留状态返回 `meeting_invalid_state`
- 删除不存在的会议返回 `meeting_not_found`；不得新增含义重复的 `invalid_state_transition` 错误码

不得包含：完整页面、表单、模拟字幕、participants、真实音频、Provider、SecretStore、Docling、DeepSeek、OCR 或 sidecar supervisor。

验收：Rust 测试覆盖每个 command、校验、合法/非法转换、删除状态限制和脱敏失败；TypeScript 测试覆盖 command 名称、序列化、响应解析和统一错误转换；Windows 本机实际调用 `create → list → start → end → delete`，重启后验证持久化。

### 5.2 MVP 1A-2-2：桌面 UI 与模拟会议

分支：`feat/meeting-ui-mock`

范围：

- 首页、新建会议、会议中、会议结果、设置等桌面页面和导航
- 使用合成数据的模拟转写流
- UI → Tauri command → 数据核心的完整模拟会议流程
- 加载、空状态、错误、取消与可访问性

不得包含真实音频、外部 Provider、Docling 或 DeepSeek。

验收：React 测试覆盖关键状态；用户在 Windows 本机通过 `npm run tauri dev` 完成“新建 → 模拟会议 → 结束 → 查看 → 删除”流程；刷新/重启后状态符合数据层定义。

完成证据：Meeting Command API 已由 PR #6 squash merge；桌面 UI 与模拟会议闭环已由 PR #7 squash merge。Windows 本机已验证创建、开始、模拟字幕暂停/恢复、结束、重启持久化和删除，数据库完整性检查通过。

## 6. MVP 1A-3：Provider 与安全基础

该阶段固定拆成三个顺序 PR。1A-3A 合并后，1A-3B-1 才能从最新 `main` 开始；1A-3B-1 合并后才能开始 1A-3B-2。

### 6.1 MVP 1A-3A：Provider 契约与 Mock Runtime

分支：`feat/provider-contracts-mocks`

范围：

- 定义 Capture、Transcription、Translation、Summary 和 DocumentParser Provider 的输入、输出、状态、错误、取消与清理契约
- 提供确定性的纯内存 Mock，实现正常、失败和取消路径
- 使用轻量 Registry 进行应用层生命周期管理和测试注入
- 将模拟会议字幕迁移到 Transcription Provider，设置页明确显示 Mock 与未启用状态
- 当前契约位于 TypeScript UI/业务边界；在没有真实 Rust/Python 调用链前不复制无用途的 Provider 层

不得包含：SecretStore、会话令牌、认证、安全日志、真实设备/文件/网络访问、数据库 migration、DeepSeek、Docling、Whisper、Screenpipe 或 sidecar 改造。

验收：Mock 状态机、取消、清理、稳定错误与 Registry 注入有自动化测试；旧模拟字幕架构被完整替换；Windows 本机验证两场连续模拟会议没有重复订阅或计时器残留。

完成证据：PR #8 已 squash merge；五类 Provider 契约、Registry、纯内存 Mock、模拟字幕迁移和 Windows 双会话验收均已完成，Provider/UI 不访问真实设备、文件或网络。

### 6.2 MVP 1A-3B-1：本地 Sidecar 会话认证

分支：`feat/local-sidecar-auth`

范围：

- Rust 每次应用启动使用操作系统 CSPRNG 生成临时会话令牌，只通过子进程环境传给 Python
- Rust 取得安全传递令牌所需的最小 Sidecar 启动所有权和退出清理，不实现 supervisor 重启/恢复循环
- Python 只允许 `127.0.0.1`，全部路由（包括 `/health`）统一验证 Bearer 令牌
- Rust `SidecarClient` 集中注入认证 Header，并提供超时、401、无效响应等稳定脱敏错误
- 设置页只显示已认证/不可用/错误、仅本机绑定、当前会话和 SecretStore 未实现状态
- 当前固定开发端口可继续使用；随机端口与完整生命周期接线留给 `feat/sidecar-supervisor`

不得包含：SecretStore、Windows Credential Manager、DeepSeek/API Key、真实 Provider、文档、音频、屏幕、完整日志系统、崩溃重启或孤儿进程恢复。

验收：无 Header、错误 scheme/令牌和非 loopback Host 全部 fail closed；令牌不出现在命令行、WebView DTO、SQLite 或日志；应用重启生成新会话且旧测试令牌失效；Windows 本机完成认证、绑定、进程、日志、模拟会议和清理验证。

### 6.3 MVP 1A-3B-2：SecretStore 与安全配置

分支：`feat/provider-security-foundation`

范围：

- Rust `SecretStore` 抽象和 Tauri App State；测试不得读取或覆盖正式凭据
- Windows Credential Manager `CRED_TYPE_GENERIC` 后端；非 Windows 平台 fail closed，不提供明文回退
- 只允许 `deepseek_api_key`，正式 target 固定为 `com.gray.meetinglens/secret/deepseek-api-key`
- React 只可调用 status、set 和 delete；已保存的值不能读取、显示、复制或回填
- `main` 窗口的最小 Tauri permission set；不授权远程 WebView 或通用 shell/fs/http/process 能力
- 凭据安全写入、读取状态、替换和幂等删除接口；当前不调用 DeepSeek，也不把 key 传给 Python Sidecar
- 稳定脱敏错误协议；完整结构化日志框架仍属于后续阶段

不得调用真实付费 API，也不得实现 Docling、DeepSeek、音频或屏幕采集。

验收：空白或超长输入失败关闭；WebView 不能读回凭据；SQLite、浏览器存储、日志、命令行和构建输出中不出现密钥；Windows 本机使用隔离测试 target 验证写入/读取/替换/删除，并通过设置页验证正式 target 的重启持久化和最终清理，但任何位置都不显示值。

## 7. 转录优先的纪要主路径与文档增强

### 7.1 已完成：定义文档导入与删除生命周期

分支：`docs/define-document-import-lifecycle`

仅更新规格，固定：不可信文件边界、原生选择器来源、格式 allowlist、配额、meeting/document 状态、UUID 私有路径、staging/原子 rename、同会议 SHA-256 去重、会议/文档删除 tombstone、稳定错误码和后续分支。

不得修改 Rust、React、Python、migration、依赖或 lockfile，不得声称文件导入、Docling 或删除协调已实现。

验收：七份项目文档没有格式/配额/状态/分支冲突；Prettier、Markdown 链接、全项目检查、敏感信息和禁止文件扫描通过；PR 保持未合并等待用户决定。

### 7.2 已完成：文档存储核心

分支：`feat/document-storage-core`

范围：

- documents migration 与元数据 service
- 应用私有 `meeting-data` 布局和受控相对路径
- 安全流式复制、100 MiB/数量/累计配额和 UTF-8 文件名限制
- allowlist、扩展/签名基础识别、OOXML 基础容器识别和未知类型 fail closed
- SHA-256、同会议重复文件检测和原子 staging → 正式目录
- 文档删除协调、会议删除 tombstone、`deleting/delete_failed` 与启动重试基础
- 只使用合成文件，无正式 UI

验收：覆盖大小变化/复制中超限、类型不一致、双重扩展、路径逃逸、重复内容、事务/rename 失败、删除失败和崩溃恢复；Windows 本机验证私有路径、原子移动、源文件不变和无孤儿文件。本 PR 不引入 Docling。

完成基线：schema migration v2 创建 `documents`/`deletion_jobs`；Rust 内部 `DocumentService` 执行流式复制、SHA-256、签名/OOXML 中央目录/UTF-8 检查、固定配额、UUID 私有路径、删除协调和启动对账。应用生命周期文件锁拒绝第二写入实例，进程内 mutex 串行化 mutation；PR #12 没有 Document command、picker 或 React 文件 API。

### 7.3 已完成：文档导入 UI

分支：`feat/document-import-ui`

范围：

- Rust/Tauri Document Commands 和集中式、可 mock 的 `documentApi`
- MeetingLens 原生文件选择器；WebView 不取得任意文件系统权限或绝对路径
- 会议详情和会议中页面的文档列表、添加、导入进度、安全错误
- 删除确认、pending/failed 状态和重试交互
- Rust 最终执行状态、类型和配额规则；页面不能直接写状态

验收：Windows 本机通过原生选择器导入 allowlist 合成文件，验证重启持久化、状态冻结、重复检测、删除/重试及无路径泄漏。本 PR 不解析文件。

实现基线：React 只通过集中、可替换的 `documentApi` 调用六个自定义 commands；`select_and_import_documents` 在 Rust 中打开原生多选对话框，并通过有类型的 Tauri channel 回传有界进度。DTO 不包含源路径、存储路径、哈希或内部错误。会议详情提供完整列表、导入、删除和重试，会议中页面提供紧凑状态；最终状态、格式、去重和配额仍由 `DocumentService` 执行。

### 7.4 已完成：Docling Worker 基础

分支：`feat/docling-worker`

范围：

- 独立 Python Worker 环境、精确 lockfile、许可与模型下载门槛
- 只接收 Rust 注册的受控文档引用，不接受任意绝对路径
- 单请求进程、取消、超时、Windows Job Object 内存限制和完整子进程树回收
- 页数、输入、协议和解析产物输出限制；固定相对产物清单及 Rust 二次完整性校验
- 明确离线、CPU-only、无插件/远程服务/隐式模型下载；模型来源、revision 和本地 manifest 固定
- 畸形协议、路径逃逸、篡改、崩溃、超时、取消和资源耗尽自动化测试

验收：只使用合成文档；Windows/Python 3.12 本机验证 TXT/Markdown/CSV/Office/PDF/图片/OCR、内存、冷启动、取消和清理。CI 不下载模型，不能替代本地 PDF/OCR 验收。本 PR 不迁移数据库、不注册 Tauri command、不改 UI 或文档状态、不调用 DeepSeek。

完成证据：PR #14 已合并。Worker 具备独立 Python 3.12/uv lock、显式离线模型、JSONL stdio、Windows Job Object、4 GiB 限制、超时/取消/崩溃回收及 TXT/Markdown/CSV/Office/PDF/图片/OCR 本机 Smoke；尚未接数据库、Provider、Commands 或 UI。Docling 作为后续参考文档增强保留，不是 transcript-only 纪要的前置依赖。

### 7.5 转录与纪要规格对齐（已完成，PR #15）

分支：`docs/transcript-and-summary-spec`

纯文档范围：冻结产品核心价值、`transcript_sources`/`transcript_segments`、相对 offset、输入/删除/修正语义、Summary 输入快照、evidence 规则、三个模板和首页快速入口。不得修改代码、migration、Provider 契约实现、依赖或 lockfile。

验收：PR #1–#14 状态正确；七份核心文档与 Worker README 不再保留旧路线、阶段、数据模型或时间语义冲突；格式、链接、全项目和敏感信息检查通过。PR #15 已完成并合并。

### 7.6 国际化与开发预览手册（已完成，PR #16）

分支：`feat/i18n-user-guide-foundation`

横向范围：一个安装包内提供 `zh-CN` 与 `en-US`，以 `en-US` 可靠回退；设置页即时切换并只将允许的 `meetinglens.uiLocale` 枚举保存为非敏感 WebView 偏好；当前 UI 状态、稳定错误码、日期、数字和文件大小集中本地化；提供安全静态 Help 页面和结构一致的中英文开发预览使用手册。

该阶段没有创建 migration，没有改变会议、文档或 Provider 数据，也没有自动把 UI locale 当作未来纪要输出语言；它未实现 transcript、DeepSeek、Docling 编排、音频或屏幕能力。PR #16 已完成并合并。

### 7.7 简约三模式规格对齐（已完成，PR #17）

分支：`docs/simplified-three-mode-spec`

纯文档范围：冻结三种入口、共享数据流、“原始音频/逐字稿/会议纪要”术语、阶段摘要触发器、权限降级、首轮明确不做和新的小 PR 顺序。不得修改 React、Rust、Python、migration、依赖或 lockfile。

验收：`SIMPLIFIED_MODE_SPEC.md` 与 `AGENTS.md`、`PRODUCT_SPEC.md`、`ARCHITECTURE.md`、`DATA_MODEL.md`、`SECURITY.md`、`README.md` 使用相同分支名和边界；旧版未实现路线被明确冻结；PR #16 的 i18n、Help 和双语手册继续保留且与三模式现状说明一致。PR #17 已完成并合并。

## 8. 总结模式

### 8.1 Transcript Data Core（已完成，PR #18）

分支：`feat/transcript-data-core`

范围：

- schema v3：`transcript_sources`、`transcript_segments` 和必要索引
- 粘贴 UTF-8、TXT、SRT、VTT 的轻量同步 parser，不调用 Docling/网络
- `pasted_text/imported_txt/imported_srt/imported_vtt`；`live_asr/platform_export` 仅保留
- 2 MiB 粘贴、10 MiB 文件、20,000 segments/source、64 KiB/segment 和严格控制字符限制
- 同会议来源 SHA-256 去重、revision、source/segment 删除语义
- 相对 `start_offset_ms/end_offset_ms` 与可空 source `base_started_at`

本阶段不做 UI、Commands、DeepSeek、平台专用 adapters 或原始转录文件长期存储。实现时同步调整 Provider/API 的 `TranscriptionSegment` 时间字段，不能保留 ISO 与 offset 两套真相。

完成证据：PR #18 已完成 schema v3、四类轻量输入解析、同会议去重、segment revision、source 删除、稳定 Summary 输入 DTO、v2 → v3 升级与 Windows 本机持久化 Smoke，并已合并。

### 8.2 Transcript Command API（已完成，PR #19）

分支：`feat/transcript-command-api`

范围：

- Transcript Tauri Commands、集中式 `transcriptApi` 和严格无路径 DTO
- Rust 原生单文件 TXT/SRT/VTT 选择器；取消为无副作用的正常结果
- 原子创建 draft meeting 与 transcript，或向可接受状态的既有 meeting 添加 transcript
- source/segment 查询、乐观 revision 修正、source 删除和固定导入策略
- WebView 不获得文件路径或 dialog/fs/shell/process/http 权限

本 PR 不实现正式页面、首页快速入口、DeepSeek、Summary schema/Provider、平台专用导出、视频/音频文件转录、真实音频、屏幕或 Docling 接线。

完成证据：PR #19 已交付 10 个安全 Transcript Commands、集中式 `transcriptApi`、Rust 原生 TXT/SRT/VTT 选择器、原子 meeting + transcript 创建、向既有 meeting 添加、source/segment 查询、乐观 revision 修正与 source 删除；路径不进入 WebView，取消不创建空 meeting。Windows 本机、SQLite 完整性和 CI 验收通过后已合并。

### 8.3 Summary Mode Input UI（已完成，PR #20）

分支：`feat/summary-mode-input-ui`

范围：首页三个模式入口中的总结模式、粘贴/原生导入交互、来源/片段预览、基础 speaker/正文修正，以及不承诺可用的实时模式状态说明。只消费已合并的 `transcriptApi`；页面不得直接调用 `invoke`，不得取得路径或引入第二套 parser。单个来源最多允许 20,000 个 segments，因此本 PR 必须通过稳定 `ordinal` 顺序和生产单页上限 200 的后端分页读取，不能先取得全量再在 React 切片。为避免逐条修改同一来源中的重复标签，可以增加 source-scoped 批量说话人显示名修正：只更新匹配 segment 的 `speaker_name`、revision 和 `corrected_at`，保留 `speaker_label`，不得跨 source/meeting，也不得引入声纹或身份推断。

本 PR 不调用 DeepSeek，不新增 transcript/Summary schema，不实现 Docling 接线、真实音频、屏幕、翻译或平台专用 adapter。

完成证据：PR #20 已交付三模式首页、总结模式粘贴与 Rust 原生 TXT/SRT/VTT 导入、来源与后端有界分页、正文修正和 source-scoped speaker 显示名修正；禁用的纪要入口不调用网络，Windows 本机、SQLite 持久化和 CI 验收通过后已合并。

### 8.4 当前：Summary Run Core

分支：`feat/summary-run-core`

范围：schema v4 的 `summaries`、`summary_sources` 与 `evidence_items`，Summary run/series/revision、transcript-only 不可变 input snapshot、稳定状态/错误、资格校验和 Rust `SummaryProvider` 边界。每个新系列从 revision 1 开始，`series_id + revision` 唯一；重新生成在事务内分配下一 revision 并重新冻结当前输入，不覆盖旧 run。快照实际复制 source/segment IDs、segment revision、source/segment/global ordinal、source 显示元数据、speaker label/name、offset、规范化正文、language、confidence、`is_final` 和快照时间；Provider 运行时只读快照，不回读 live segment。

本阶段只创建 `run_purpose=final`、`trigger_kind=manual`，每个 run 最多冻结 20,000 个 final segments 和 10 MiB UTF-8 正文，超限整体拒绝、不截断。状态机固定为 `queued → running → completed/failed/cancelled` 和 `queued → cancelled`；启动恢复把遗留 `running` 标为 `failed + summary_interrupted`。确定性 Rust Mock 不读取 SecretStore、环境凭据、文件、Sidecar 或网络；Provider 输出必须先在 Rust 中完成结构、大小和 evidence allowlist 校验，再原子写入结果与当次实际引用的 evidence，未知或跨 run ID 必须拒绝。

本 PR 交付 `get_summary_policy`、`create_summary_run`、`create_summary_revision`、`execute_summary_run`、`get_summary_run`、`list_summary_runs`、`cancel_summary_run` 和 `list_summary_evidence` 八个最小 Tauri Commands，以及集中式 TypeScript `summaryApi`。WebView 只提交用户选择和 run ID，不能提交 Provider 结果或 evidence，也不获得 dialog/fs/shell/process/http 权限。生成资格允许 `draft/recording/ready + transcript`，禁止 `deleting/delete_failed`、空输入和无法安全冻结的输入；不得新增 summary 型 meeting 状态。

所有 command/result DTO（含嵌套结构化结果）统一 camelCase；run/evidence 列表均使用默认 50、最大 100 的 keyset 分页。schema 独立保存 `provider_mode`，当前固定 `mock`；revision lineage 由数据库约束同 meeting/series 的直接前驱，再由 Service 在 IMMEDIATE 事务中保证 terminal/latest。竞态 stale base 与未知/重复/跨 run evidence 分别返回 `summary_revision_conflict` 和 `summary_evidence_invalid`。

segment 修正只把依赖旧 revision 的 run 标记 stale，不改变其快照或结果；queued/running run 使用的 source 删除稳定拒绝，terminal source 删除清理完整正文/显示元数据快照，只保留已引用的最小 evidence 并标记 `source_deleted`/stale。会议删除先取消 active Summary run，再通过既有协调器删除该会议的 summaries、snapshots 和 evidence。当前 PR 不实现 Consent、DeepSeek、正式纪要页面、持久编辑/导出、Docling、document/screen/manual-note evidence、checkpoint、音频、翻译或屏幕能力。

验收必须用 development-only、系统 Temp 直属隔离 app-data 根完成两次 `npm run tauri dev`，并为两次启动合并与根目录 UUID 相同的合成 Tauri identifier，隔离 SQLite 与 WebView2 profile，不得触碰用户正常数据库或浏览器存储。第一次覆盖 queued/completed/evidence/stale/revision/cancel/source 删除与零外网/零正文浏览器持久化；第二次复验 run/result/revision/stale/evidence、遗留 running 恢复、SQLite integrity 和全部合成数据/文件/profile/进程/端口清理。production dist 不得包含 harness。只有本 PR 由用户合并后，下一阶段才是 `feat/consent-data-core`。

### 8.5 Consent Data Core

分支：`feat/consent-data-core`

范围：通用 `consent_events` migration、capability allowlist、grant/revoke、当前应用会话失效、审计查询和稳定 commands/API。不得访问设备、调用外部 Provider、保存正文/凭据或增加产品工作区。后续 Summary、音频、翻译、checkpoint 和屏幕阶段只复用并在文档先行的前提下扩展该能力，不得各建一套授权记录。

### 8.6 DeepSeek Summary Provider

分支：`feat/summary-provider-deepseek`

范围：Rust 从 SecretStore 取得 key、受限 DeepSeek adapter、超时/重试/取消、请求/响应 schema、外发同意记录和日志脱敏。不得新增 schema、修改转录持久化或实现正式结果 UI。

真实 DeepSeek 调用会产生外部数据处理和潜在费用；实现前必须重新向用户确认 Provider、模型、价格/条款和发送范围。测试只使用 Mock 与合成数据，不得消费用户曾在聊天中提供的测试 key。

### 8.7 Summary Mode Minutes Workspace

分支：`feat/summary-mode-minutes`

范围：来源/模板/语言选择、外发前的 segment 数/时间范围/speaker/数据类型预览、本次确认、运行状态、结果、有效 evidence 导航，以及 Markdown 查看、复制和下载。只集成已合并的 Summary core/Provider，不新增数据库、Provider、Docling、DOCX 或持久编辑。

模板为详实会议纪要、简洁会议总结、行动项与决定清单；输出为中文、英文或双语。不得发明参会人、负责人、截止日期或共识；缺负责人标“未分配”，缺截止日期为空。每项关键决定、行动项和争议必须带当前快照中的有效 evidence。

### 8.8 Summary Editing and Export

分支：`feat/summary-editing-export`

范围：用户编辑保存、summary revision、输入变化 stale 提示、speaker 修正后显式重新生成、章节级重生成、DOCX 和完整历史。重新生成不能覆盖用户编辑章节，后台不得静默重生成；PDF 留到产品化阶段。

## 9. 总结模式参考材料增强

该阶段只在 transcript-only 纪要、工作区和编辑导出闭环完成后开始，并固定拆为两个顺序 PR。Docling 不得成为第一份 transcript-only 纪要的前置条件。

### 9.1 Document Parsing Orchestration

分支：`feat/document-parsing-orchestration`

范围：基于合并时最新 schema 顺序递增 migration，建立 parse jobs、调用已完成的隔离 Docling Worker、持久化文档状态转换、原子提交受控 artifacts，并实现取消、重试、崩溃恢复和安全 UI。本 PR 不调用 Summary Provider，不创建 document evidence，不实现音频。

### 9.2 Document Evidence Index

分支：`feat/document-evidence-index`

范围：创建 `document_chunks` 及页码、章节、表格和结构位置，生成经过 allowlist 校验的 document evidence，并接入既有 Summary pipeline。文档提供背景和材料，不能在没有 transcript 或人工确认时自动证明会议决定。本 PR 不创建第二套 Summary 或 transcript 体系。

两个 PR 完成后，总结模式可在同一纪要管线中使用 PPT、PDF、Word 等参考材料；粘贴/TXT/SRT/VTT 仍使用 Rust 轻量 parser。文档中写着某项计划不等于会议已经确认该计划。

## 10. 共享实时采集与 ASR

### 10.1 Realtime Capture Foundation

该基础固定拆为三个顺序 PR：

1. `feat/live-session-data-core`：可空 workflow mode 提示、capture session、可选音频 artifact/保留策略和删除协调；复用已有 consent service，不访问设备、不启动进程。
2. `feat/sidecar-supervisor`：sidecar 打包、随机端口接线、健康恢复、崩溃重启、退出清理、孤儿进程回收和生命周期安全日志。
3. `feat/audio-capture`：WASAPI 系统音频与麦克风、设备枚举、分别同意、明确开始/停止、持续采集指示和异常恢复；只消费既有数据/删除能力，不新增 schema。

保存完整听录文本、保存原始音频和保存屏幕关键帧是三个独立选项；原始音频默认关闭。“不导出逐字稿”不能被实现为“没有规范化转录”。

### 10.2 Real-time ASR

分支：`feat/realtime-transcription`

范围：实时 ASR 直接写入既有 `transcript_sources/transcript_segments`，实现片段顺序/去重、相对 offset、CPU 回退、取消与会后校正入口。不得创建第二套转录模型，也不得顺带实现翻译、屏幕或说话人分离。

验收：`feat/live-session-data-core` 使用合成会话与保留/删除场景验证数据边界；`feat/sidecar-supervisor` 使用故障注入并在 Windows 本机验证启动、恢复和进程回收；只有 `feat/audio-capture` 与 `feat/realtime-transcription` 使用合成音频自动测试，并在 Windows 本机验证真实设备、权限、采集与 ASR 行为。所有涉及进程或文件句柄的阶段在应用退出或崩溃后都不得留下孤儿进程或句柄，日志不得包含音频、转录正文、设备 ID 或绝对用户路径。

## 11. 屏幕与窗口上下文

### 11.1 Window Context

分支：`feat/window-context`

范围：用户显式选择单个目标窗口、验证窗口身份、持续显示正在读取的应用、支持暂停/撤销并提取 accessibility text。目标窗口关闭、身份变化或捕获对象被替换时必须立即暂停并要求重新确认；不得默认监控全部屏幕。

### 11.2 Screen OCR

分支：`feat/screen-ocr`

范围：本地变化检测与 OCR 回退、敏感内容遮蔽、screen events 与 transcript 时间线对齐、用户确认的 evidence 和临时帧删除。屏幕关键帧默认不保存，画面变化本身不得自动确认决定或触发最终总结。

Screenpipe 只能是可替换的可选 Capture Provider，不得成为核心强依赖。采用前必须获得用户批准，并复审商业许可、默认采集范围、本地 API 暴露和 Windows 资源占用。

## 12. 说话人分离与命名

### 12.1 Live Language Data Core

分支：`feat/live-language-data-core`

范围：权威 `translations` 表、segment revision 对应关系、会话级 `speaker_aliases`、checkpoint 增量快照/游标和必要索引。该纯数据阶段不得访问外部 Provider、调度定时任务或增加正式 UI；译文不得写回 `transcript_segments`。

### 12.2 Speaker Diarization and Naming

分支：`feat/speaker-diarization`

范围：会话级匿名 speaker 聚类、开场姓名校准、会后人工修改/合并，以及隔离的实时/会后校正 worker。不得默认保存 voice sample，不承诺跨会议声纹自动复用。Diart 与 WhisperX 必须保持独立环境、进程和 lockfile，采用模型前须审查条款和资源预算。

## 13. 实时翻译

### 13.1 Realtime Translation Provider

分支：`feat/realtime-translation-provider`

范围：只消费最终转录片段的实时 Translation Provider、持续外发的独立同意、关闭/取消、顺序、重试和错误脱敏。不得新增 schema、checkpoint 调度或正式 UI。真实外部 Provider 启用前必须重新取得用户对费用、条款和持续发送范围的批准。

### 13.2 Checkpoint Summary Runtime

分支：`feat/checkpoint-summary-runtime`

范围：用户手动触发和“每 5 分钟且存在新最终片段”的调度；每次只冻结上一个成功 checkpoint 后的新片段，失败重试复用同一快照；独立 `checkpoint_external_ai` 同意、关闭后取消排队任务。不得实现自动话题判断或正式页面。

### 13.3 Topic Checkpoints

分支：`feat/topic-checkpoints`

范围：在真实转录质量稳定后提供可确认、忽略或修正的话题边界建议。不得静默切段，不得把建议自动写成会议决定，也不得替代手动重点标记。

### 13.4 Translation Overlay

分支：`feat/translation-overlay`

范围：Windows 置顶、透明度、收起/展开、滚动、点击穿透和多显示器/DPI 验收。只消费已合并的转录和翻译能力，不新增 schema、Provider 或采集范围。

## 14. 翻译模式与会议模式集成

### 14.1 Translation Mode Workspace

分支：`feat/translation-mode-workspace`

范围：只集成已合并的数据、Provider、speaker、checkpoint 和 overlay；显示原文/译文并允许手动改名，展示阶段摘要和会后从原始转录快照生成的最终纪要。不得新增 schema 或 Provider。

### 14.2 Meeting Notes Core

分支：`feat/meeting-notes-core`

范围：可追溯的转录片段范围书签、独立用户文字笔记、修正/删除、Summary snapshot/evidence 接入和稳定 commands/API。不得增加会议模式页面、屏幕、OCR 或模型调用。

### 14.3 Meeting Mode Workspace

分支：`feat/meeting-mode-workspace`

范围：只集成已合并的实时转录、屏幕上下文、speaker、翻译、checkpoint 和 meeting notes；加入可选翻译、手动重点记录、阶段摘要、可选逐字稿导出和最终纪要。没有屏幕权限时仍必须可正常完成音频会议；没有音频时允许切换到总结模式。不得新增 schema 或 Provider。

### 14.4 后续结构化视觉

`feat/structured-visuals` 只能从校验过且可追溯的数据生成固定时间线、行动项表或决定表，不执行模型生成代码，默认不生成图表。

## 15. 产品化：安装包与加固

- `chore/windows-packaging`：Windows 安装包、签名、SBOM、升级/回滚、安装与卸载冒烟测试。
- `feat/product-hardening`：托盘、诊断/隐私设置、长期稳定性、性能、崩溃恢复和删除完整性。

验收必须包含用户本机安装、启动、升级、卸载和残留数据检查；仅有 CI bundle 成功不算完成。

## 16. 测试与验收矩阵

| 层级   | 自动化                              | 本地人工验收                 |
| ------ | ----------------------------------- | ---------------------------- |
| 文档   | Prettier、链接与冲突扫描            | 路线图/分支名审阅            |
| React  | Vitest、Testing Library、typecheck  | Tauri 窗口完整流程           |
| Python | unittest、Ruff、Python 3.12         | worker 启停、模型/资源行为   |
| Rust   | fmt、test/check `--locked`          | 权限、文件、进程、设备行为   |
| 安全   | secret/path/upload/fail-closed 测试 | 凭据库、同意、日志和删除验证 |

所有测试数据均为合成数据。CI 失败时不得声称完成；Windows 桌面能力未做本机验收时，必须明确标记“自动化通过，本地验收待完成”。

## 17. Definition of Done

1. PR 范围与本计划对应阶段一致，且没有夹带下一阶段功能。
2. 正常、失败、取消和安全路径有测试或明确的本机验收记录。
3. 文档、类型、迁移和实现同步，直接依赖精确锁定。
4. staged files 不含凭据、会议数据、录音、截图、上传文件、数据库或日志。
5. diff 已检查，提交清晰，分支已 push，PR 描述包含测试与限制。
6. CI 全绿；涉及 Windows 桌面行为时本机验收完成。
7. PR 保持未自动合并，等待用户决定。

## 18. 当前下一步

PR #1–#20 已合并；三模式路线、i18n、Help、中英文开发预览手册、Docling Worker Foundation、Transcript Data Core、Transcript Command API 和 Summary Mode Input UI 已完成，Docling 尚未接入产品。当前阶段为 `feat/summary-run-core`：只交付 schema v4、持久不可变 transcript snapshot、run/series/revision、Rust zero-network Mock、evidence allowlist、稳定状态/错误/恢复、删除协调和开发 API；普通用户的“生成会议纪要”仍禁用，真实 AI 与正式结果 UI 仍不可用。

`feat/summary-run-core` 必须从已合并 PR #20 的最新 `main` 独立完成并停在待审 PR，不得同时启动 Consent、外部 Provider、Docling、音频、翻译或屏幕功能。只有该 PR 经用户审查并明确合并后，下一阶段才是 `feat/consent-data-core`；当前任务不得预先开始它。首个 transcript-only 纪要与编辑导出闭环完成后，必须先完成 `feat/document-parsing-orchestration` 和 `feat/document-evidence-index`，再进入实时采集与 ASR；Docling 仍不阻塞第一份纪要。未来 GitHub PR 编号只在实际创建后记录，不是计划契约。
