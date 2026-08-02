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

PR #14 合并后，路线以“尽快生成真实、可追溯纪要”为价值中心重新排序：先补一等公民 transcript 数据和导入路径，再交付 transcript-only Summary；Docling 保留为参考文档增强并在 PR #20 接线，屏幕能力排在说话人增强和实时翻译之前。

## 2. 统一路线图

| 阶段/PR      | 目标                                                 | 计划分支                                         | 状态         |
| ------------ | ---------------------------------------------------- | ------------------------------------------------ | ------------ |
| Phase 0      | 规格与技术基线                                       | `docs/project-specification` / PR #1             | 已完成并合并 |
| MVP 1A-1     | SQLite、迁移、会议 CRUD、删除语义和持久化            | `feat/meeting-data-core` / PR #3                 | 已完成并合并 |
| MVP 1A-2-1   | Meeting Tauri Command API 与 TypeScript 客户端       | `feat/meeting-command-api` / PR #6               | 已完成并合并 |
| MVP 1A-2-2   | 桌面 UI、模拟转写和完整模拟会议流程                  | `feat/meeting-ui-mock` / PR #7                   | 已完成并合并 |
| MVP 1A-3A    | Provider 契约、Registry 和内存 Mock                  | `feat/provider-contracts-mocks` / PR #8          | 已完成并合并 |
| MVP 1A-3B-1  | Sidecar 会话认证、Loopback 和基础日志脱敏            | `feat/local-sidecar-auth` / PR #9                | 已完成并合并 |
| MVP 1A-3B-2  | SecretStore、Windows Credential Manager              | `feat/provider-security-foundation` / PR #10     | 已完成并合并 |
| 文档基线     | 文件导入生命周期与删除语义                           | `docs/define-document-import-lifecycle` / PR #11 | 已完成并合并 |
| 文档存储     | 安全导入、去重、删除协调与 UI                        | PR #12–#13                                       | 已完成并合并 |
| Docling 基础 | 隔离 Worker、离线模型与受限进程协议                  | `feat/docling-worker` / PR #14                   | 已完成并合并 |
| PR #15       | 转录、快速纪要、输入快照和新路线规格                 | `docs/transcript-and-summary-spec`               | 当前阶段     |
| PR #16       | schema v3、转录来源/片段、TXT/SRT/VTT/粘贴解析       | `feat/transcript-data-core`                      | 待开始       |
| PR #17       | 转录 Commands/API、导入/粘贴/预览和首页快速入口      | `feat/transcript-import-ui`                      | 待开始       |
| PR #18       | transcript-only DeepSeek 纪要、模板、证据与 Markdown | `feat/deepseek-summary-mvp`                      | 待开始       |
| PR #19       | 用户编辑、revision/stale、章节重生成和 DOCX          | `feat/summary-editing-export`                    | 待开始       |
| PR #20       | Docling 解析编排、产物提交、取消/重试/恢复           | `feat/document-parsing-orchestration`            | 已排期       |
| PR #21       | 文档 chunks/页码/章节/表格证据接入 Summary           | `feat/document-evidence-index`                   | 已排期       |
| 后续         | Sidecar supervisor、真实音频、实时 ASR               | 见 §8                                            | 待开始       |
| 后续         | 屏幕窗口与 screen events                             | 见 §9                                            | 待开始       |
| 后续         | 说话人增强与实时翻译                                 | 见 §10                                           | 待开始       |
| 产品化       | Windows 安装包与加固                                 | 见 §11                                           | 待开始       |

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
- 当前固定开发端口可继续使用；随机端口与完整生命周期接线留给 MVP 1C

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

### 7.5 PR #15：转录与纪要规格对齐

分支：`docs/transcript-and-summary-spec`

纯文档范围：冻结产品核心价值、`transcript_sources`/`transcript_segments`、相对 offset、输入/删除/修正语义、Summary 输入快照、evidence 规则、三个模板、首页快速入口和 PR #16–#21 顺序。不得修改代码、migration、Provider 契约实现、依赖或 lockfile。

验收：PR #1–#14 状态正确；七份核心文档与 Worker README 不再保留旧路线、阶段、数据模型、时间语义或分支顺序冲突；格式、链接、全项目和敏感信息检查通过。

### 7.6 PR #16：Transcript Data Core

