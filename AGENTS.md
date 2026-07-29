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
   - Rust 可用时：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 和 `cargo check --manifest-path src-tauri/Cargo.toml`
6. 直接依赖必须锁定精确版本并提交相应 lockfile；不得为解决兼容性问题擅自放宽版本范围或关闭安全检查。
7. Windows 11 和 Python 3.12 是当前项目基线。其他平台/版本只能作为明确标注的额外验证，不得替代该基线。

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
