# MeetingLens Agent Guide

本文件适用于整个仓库。执行任务时，系统/开发者指令和用户的最新明确要求优先于本文件。

## Git 工作流

1. 开始任务前必须运行并检查：
   - `git status`
   - `git branch --show-current`
   - `git pull --rebase`
2. 发现未知或属于用户的未提交改动时，必须保留并绕开；不得擅自覆盖、清理或暂存。若无法安全绕开，停止并请求用户确认。
3. 除非用户明确允许，否则不得直接在 `main` 上开发、提交或推送，也不得自动合并到 `main`。
4. 每个可验收任务使用独立分支，名称必须简短且可读：
   - `feat/<feature-name>`
   - `fix/<bug-name>`
   - `docs/<document-name>`
   - `chore/<task-name>`
5. 同步远端时使用普通 pull/rebase；不得使用 `--force`、`--force-with-lease`，不得绕过分支保护。
6. 提交信息使用 Conventional Commits，例如 `feat: add audio file transcription`、`docs: update security model`。
7. 分支名称和任务范围必须与 `DEVELOPMENT_PLAN.md` 一致。PR #23 合并后，普通状态同步、架构说明、README、安全说明和用户指南必须随对应功能 PR 更新；只有用户明确要求或发生重大路线重构时，才单独创建纯文档 PR。
8. 一个 PR 只交付一个明确、可独立验收的用户能力。当数据模型和边界已经清晰时，同一功能 PR 可以包含完成该能力所必需的数据层、Rust Commands、TypeScript API 和 UI；不得为了形式对称强制拆成 Data Core、Command API 和 UI 三个 PR。
9. 不得把无关功能或多个可独立审查的高风险信任边界混入同一 PR。schema、migration、安全边界、真实网络、文件解析或设备访问范围扩大时，应按可审查性、可回滚性和风险决定是否进一步拆分。

## 自动化与合并边界

- Codex 可以自主开发、测试、修复 CI、提交、推送以及创建或更新 Pull Request。
- Codex 必须在 PR checks 全绿后停在待审状态；是否合并由用户决定。
- 不得启用 auto-merge，不得把“用户最终决定”解释为预先授权自动合并。
- 只有用户针对某个明确 PR 发出当次合并指令后，才能执行该次合并；不得顺带合并其他 PR 或开始下一阶段。

## 风险分级质量门槛

所有 PR 均须运行 `git diff --check`、人工检查完整 diff、使用合成数据、完成敏感信息门禁并等待 CI。不得把“未运行”写成“通过”，不得提交明知失败的结果。具体门槛按本次改动的最高风险确定：

### 文档 PR

- 运行 Markdown/格式检查、文档链接检查和路线一致性检索。
- 确认只修改预期文档，没有源代码、migration、依赖、lockfile 或生成文件。
- PR #23 是重新定基线前最后一个普通独立路线文档 PR；它自身仍须执行任务开始时已有规则要求的全项目检查。

### 不修改 schema 的普通功能 PR

- 运行所有相关 formatter、lint、类型检查、自动测试和构建。
- 涉及 Windows 桌面行为时，至少运行一次 `npm run tauri dev`，在用户本机走通主要用户流程。
- 检查正文、密钥、私人路径和禁止文件没有进入 WebView 持久化、日志、测试输出或 Git；确认工作区干净。

### 修改 schema、migration、安全边界或真实网络的高风险 PR

- 运行相关完整自动化检查和 Windows 本机主流程。
- migration 必须验证受支持旧版本升级、最终 schema version/checksum、`PRAGMA integrity_check` 和 `PRAGMA foreign_key_check`。
- 按风险执行隔离 app-data、必要的双启动、故障恢复、取消、超时和清理验证；双隔离和完整双启动不是普通 UI PR 的默认要求。
- 安全或真实网络改动必须验证缺少凭据、缺少同意和空发送范围均 fail closed，并检查网络范围、错误脱敏、日志和取消链路。

当前项目的标准检查命令包括：

- `npm run format:check`
- `npm run lint`
- `npm run typecheck`
- `npm run test`
- `npm run test:sidecar`
- `npm run build`
- Rust 可用时：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`cargo test --locked --manifest-path src-tauri/Cargo.toml` 和 `cargo check --locked --manifest-path src-tauri/Cargo.toml`

只运行与风险等级和改动范围相符的检查；若现有聚合脚本是该范围的门禁，则不得绕过。直接依赖必须锁定精确版本并提交相应 lockfile，不得为解决兼容性问题放宽版本范围或关闭安全检查。Windows 11 和 Python 3.12 是当前项目基线，其他平台或版本只能作为额外验证。

