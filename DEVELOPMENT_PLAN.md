# MeetingLens 开发计划

## 1. 权威状态与 Personal Edition 基线

本文件是 MeetingLens 当前实施顺序、分支名、停止线和验收条件的权威文档。产品行为、安全边界和数据事实分别由 `PRODUCT_SPEC.md`、`SECURITY.md`、`DATA_MODEL.md` 和 `ARCHITECTURE.md` 说明；这些文档不得形成第二条实施路线。

截至 PR #22，`main` 已完成工程与安全基础、安全文件导入、隔离 Docling Worker、zh-CN/en-US 与 Help、粘贴/TXT/SRT/VTT 转录工作区，以及 schema v4 Summary Run Core。当前已经具备：

- `transcript_sources` / `transcript_segments`、有界分页、正文修正和 source-scoped speaker 显示名；
- `summaries` / `summary_sources` / `evidence_items`；
- 不可变 transcript snapshot、series/revision、evidence allowlist；
- queued/running/completed/failed/cancelled、stale、取消、删除协调和 interrupted recovery；
- 由 Rust 执行的 deterministic、零会议内容外发 Mock Summary Provider。

当前尚未实现真实 DeepSeek Provider、SecretStore 中 API Key 的实际消费、用户外发确认、正式纪要结果页、Markdown 复制/保存、录音文件转录、Docling 产品编排、document evidence、实时音频、实时 ASR、实时翻译或独立会议模式工作区。文档规划不得被描述为已经实现的能力。

MeetingLens 现在采用 **Personal Edition / 自用版基线**：Windows-first、local-first、transcript-first，优先让单个用户尽快完成：

```text
粘贴或导入转录
→ 用户确认后由 Rust 调用 DeepSeek
→ 查看并修改纪要
→ 一键复制或保存 Markdown
```

总结模式是当前唯一近期模式。翻译模式与会议模式仍是长期愿景，但属于 Frozen / Deferred，不计入 Personal Edition 的近期完成标准。

## 2. 工作流与 PR 粒度

`main` 始终保持可构建、可演示。Codex 可以自主开发、测试、修复 CI、提交、推送和创建/更新 PR，但不得启用 auto-merge，也不得自动合并；最终 Merge 由用户决定。

每个实现 PR 必须：

1. 只交付一个明确、可独立验收的用户能力，并使用本计划指定的分支名。
2. 从上一个已合并阶段的最新 `main` 创建；不得堆叠未审查的大型 PR。
3. 描述正常、失败、取消、安全和隐私边界，并按本文件的风险等级验收。
4. 同步更新受影响的架构、数据、安全、README 和中英文用户指南。
5. checks 全绿后停在待审状态，不开始下一阶段，直到用户明确决定是否合并。

当数据模型和信任边界已经清晰时，一个完整功能 PR 可以同时包含完成该能力所需的数据层、Rust Commands、TypeScript API 和 UI。不得为了形式对称强制执行：

```text
Data Core PR
→ Command API PR
→ UI PR
```

一个 PR 仍不得夹带无关能力。schema、migration、安全边界、真实网络、文件解析和设备访问等高风险范围，应根据可审查性和可回滚性决定是否进一步拆分，而不是机械地按技术层拆分。

PR #23 是本次重大路线重构所需的最后一个普通独立路线文档 PR。此后不得再为普通“状态同步”单独创建 PR；文档必须随对应功能 PR 更新。只有用户明确要求或发生重大路线重构时，才可再次创建纯文档 PR。

## 3. 已完成阶段与历史事实

| 历史范围  | 主要交付                                                                                                     | 状态                                 |
| --------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------ |
| PR #1–#10 | Phase 0、meeting CRUD/API/UI、Provider Mock、本地 Sidecar 会话认证、Windows Credential Manager `SecretStore` | 已完成并合并                         |
| PR #11    | 不可信文档导入、私有存储和可恢复删除规格                                                                     | 已完成并合并                         |
| PR #12    | schema v2、DocumentService、安全存储和删除协调                                                               | 已完成并合并                         |
| PR #13    | Rust 原生文档选择器、Commands/API 和导入/删除 UI                                                             | 已完成并合并                         |
| PR #14    | 隔离、离线、受资源限制的 Docling Worker Foundation                                                           | 已完成并合并                         |
| PR #15    | transcript-first 与可追溯 Summary 规格                                                                       | 已完成并合并                         |
| PR #16    | zh-CN/en-US、locale 偏好、Help 和双语开发预览手册                                                            | 已完成并合并                         |
| PR #17    | 三模式共享管线与当时路线的文档对齐                                                                           | 已完成并合并；现由 PR #23 重新定基线 |
| PR #18    | schema v3、粘贴/TXT/SRT/VTT parser、转录来源/片段和修正/删除语义                                             | 已完成并合并                         |
| PR #19    | 安全 Transcript Commands/API、Rust 原生选择器和原子 meeting + transcript 创建                                | 已完成并合并                         |
| PR #20    | Summary 输入 UI、分页预览、正文和 speaker 显示名修正                                                         | 已完成并合并                         |
| PR #21    | schema v4 Summary Run Core、不可变快照、series/revision、evidence、Mock、取消和恢复                          | 已完成并合并                         |
| PR #22    | Summary Run Core 合并后状态与路线文档同步                                                                    | 已完成并合并                         |