分支：`feat/transcript-data-core`

范围：

- schema v3：`transcript_sources`、`transcript_segments` 和必要索引
- 粘贴 UTF-8、TXT、SRT、VTT 的轻量同步 parser，不调用 Docling/网络
- `pasted_text/imported_txt/imported_srt/imported_vtt`；`live_asr/platform_export` 仅保留
- 2 MiB 粘贴、10 MiB 文件、20,000 segments/source、64 KiB/segment 和严格控制字符限制
- 同会议来源 SHA-256 去重、revision、source/segment 删除语义
- 相对 `start_offset_ms/end_offset_ms` 与可空 source `base_started_at`

本 PR 不做 UI、Commands、DeepSeek、平台专用 adapters 或原始转录文件长期存储。实现时同步调整 Provider/API 的 `TranscriptionSegment` 时间字段，不能保留 ISO 与 offset 两套真相。

### 7.7 PR #17：Transcript Import UI

分支：`feat/transcript-import-ui`

范围：

- Transcript Tauri Commands、集中式 `transcriptApi` 和无路径 DTO
- Rust 原生 TXT/SRT/VTT 选择器、粘贴面板、解析预览、来源列表和基础 speaker 修正
- 首页入口：开始实时会议、粘贴会议记录、导入会议转录、导入会议材料、历史会议
- 会议日期默认用户本地今天且可编辑；高级“新建会议”向导继续保留
- 自动创建 meeting 或选择已有 meeting；用户不必手动操作内部状态机

本 PR 不调用 DeepSeek，不实现平台专用导出、视频/音频文件转录或 Docling 接线。

### 7.8 PR #18：DeepSeek Summary MVP

分支：`feat/deepseek-summary-mvp`

范围：

- transcript-only SummaryProvider；Rust 从 SecretStore 取得 key，React/Docling/日志不得读取
- 用户选择来源后创建不可变 run/revision 快照：source/segment IDs、segment revision/order、speaker、offset、language、模板、输出语言和创建时间
- 外发前显示来源、segment 数、时间范围、speaker、模板、语言、文档包含状态和发送内容类型，并要求本次确认
- 三个模板：详实会议纪要、简洁会议总结、行动项与决定清单；中文、英文、双语
- 分段事实提取、主题归并、decision/action/open question/conflict/risk 和 evidence ID 验证
- Markdown 与一键复制；不接 Docling、不做 DOCX、不做章节重生成

生成资格取决于至少一个可冻结的用户确认输入。允许 `draft/recording/ready + transcript`，禁止 `deleting/delete_failed`、空输入和无法安全冻结的输入；不得新增 summary 型 meeting 状态。

行为规则：不得发明参会人、负责人、截止日期或共识；缺负责人标“未分配”，缺截止日期为空；decision 区分 `proposed/confirmed/superseded`，行动项区分明确分配/建议/未确认；冲突和“无法确定”必须保留。每项关键决定、行动项和争议必须带当前快照中的有效 evidence，未知 ID 不接受。

### 7.9 PR #19：Summary Editing and Export

分支：`feat/summary-editing-export`

范围：用户编辑保存、summary revision、输入变化 stale 提示、speaker 修正后显式重新生成、章节级重生成、DOCX 和完整历史。重新生成不能覆盖用户编辑章节，后台不得静默重生成；PDF 留到产品化阶段。

### 7.10 PR #20：Document Parsing Orchestration

分支：`feat/document-parsing-orchestration`

范围：基于合并时最新 schema 顺序递增 migration（不得再称 v3）、解析任务、Docling Worker 接入、artifact 原子提交、`staged → parsing → parsed/parse_failed`、取消/重试/恢复和安全 UI。本 PR 不调用 DeepSeek，也不重新实现 transcript pipeline。

### 7.11 PR #21：Document Evidence Index

分支：`feat/document-evidence-index`

范围：`document_chunks`、页码/章节/表格定位和 document evidence 接入现有 Summary pipeline。文档提供背景和材料，不能在没有 transcript 或人工确认时自动证明会议决定。本 PR 不创建第二套 Summary 或 transcript 体系。

## 8. 后续：Sidecar 生命周期、真实音频与实时 ASR

该阶段风险较高，固定拆为三个顺序 PR：