## 不得弱化的工程与安全规则

- migration 只能追加，必须在事务中执行并记录 checksum；不得修改、替换或复用已合并 migration 版本，不得静默删除未知数据。
- 真实 DeepSeek API Key 只能由受信任 Rust 代码从 Windows Credential Manager `SecretStore` 读取；不得进入 React、SQLite、日志、Python、环境变量或命令行。自动测试只使用 Mock/fake transport 和明显的合成占位值。
- React/WebView 不直接调用外部 Provider，不提交 Provider 结果或 evidence；Rust 是持久 Summary run、不可变快照、状态转换、Provider 调用、响应校验和 evidence allowlist 的执行权威。
- 保留路径守卫、canonicalization、reparse point/symlink 防护、受控 UUID 路径、稳定错误码、错误脱敏和正文不进入普通日志的规则。
- Rust Mock 不外发会议内容；production 只有受信任 Rust DeepSeek Provider 可把已明确确认的冻结范围发送到固定目标。不得把该应用层边界描述为 WebView2 Runtime 在物理层面完全断网。

## 敏感信息与私人数据

每次提交前必须先展示 staged 文件列表，再检查 `git diff --cached` 中是否存在密钥或私人数据。发现后立即停止，不得提交，并告知用户。

最低检查必须同时包含：`git diff --cached --name-status`、完整 staged diff、常见 secret/token/private-key 模式扫描，以及被跟踪文件中的会议媒体/数据库/日志扩展名检查。扫描结果不得回显疑似密钥原文。

永远不得提交：

- API keys、access tokens、密码、私钥或 DeepSeek 凭据
- `.env` 或其他真实凭据文件（无真实值的 `.env.example` 可以提交）
- 会议录音、voice samples、转录原文或参会者私人信息
- 截图、屏幕捕获、用户上传文档或会议附件
- 本地 SQLite 数据库、数据库 WAL/SHM 文件
- 日志、崩溃转储、临时文件或构建产物

示例、测试和文档只使用明显的占位值与合成数据。不得在终端输出、测试快照、日志、错误信息或 PR 描述中回显密钥或会议正文。对会议数据执行最小采集、明确同意、可删除和本地优先原则。

## Push、Pull Request 与 CI

1. 提交前展示 staged 文件列表和 diff 摘要。
2. 创建清晰、单一目的的 commit，并将当前分支 push 到 `origin`。
3. 创建或更新 Pull Request；PR 必须描述范围、测试结果、安全/隐私影响和已知限制。
4. 功能 PR 必须同步更新受影响的架构、开发计划、安全说明、README 和中英文用户指南；中英文能力陈述必须一致。
5. CI 失败时检查日志、修复、重新测试并再次 push。
6. 所有 required checks 通过前不得把工作报告为完成。
7. 不得启用 auto-merge，不得自动合并到 `main`；合并决定由用户作出。

## Personal Edition 产品基线与近期顺序