历史阶段曾按 Data/API/UI 或多个安全基础 PR 拆分，这是已完成工作的审查记录，不再构成所有新功能必须采用相同拆法的通用规则。

PR #21 的 Windows 验收使用 development-only 系统 Temp 直属隔离 app-data 根，并让两次 Tauri 启动使用与根 UUID 匹配的合成 identifier，从而隔离 SQLite 和 WebView2 profile。两次启动覆盖 run/result/revision/stale/evidence、running recovery、SQLite integrity、零 MeetingLens 应用层外发、无正文浏览器持久化及最终清理。该记录必须保留为 Summary Run Core 的历史验收证据，但双隔离 app-data 和完整双启动不是普通 UI PR 的默认门槛。

当前 SQLite schema 版本仍是 v4。PR #23 不修改代码、schema、migration、依赖、lockfile、权限或 CSP。

## 4. 当前：Personal Edition Re-baseline

分支：`docs/personal-edition-descope`

实际 PR：PR #23。

范围：

- 定义 Personal Edition 当前目标；
- 将实时翻译、实时采集、独立会议模式、Structured Visuals 和商业分发能力标为 Frozen / Deferred；
- 取消独立 Consent Data Core；
- 简化编辑、Docling 编排、PR 粒度和本机验收；
- 统一后续最短路线。

本 PR 只修改权威文档。不得修改 Rust、TypeScript/React、Python、SQL、migration、依赖、lockfile、Tauri Commands、权限或 CSP；不得添加 DeepSeek、Consent 表、faster-whisper、音频依赖或 Docling 产品接线；不得创建下一阶段分支。

## 5. 下一阶段：DeepSeek Summary Provider

分支：`feat/summary-provider-deepseek`

预期顺序：PR #24；PR 编号只是预期，分支名与能力边界才是权威标识。

这是 PR #23 合并后唯一明确的下一阶段。范围可以在一个完整功能 PR 内包含：

- Rust `SummaryProvider` 的受限 DeepSeek adapter；
- 只由受信任 Rust 代码从 Windows Credential Manager `SecretStore` 读取 API Key；
- 真实 DeepSeek HTTPS 请求、超时、取消和安全错误；
- 严格、版本化的响应解析与大小/数量限制；
- 继续由 Rust 验证 run-local evidence allowlist，并原子提交结果；
- 第一次准备外发时显示明确确认，说明 Provider 为 DeepSeek，且只发送用户本次选择的规范化转录内容；
- “本次同意”“记住我的选择”“取消”；
- 本地保留实现所需的最小时间戳或状态；未确认、没有 Key 或发送范围为空时 fail closed；
- 与实现同步更新架构、安全、README、开发计划和中英文用户指南。

Personal Edition 当前不建立通用 `consent_events` 审计体系，不创建 Consent 查询页、多用户 Consent、复杂会话失效矩阵或企业合规导出。最小确认状态的具体持久化设计由本 PR 根据实际实现决定；不得修改既有 migration，也不得破坏 schema v4 的不可变快照、series/revision、状态机或 Rust Provider 权威边界。

这套最小 Consent 只适用于单人自用版，不是面向团队、客户或公众分发的企业合规方案。真实 Provider 会产生外部数据处理和潜在费用；开始实现前仍须用户明确批准 Provider、模型、价格/条款和发送范围。测试只使用 Mock 与合成数据，不得读取聊天中出现过的 key，不得从 `.env`、环境变量、Python 或命令行消费真实 DeepSeek 凭据。

本阶段不实现完整纪要结果页面、Markdown 文件保存、持久正文编辑、Docling、录音文件转录、实时功能或独立 Consent schema。

