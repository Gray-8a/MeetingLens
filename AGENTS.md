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
6. 提交信息使用 Conventional Commits，例如 `feat: add audio capture`、`docs: define security model`。
7. 分支名称和任务范围必须与 `DEVELOPMENT_PLAN.md` 一致；若计划未定义或文档互相冲突，先创建纯文档 PR 对齐，不得自行混用旧阶段名称。
8. 一个较大阶段必须拆成可独立验收的小型 PR；禁止把数据库、UI、Provider、文档解析、模型调用或音频采集塞入同一个实现 PR。

## 自动化与合并边界

- Codex 可以自主开发、测试、修复 CI、提交、推送以及创建或更新 Pull Request。
- Codex 必须在 PR checks 全绿后停在待审状态；是否合并由用户决定。
- 不得启用 auto-merge，不得把“用户最终决定”解释为预先授权自动合并。
- 只有用户针对某个明确 PR 发出当次合并指令后，才能执行该次合并；不得顺带合并其他 PR 或开始下一阶段。

## 质量门槛

完成一个可验收任务后，在提交前：

1. 运行与改动相关的全部测试、lint、类型检查和构建。
2. 运行 `git diff --check`，并人工检查 `git diff`。
3. 若工具链缺失或某项检查无法运行，必须明确说明原因；不得把“未运行”写成“通过”。
4. 测试失败时不得声称任务完成，不得提交明知失败的结果。
5. 当前项目的标准检查命令为：
   - `npm run format:check`
   - `npm run lint`
   - `npm run typecheck`
   - `npm run test`
   - `npm run test:sidecar`
   - `npm run build`
   - Rust 可用时：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`cargo test --locked --manifest-path src-tauri/Cargo.toml` 和 `cargo check --locked --manifest-path src-tauri/Cargo.toml`
6. 直接依赖必须锁定精确版本并提交相应 lockfile；不得为解决兼容性问题擅自放宽版本范围或关闭安全检查。
7. Windows 11 和 Python 3.12 是当前项目基线。其他平台/版本只能作为明确标注的额外验证，不得替代该基线。
8. 涉及 Windows 桌面能力的 PR 必须在用户本机实际运行对应验收流程（至少包括 `npm run tauri dev`，并按功能验证窗口/权限/设备/进程行为）。CI 通过只能证明自动化构建与测试通过，不能替代本地桌面验收。

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

规则：

- 开发密钥只放在已忽略的 `.env`；发布版本使用操作系统凭据存储。
- 不得在终端输出、测试快照、日志、错误信息或 PR 描述中回显密钥。
- 示例、测试和文档只使用明显的占位值与合成数据。
- 对会议数据执行最小采集、明确同意、可删除和本地优先原则。

## Push、Pull Request 与 CI

1. 提交前展示 staged 文件列表和 diff 摘要。
2. 创建清晰、单一目的的 commit，并将当前分支 push 到 `origin`。
3. 创建或更新 Pull Request；PR 必须描述范围、测试结果、安全/隐私影响和已知限制。
4. CI 失败时检查日志、修复、重新测试并再次 push。
5. 所有 required checks 通过前不得把工作报告为完成。
6. 不得启用 auto-merge，不得自动合并到 `main`；合并决定由用户作出。

## 产品价值与近期顺序