- MeetingLens 当前采用 Personal Edition / 自用版基线：Windows-first、local-first、transcript-first，优先让单个用户日常完成“粘贴或导入转录 → DeepSeek 生成纪要 → 查看和修改 → Markdown 复制或保存”。
- PR #1–#24 已完成工程/安全基础、Personal Edition 重定基线、粘贴与 TXT/SRT/VTT 转录工作区、schema v4 Summary Run Core，以及 schema v5 Rust-only DeepSeek Provider、SecretStore 消费、严格 JSON/evidence、取消/超时/稳定错误和最小外发 Consent。一次性确认仅在内存中保留 10 分钟并单次消费；remembered Consent 只使用 `provider_consent_preferences` 当前偏好表，不恢复独立 `feat/consent-data-core` 或通用 `consent_events`。
- 当前 `feat/summary-mode-minutes` 功能分支把正式生成、结果、编辑、evidence、Markdown 复制/原生 Save As 和整份重新生成接成日常可用闭环；它保持 schema v5，不修改 v1–v5 migration、固定 DeepSeek 协议或现有五区 `overview/decisions/actionItems/risks/openQuestions` 内容契约。该功能完成并经用户决定合并后，唯一下一阶段才是 `feat/audio-file-transcription`，随后为 `feat/document-parsing-orchestration` → 可选 `feat/window-context`。
- production Summary run 固定使用 DeepSeek；确定性 Mock 只允许测试/debug 隔离，历史 Mock run 只读但仍可查看、复制和保存。没有 Key、没有有效确认、空范围、响应不合法或 evidence 越界时必须 fail closed；queued run 重启后不得自动发送，真实付费 API smoke 未执行时不得声称已完成真实账户联网验收。
- 纪要编辑只允许修改现有五区正文和行动项 owner/due date，Rust 保留 evidence、状态、顺序及其他 Provider 元数据；保存使用派生 `contentVersion` 做并发保护，在原 revision 内更新 `content_json` 并置 `is_user_edited=1`。同一 revision 不另存 Provider 原稿或编辑历史；整份重新生成只允许 latest terminal DeepSeek revision 创建下一 revision，并保留旧 revision。章节/段落级重生成、复杂 stale UI、完整编辑时间线、冲突合并、DOCX/PDF 导出均冻结。
- Markdown 必须由 Rust 的同一确定性 renderer 生成并供预览、显式剪贴板写入和 Rust 原生 Save As 使用；React 不接收或提交文件路径、不读取剪贴板、不把结果渲染为不可信 HTML。用户保存到外部位置的 `.md` 和剪贴板副本不受 MeetingLens 删除协调管理。
- `feat/audio-file-transcription` 只做用户主动导入录音文件、本地 faster-whisper CPU int8 一次性受控 Worker、写入既有 transcript 模型并回到现有工作区；不做 WASAPI、实时音频、实时字幕或实时 diarization。
- Docling Worker Foundation 已完成并保留。简化版 `feat/document-parsing-orchestration` 可以在一个功能 PR 内接入解析、document chunks/evidence 和 Summary pipeline；失败自动重试一次，再失败提示手动重导，不建立通用重型调度平台。
- `feat/window-context` 只有在用户确认实际经常需要时才启动；必须由用户选择具体窗口，身份变化时暂停/重新确认，默认不保存屏幕关键帧，不监控全部桌面。

以下能力是 Frozen / Deferred：实时会议录音、实时 ASR、完整 Sidecar supervisor 路线、实时说话人分离、实时翻译及外发、checkpoint、自动话题判断、翻译浮窗、翻译模式工作区、独立会议模式工作区、screen OCR、structured visuals、安装包完善、签名、SBOM、自动更新、托盘、商业发布和企业部署。冻结不等于删除；不得删除已完成基础，也不得在用户解冻和文档更新前创建这些功能分支。

转录是纪要的第一类来源；参考文档是可选增强。`transcripts` 与 `documents` 是不同语义和信任边界，粘贴/TXT/SRT/VTT 不得强制经过 DocumentService 或 Docling。PDF/DOCX 可以继续作为未来 Docling 输入格式，但不是近期导出目标。三种模式的长期愿景可以保留，但不得建立三套会议、转录、摘要或删除模型。

## 文档导入与删除边界

- 用户导入文件一律视为不可信数据；“保存在本地”不代表文件可信或可以执行。
- React/WebView 不得获得任意文件系统读取权限，不得接收源文件绝对路径，也不得直接渲染未经处理的用户文件。
- 文件选择、类型/配额校验、私有目录路径构造、流式复制、哈希和删除协调必须由 Rust 信任边界执行；Python Worker、Sidecar 和 DeepSeek 不得绕过该边界取得原始文件。
- 原始文件名只可作为显示元数据，不能参与目录拼接；应用内部路径必须使用受控 meeting/document UUID，并验证最终路径仍位于应用私有根目录且不是不受信任的 reparse point/symlink 逃逸。
- 已完成的可恢复、幂等会议/文档删除协调必须继续覆盖新增 artifact 和派生数据；不得恢复“先删数据库再尽力删文件”的直接硬删除。

## 必须先获得批准的操作

执行以下操作前必须停止并请求用户批准：

- 删除大量或重要项目数据
- 将仓库设为公开或改变仓库所有权
- 修改、削弱或绕过分支保护和安全检查
- 引入会产生费用的外部服务或扩大数据发送范围
- 采用需要商业许可的采集组件，或接受新的模型/数据处理条款
- 执行不可逆迁移、破坏性数据库操作或凭据轮换

## 完成报告

任务结束时必须报告：

- 分支名称
- commit hash
- Pull Request 链接和状态
- 实际运行的测试、lint、类型检查和构建结果
- CI 结果
- push 结果
- merge 状态（默认应为未合并）
- 任何未运行的检查、已知限制或需要用户决定的事项