里程碑：

```text
真实 AI Provider 接通，但正式日常纪要工作区尚未完成
```

## 6. 随后：Summary Mode Minutes

分支：`feat/summary-mode-minutes`

预期顺序：PR #25。

范围：

- 来源、模板和中文/英文/双语输出选择；
- 正式生成入口、运行状态和安全错误；
- 纪要结果页、Markdown 展示和有效 evidence 导航；
- 一键复制和 Markdown 文件保存；
- 修改纪要正文并保存当前结果；
- 整份重新生成；新结果继续使用既有 series/revision，不覆盖旧 revision；
- 对已实现 stale 字段提供满足当前用户流程的最小说明；
- 同步更新所有受影响文档和中英文用户说明。

本阶段不开发章节/段落级 regenerate、自动局部更新、复杂 stale 提示、编辑历史时间线、多人协作、冲突合并、DOCX/PDF 导出、Office 兼容性矩阵、Docling 或音频转录。

模型不得发明参会人、负责人、截止日期或会议共识；关键决定、行动项和争议必须引用当前 run 的有效 evidence。重新生成不得静默覆盖旧 revision 或用户已保存的结果。

日常可用里程碑：

```text
★ 粘贴或导入转录
→ 生成真实纪要
→ 查看和修改
→ 复制或保存 Markdown
```

## 7. 然后：Audio-file Transcription

分支：`feat/audio-file-transcription`

预期顺序：PR #26。

范围：

- 用户通过 Rust 原生选择器主动导入会议录音文件；
- 本地 faster-whisper、CPU int8；
- 每个受控请求使用一次性 Worker 进程，复用 Docling Worker 已验证的进程隔离、清理和协议原则；
- Rust 控制路径、输入校验、资源上限、取消、进程树回收和安全错误；
- 转写结果写入既有 `transcript_sources` / `transcript_segments`，进入现有 Transcript Workspace 和 Summary pipeline；
- 如需 schema 变化，基于合并时最新版本追加 migration 并完成高风险验收；不得复用或修改 v1–v4。

本阶段不做实时音频、WASAPI、麦克风/系统音频采集、实时字幕、实时 ASR、实时 diarization、说话人身份推断、实时翻译或第二套 transcript 模型。

里程碑：

```text
覆盖使用其他软件录制、会后批量转写并生成纪要的场景
```

## 8. 再后：简化 Docling 编排

分支：`feat/document-parsing-orchestration`

预期顺序：PR #27。

该阶段在一个明确用户能力内完成必要的数据、Rust 编排、Commands/API 和 UI：

- 将已完成的隔离 Docling Worker 接入正式文档生命周期；
- 只允许 Rust 注册、校验且位于受控私有根的文档进入 Worker；
- 原子提交经过 Rust 二次验证的解析产物；
- 生成 document chunks 和 document evidence，并接入既有 Summary pipeline；
- 解析失败自动重试一次；再次失败后显示稳定安全错误并提示用户手动重新导入；
- 维持 transcript 与 document 的不同语义和 evidence 边界；
- 同步实现必要的删除协调和文档更新。

本阶段不建立通用 processing job 调度器、多级自动恢复、长期 checkpoint、复杂取消矩阵、崩溃后自动续跑、多次指数退避或企业级监控面板。不得向 React 暴露路径，不得静默外发文件，不得让文档在缺少 transcript 或用户确认时自动证明会议决定。

PDF、DOCX、PPTX 等继续是参考文档输入格式；冻结的是 DOCX/PDF 纪要导出，不得混淆输入解析与输出格式。

里程碑：

```text
会议材料可以作为受控背景证据进入既有纪要管线
```

## 9. 可选：Window Context

分支：`feat/window-context`

预期顺序：PR #28 或更后。只有用户确认实际经常需要屏幕分享上下文时才启动。

要求：

- 用户明确选择一个具体窗口；
- 持续显示当前目标，窗口身份变化、关闭或被替换时立即暂停并重新确认；
- 优先使用可控的 accessibility text；
- 默认不保存屏幕关键帧，不监控全部桌面；
- screen evidence 与 transcript/document evidence 保持不同语义；
- 不顺带实现 screen OCR、实时录音、翻译或会议模式工作区。

## 10. Frozen / Deferred

以下能力当前不开发、不计入 Personal Edition 日常可用门槛，但不删除已完成基础，也不表示永久取消：

### 实时采集与实时语言栈