- 当前产品采用 `SIMPLIFIED_MODE_SPEC.md` 定义的“一个共享引擎、三个简约入口”：总结模式、翻译模式、会议模式。旧版未开始的路线冻结但不删除已完成基础。
- MeetingLens 的第一核心价值仍是让用户以最少步骤把已有会议转录或粘贴文本转化为详实、可编辑、可追溯的会议纪要；总结模式必须先形成真实闭环，翻译和会议模式复用同一数据管线逐步开放。
- 转录是纪要的第一类来源；参考文档是可选增强。`transcripts` 与 `documents` 是不同语义和信任边界，转录导入不得强制经过 DocumentService 或 Docling。
- Docling Worker Foundation 已完成并保留；数据库/UI 编排在 transcript-only 总结闭环和编辑导出完成后由 `feat/document-parsing-orchestration` 接入，并先于真实音频阶段。不得把 Docling 恢复为第一份 transcript-only 纪要的前置依赖。
- 国际化、Help 页面和中英文开发预览手册已通过 `feat/i18n-user-guide-foundation` / PR #16 完成并合并，必须继续保留并随产品能力同步维护。
- 当前固定顺序为：`docs/simplified-three-mode-spec` → `feat/transcript-data-core` → `feat/summary-mode-import` → `feat/summary-run-core` → `feat/consent-data-core` → `feat/summary-provider-deepseek` → `feat/summary-mode-minutes` → `feat/summary-editing-export` → `feat/document-parsing-orchestration` → `feat/document-evidence-index` → `feat/live-session-data-core` → `feat/sidecar-supervisor` → `feat/audio-capture` → `feat/realtime-transcription` → `feat/window-context` → `feat/screen-ocr` → `feat/live-language-data-core` → `feat/speaker-diarization` → `feat/realtime-translation-provider` → `feat/checkpoint-summary-runtime` → `feat/topic-checkpoints` → `feat/translation-overlay` → `feat/translation-mode-workspace` → `feat/meeting-notes-core` → `feat/meeting-mode-workspace`。有依据的固定图表继续作为独立后续增强。分支名、停止线和更后续顺序以 `DEVELOPMENT_PLAN.md` 为准。
- 不得恢复 `feat/transcript-import-ui`、`feat/deepseek-summary-mvp` 等已被新拆分取代的旧路线名称，也不得从未合并的旧路线分支开始新功能。
- 第一版转录只承诺粘贴 UTF-8 文本以及 TXT/SRT/VTT。平台专用 adapter 必须取得脱敏真实样例后单独增加；视频/音频文件转录不属于近期范围。
- 纪要资格取决于是否存在可冻结的用户确认输入，不得以 `meeting.status == ready` 作为唯一门槛。纪要运行使用不可变输入快照，重新生成创建 revision，不得静默覆盖用户编辑。
- 三种模式只是界面和能力预设，不得建立三套会议、转录、摘要或删除模型。原始音频、逐字稿和会议纪要必须作为不同数据类型、同意范围和保留选项处理。
- 应用启动时不得自动申请麦克风、系统音频或屏幕权限。每项采集、保存原始音频、持续翻译外发和纪要外发必须分别获得本次会话同意并持续显示状态。

## 文档导入与删除边界

- 用户导入文件一律视为不可信数据；“保存在本地”不代表文件可信或可以执行。
- React/WebView 不得获得任意文件系统读取权限，不得接收源文件绝对路径，也不得直接渲染未经处理的用户文件。
- 文件选择、类型/配额校验、私有目录路径构造、流式复制、哈希和删除协调必须由 Rust 信任边界执行；Python Sidecar 和 DeepSeek 不得绕过该边界取得原始文件。
- 原始文件名只可作为显示元数据，不能参与目录拼接；应用内部路径必须使用受控 meeting/document UUID 并验证仍位于应用私有根目录。
- 首次引入会议文件的实现 PR 必须同步实现可恢复、幂等的会议和文档删除协调；不得继续沿用无附件阶段的直接硬删除。
- 文档功能必须按 `DEVELOPMENT_PLAN.md` 拆分为已完成的存储核心、导入 UI、隔离 Docling Worker，以及后续独立的解析编排和证据索引；转录数据、转录 UI、摘要 Provider 与文档增强不得在一个 PR 中合并多个信任边界。

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
- 测试、lint、类型检查和构建结果
- CI 结果
- push 结果
- merge 状态（默认应为未合并）
- 任何未运行的检查、已知限制或需要用户决定的事项