1. `feat/sidecar-supervisor`：sidecar 打包、随机端口接线、健康恢复、崩溃重启、退出清理、孤儿进程回收和生命周期安全日志。
2. `feat/audio-capture`：WASAPI 系统音频与麦克风、设备枚举、明确开始/停止、持续录音指示、分块文件和异常恢复。
3. `feat/realtime-transcription`：实时 ASR 写入既有 `transcript_sources/transcript_segments`、片段顺序/去重、相对 offset、CPU 回退、取消与会后校正入口。

验收：每个 PR 使用合成音频自动测试，并在 Windows 本机验证真实设备/权限/窗口/进程行为；应用退出或崩溃后不得留下 sidecar 或文件句柄；日志不得包含音频、转录正文、设备 ID 或绝对用户路径。

## 9. 后续：会议窗口、OCR 与屏幕上下文

- `feat/window-context`：用户显式选择目标窗口、窗口身份验证、暂停/撤销和 accessibility tree 提取。
- `feat/screen-ocr`：本地 OCR 回退、敏感内容遮蔽、关键事件/evidence 生成和截图短期缓存删除。

Screenpipe 只能是可替换的可选 Capture Provider，不得成为核心强依赖。采用前必须获得用户批准，并复审商业许可、默认采集范围、本地 API 暴露和 Windows 资源占用。

## 10. 后续：说话人增强与实时翻译

- `feat/speaker-diarization`：匿名实时 diarization、可选短 voice sample、人工命名/纠错，以及隔离的 WhisperX 会后校正 worker。
- `feat/realtime-translation`：流式片段协议、`deepseek-v4-flash` 中英翻译、原文/译文对照、延迟和失败处理。

Diart 与 WhisperX 必须保持独立环境、进程和 lockfile；不得默认做跨会议真实身份识别。平台专用 transcript adapters 只有取得脱敏测试样例后安排；视频导入、音频文件离线转录和跨会议声纹复用继续作为未来增强。

## 11. 产品化：安装包与加固

- `chore/windows-packaging`：Windows 安装包、签名、SBOM、升级/回滚、安装与卸载冒烟测试。
- `feat/product-hardening`：托盘、诊断/隐私设置、长期稳定性、性能、崩溃恢复和删除完整性。

验收必须包含用户本机安装、启动、升级、卸载和残留数据检查；仅有 CI bundle 成功不算完成。

## 12. 测试与验收矩阵

| 层级   | 自动化                              | 本地人工验收                 |
| ------ | ----------------------------------- | ---------------------------- |
| 文档   | Prettier、链接与冲突扫描            | 路线图/分支名审阅            |
| React  | Vitest、Testing Library、typecheck  | Tauri 窗口完整流程           |
| Python | unittest、Ruff、Python 3.12         | worker 启停、模型/资源行为   |
| Rust   | fmt、test/check `--locked`          | 权限、文件、进程、设备行为   |
| 安全   | secret/path/upload/fail-closed 测试 | 凭据库、同意、日志和删除验证 |

所有测试数据均为合成数据。CI 失败时不得声称完成；Windows 桌面能力未做本机验收时，必须明确标记“自动化通过，本地验收待完成”。

## 13. Definition of Done

1. PR 范围与本计划对应阶段一致，且没有夹带下一阶段功能。
2. 正常、失败、取消和安全路径有测试或明确的本机验收记录。
3. 文档、类型、迁移和实现同步，直接依赖精确锁定。
4. staged files 不含凭据、会议数据、录音、截图、上传文件、数据库或日志。
5. diff 已检查，提交清晰，分支已 push，PR 描述包含测试与限制。
6. CI 全绿；涉及 Windows 桌面行为时本机验收完成。
7. PR 保持未自动合并，等待用户决定。

## 14. 当前下一步

PR #1–#14 已合并，Docling Worker Foundation 已完成但未接入产品。当前只交付纯文档分支 `docs/transcript-and-summary-spec`（PR #15），冻结转录、纪要和新路线；不得开始产品代码。PR #15 合并后才能从最新 `main` 创建 `feat/transcript-data-core`，随后严格按 PR #16–#21 顺序推进。原 `feat/document-parsing-orchestration` 顺延到 PR #20，不能作为 transcript-only 纪要的前置任务。