- `feat/live-session-data-core`；
- 完整 `feat/sidecar-supervisor` 路线；
- `feat/audio-capture` 与 WASAPI 麦克风/系统音频采集；
- `feat/realtime-transcription`；
- `feat/live-language-data-core`；
- `feat/speaker-diarization`；
- `feat/realtime-translation-provider`；
- `feat/checkpoint-summary-runtime`；
- `feat/topic-checkpoints`；
- `feat/translation-overlay`；
- `feat/translation-mode-workspace`。

已完成的认证 Sidecar、Provider 契约和进程安全基础继续保留；冻结的是把它们扩展成完整实时产品栈。

### 独立会议模式与屏幕增强

- `feat/meeting-notes-core`；
- `feat/meeting-mode-workspace`；
- `feat/screen-ocr`；
- 自动屏幕变化判断、持续全桌面监控和屏幕关键帧长期保存。

未来如恢复会议模式，必须优先复用录音文件/转录输入、现有 Transcript Workspace 与现有 Summary pipeline，不得建立第二套会议数据库、转录模型或 Summary pipeline。

### 复杂编辑、视觉与导出

- 独立 `feat/summary-editing-export`；
- 章节/段落级重生成、自动局部更新、复杂编辑时间线与协作冲突合并；
- 独立 `feat/document-evidence-index`；document evidence 已并入简化 Docling 编排；
- `feat/structured-visuals`、自动图表、自动流程图和模型生成可执行可视化；
- DOCX/PDF 纪要导出、复杂版式模板和 Office 兼容性矩阵。

### 商业分发

- 安装包完善、代码签名、SBOM、自动更新和托盘；
- 商业发布、企业部署、企业审计与多人/账号体系。

已有构建能力、依赖许可证记录和安全设计不得删除。未来若恢复分发，仍须补齐签名、许可证/SBOM、升级回滚、安装卸载和残留数据验收。

### 已取消的独立阶段

`feat/consent-data-core` 不再作为独立阶段，也不属于 Frozen 功能分支。最小外发确认并入 `feat/summary-provider-deepseek`；不得重新创建通用 Consent schema PR，除非未来产品从 Personal Edition 扩展到需要重新设计合规模型，并先获得用户批准和更新权威文档。

冻结阶段不得自行创建。只有用户明确解冻并先更新本文件的范围、数据边界和验收条件后，才可进入实现。

## 11. 风险分级测试与验收

所有测试数据必须是合成数据。所有 PR 都必须运行 `git diff --check`、人工检查完整 diff、执行敏感信息门禁并等待 CI 全绿；未运行的检查必须明确报告。

### 11.1 文档 PR

最低要求：

- Markdown/格式检查；
- 文档链接检查；
- 全仓路线、分支名和状态一致性检索；
- `git diff --check`；
- 确认没有代码、migration、依赖、lockfile、生成文件或禁止文件；
- CI。

PR #23 是迁移到新流程的文档 PR，自身仍须遵守任务开始时已有质量门；如果仓库现有规则要求 `npm run check`，本 PR 必须运行，不能用新写入的分级规则反过来跳过本次验收。

### 11.2 不修改 schema 的普通功能 PR

最低要求：

- 所有相关 formatter、lint、typecheck、自动测试和构建；
- 在 Windows 11 本机至少启动一次 `npm run tauri dev`；
- 跑通主要用户流程以及关键失败/取消路径；
- 检查 WebView 持久化、日志、错误和测试输出没有正文、密钥或私人路径；
- 确认没有孤儿进程、临时文件或工作区残留。

### 11.3 修改 schema、migration、安全边界或真实网络的高风险 PR

除普通功能要求外，还必须按实际风险覆盖：

- 从受支持旧版本升级到最新 migration；
- schema version 与 migration checksum；
- `PRAGMA integrity_check` 返回 `ok`；
- `PRAGMA foreign_key_check` 无记录；
- migration 中途失败、事务回滚和再次启动；
- 必要的隔离 app-data、双启动或恢复验证；
- 缺少 Key、缺少同意、空发送范围、超时、取消和错误响应均 fail closed；
- MeetingLens 应用代码与 Mock 的网络边界、真实 Provider 的唯一允许目标，以及错误/日志不含 key、请求正文或外部响应正文；
- 合成数据、文件、数据库、WebView profile、Worker、进程、句柄和端口清理。

双隔离 app-data 和完整双启动只在 migration、恢复、持久状态或安全边界确实需要时执行，不是所有普通 UI PR 的默认要求。CI 不能替代 Windows Credential Manager、原生文件选择器、Worker 进程、真实网络取消或窗口身份等本机行为验收。

当前标准检查命令包括：

- `npm run format:check`
- `npm run lint`
- `npm run typecheck`
- `npm run test`
- `npm run test:sidecar`
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --locked --manifest-path src-tauri/Cargo.toml`
- `cargo check --locked --manifest-path src-tauri/Cargo.toml`

根据改动和最高风险选择相关检查；不得绕过适用的聚合门禁。

## 12. 永久工程、安全与隐私约束

### Git 与 Merge

- 不直接在 `main` 开发、提交或 push；不 force push，不绕过分支保护。
- auto-merge 默认关闭；只有用户针对明确 PR 发出当次合并指令后才可合并。
- 合并一个 PR 不授权开始、合并或清理其他阶段。

### Migration 与数据

- migration 只能追加、在事务中执行并记录 checksum；不得修改、替换或复用已合并的 v1–v4 或任何后续 migration。
- 数据层变化必须有自动测试；破坏性 migration、数据删除或不可逆转换必须先备份并获得用户批准。
- schema v4 的 Summary series/revision、不可变 input snapshot、stale 字段、evidence 和 completed/failed/cancelled 状态机必须保留。

### 凭据、网络与 Provider

- DeepSeek API Key 只能保存在 Windows Credential Manager，并由 Rust `SecretStore` 读取；不得进入 React、SQLite、日志、Python、环境变量或命令行。
- React/WebView 不直接请求 DeepSeek，不提交 Provider 结果或 evidence，也不获得通用 HTTP、shell、process 或文件系统权限。
- Rust 是持久 Summary run、快照、状态转换、Provider 调用、响应 schema、evidence allowlist 和原子提交的执行权威。
- 未确认外发必须 fail closed。当前 Mock 和 MeetingLens 应用代码不外发会议内容；这不等于声称 WebView2 Runtime 在物理层面完全断网。

### 路径、文件与进程

- 所有导入文件视为不可信数据。Rust 负责原生选择、类型/配额、流式复制、哈希、受控 UUID 路径和删除协调。
- 源绝对路径不得进入 React、SQLite 或日志；原始文件名只作显示元数据，不参与路径拼接。
- 所有文件访问必须确认最终路径仍位于批准私有根，并防止 traversal、UNC、ADS、reparse point 和 symlink 逃逸。
- Worker 使用清理后的环境、受控输入/输出根、资源限制和完整进程树回收；不得静默外发文件。

### 错误、日志与 Git 内容

- Commands/API 使用稳定错误码和安全消息；未知错误 fail closed，不返回 SQL、原生异常、绝对路径、key 或正文。
- 普通日志只记录 allowlist 中的事件类型、安全 ID、状态码和耗时；转录正文、prompt、speaker、文件名、凭据和 Provider 请求/响应正文不得进入日志。
- 用户文件、会议媒体、转录、截图、数据库、WAL/SHM、日志、崩溃转储、临时文件和构建产物不得进入 Git。
- 提交前必须展示 staged 文件列表、检查完整 staged diff，并执行不回显疑似秘密原文的 secret/private-path/禁止扩展名扫描。

## 13. Definition of Done

1. PR 只交付本计划中的一个明确能力，没有夹带下一阶段或 Frozen 能力。
2. Data/API/UI 如在同一 PR 中交付，必须共同服务同一用户流程，并保持 Rust 与 WebView 信任边界清晰。
3. 已按最高风险等级完成自动化和 Windows 本机验收；未运行项和限制明确报告。
4. 文档、中英文用户说明、类型、schema 和实现同步；直接依赖精确锁定。
5. staged 内容不含凭据、会议数据、录音、截图、上传文件、数据库、日志或私人路径。
6. diff 已检查，提交清晰，分支已 push，PR 描述包含范围、测试、安全/隐私影响和限制。
7. CI 全绿，auto-merge 关闭，PR 保持未合并等待用户决定。

## 14. 当前下一步

当前任务是完成并审查 `docs/personal-edition-descope` / PR #23，只修改权威文档并停在待审状态。

PR #23 合并后，唯一明确下一阶段为：

```text
feat/summary-provider-deepseek
```

在用户明确批准并开始该阶段前，不得创建该分支、接入 DeepSeek 或实现最小 Consent。不得创建 `feat/consent-data-core`。Frozen / Deferred 阶段不得提前启动。
