# MeetingLens 安全与隐私设计

## 1. 安全目标

MeetingLens 处理录音、转录、文档、屏幕上下文和 API 凭据，这些都属于高敏感数据。安全目标是：未经同意不采集、未经确认不外发、最小化本地暴露、允许完整删除，并让每个权限边界可检查。

## 2. 保护资产

- DeepSeek API key 和未来的 provider credentials
- 原始会议录音、voice samples 和上传文档
- 转录、翻译、摘要、行动项及参会者信息
- 屏幕文字上下文和可能包含的账号/财务/健康信息
- SQLite 数据库、导出文件和本地备份
- 应用更新、构建依赖和 CI 凭据

## 3. 威胁模型

| 威胁                 | 示例                           | 主要控制                                    |
| -------------------- | ------------------------------ | ------------------------------------------- |
| 意外提交             | `.env`、数据库或录音进入 Git   | 严格 `.gitignore`、staged 扫描、PR 检查     |
| WebView 被利用       | XSS 调用本地命令               | CSP、无远程内容、最小 Tauri capabilities    |
| 本机进程访问 sidecar | 猜测端口并读取任务             | loopback、随机端口、临时令牌、短超时        |
| 恶意上传文件         | 路径穿越、解析器漏洞、压缩炸弹 | MIME/大小/扩展联合校验、隔离目录、资源限制  |
| 外部数据泄漏         | 完整转录被默认发送             | 明示同意、发送预览、裁剪和脱敏              |
| 日志泄漏             | 错误堆栈包含正文或 key         | 结构化 allowlist 日志、集中脱敏             |
| 供应链风险           | npm/Python/Rust 依赖被劫持     | lockfile、依赖审计、最小依赖、CI 可复现安装 |
| 不完整删除           | WAL、临时文件仍保留            | 删除协调、WAL checkpoint、临时目录清理测试  |

MVP 1A-3B-1 的本地会话认证防止局域网设备、普通网页和未持有当前 token 的本机客户端直接调用 Sidecar，并防止错误配置将服务绑定到非 loopback 地址。它不声称防御已取得当前 Windows 用户同等权限、可读取进程内存/环境的恶意软件，也不防御管理员或内核级攻击者。

## 4. 凭据管理

### 开发阶段

- API key 只存放于根目录 `.env`，该文件必须保持 Git ignored/untracked。
- 代码从进程环境读取 `DEEPSEEK_API_KEY`，不得提供硬编码回退值。
- `.env.example` 只能包含空值或明显占位符。
- 测试使用 fake provider，不调用真实付费 API。

### 发布阶段

- API key 存入当前 Windows 用户的 Credential Manager Generic Credential，并通过可替换的 Rust `SecretStore` 接口访问；正式 target 固定为 `com.gray.meetinglens/secret/deepseek-api-key`。
- WebView 只可 set/status/delete，不能读回、显示、复制或回填 key；`get_for_backend` 不注册为 Tauri command，当前也不把 key 传给 Python Sidecar。
- key 不写入 SQLite、配置 JSON、日志、崩溃报告、遥测或导出。
- Rust 使用零化包装减少临时明文驻留；JavaScript 输入期间仍会短暂持有明文，本项目不声称能清除交换文件或全部进程内存。
- 撤销/更新 key 后清空输入和应用持有的临时缓冲区，并只提供配置状态而不是显示值。
- Windows 11 发布版使用 Windows Credential Manager；不得把 key 放入 WebView 的 localStorage、前端 bundle 或 sidecar 命令行参数。
- 非 Windows 平台 fail closed，不使用文件、SQLite、注册表明文或浏览器存储回退。

当前边界不防御取得同等 Windows 用户权限、可读取进程内存或用户 Credential Manager 的恶意软件，也不防御管理员/内核级攻击者。DeepSeek 网络有效性测试、真实调用和 Provider 消费凭据均不属于本阶段。

任何已经出现在聊天、日志或提交中的 key 都应视为已泄漏并立即撤销，而不是仅删除文件。

## 5. 录音与同意

- 开始录音必须由明确点击触发，并持续显示不可忽略的录音指示。
- UI 在首次录音前提示用户遵守所在地法律、组织政策和参会者同意要求。
- 撤销录音权限后立即停止采集并完成文件句柄关闭。
- 不实现隐藏录音、静默自启或远程触发录音。
- 录音设备 ID 属于本地设置，不写入诊断日志。

MVP 1A-3A 的 `MockCaptureProvider` 不访问真实设备、不申请权限，也不生成媒体文件；“microphone/systemAudio/window/screen”只作为未来契约类型和显式 Mock 来源存在。真实权限与同意流程只能在对应后续阶段实现。

## 6. 屏幕上下文

- 默认关闭，只采集用户选定窗口或区域。
- 首选本地 OCR 后保存文字片段，不长期保存原始截图。
- 保存前提供预览、编辑和取消；密码框、通知和常见敏感模式应自动遮蔽。
- 不对屏幕数据执行后台连续上传。
- 只允许捕获用户显式选定的窗口；窗口失效、标题/进程身份变化或权限撤销时立即暂停。
- Screenpipe 只能作为 MVP 3 中可替换的可选 Capture Provider。采用前必须先复核商业授权、版本、默认采集行为、本地 API 暴露和资源占用；未经批准不得把它作为常驻全局采集器或核心依赖。

## 7. 外部 AI 数据流

调用 DeepSeek 前必须：

1. 显示 provider、将发送的数据范围和用途。
2. 让用户确认本次发送；同意范围写入 `consent_events`。
3. 优先发送选择后的转录片段，不发送音频、截图或原始文档。
4. 执行可配置脱敏，并让用户查看最终文本预览。
5. 设置连接/读取超时和最大重试，错误中不得包含请求正文或 key。
6. 将 AI 输出标记为草稿，并保留来源链接供核验。

提供商的保留政策和地区处理条款必须在启用生产集成前重新审查。

DeepSeek 生产适配器必须使用当前模型名 `deepseek-v4-flash` / `deepseek-v4-pro`，base URL 仅允许 HTTPS；自定义地址需由用户明确配置并重新显示数据接收方。

## 8. Tauri 安全基线

- WebView 只加载打包的本地前端，不允许远程页面获得 Tauri API 权限。
- 设置非空 CSP；默认 `default-src 'self'`，只开放实际需要的来源。
- `src-tauri/capabilities` 采用最小权限；不授予任意 shell 或全文件系统访问。
- 自定义应用 commands 登记进 Tauri `AppManifest`；Secret permission set 只授予本地 `main` 窗口 status/set/delete，不包含 get/reveal，也不授予 remote WebView。
- Document permission set 只授予本地 `main` 窗口 select/import、list/get、delete/retry 和 policy 六个自定义 commands；不授予 WebView dialog、fs、shell、process 或 HTTP permission。
- 文件对话框只由 Rust 打开；返回路径继续留在 Rust 并再次校验，不能由前端传入、拼接或读回任意路径。
- Sidecar 通过固定名称 allowlist 启动，不向前端暴露通用命令执行。
- Rust commands 对长度、枚举、UUID、路径、文件大小和超时做边界校验。

## 9. Sidecar 安全基线

- 默认只监听 `127.0.0.1`，启动失败时不得退回 `0.0.0.0`。
- 发布模式使用随机高位端口和高熵会话 token。
- `/health` 只返回状态和版本；所有端点（包括 `/health`）默认要求认证。
- MVP 1A-3B-1 继续使用固定开发端口，但只允许 `127.0.0.1`，并由 Rust 每次启动生成至少 32 bytes 随机数据的临时 bearer token。
- Token 只通过子进程环境传递，不写入命令行、URL、文件、SQLite、WebView、日志或凭据库；应用重启后旧会话失效。
- Rust 是唯一授权业务调用方并集中注入 Authorization Header；React/WebView 不直接请求 Sidecar，也不持有 token。
- 随机端口、打包二进制、崩溃重启、健康监控和孤儿进程回收由 MVP 1C 的 supervisor 完成。
- 限制请求体大小、并发任务数、单任务运行时间和临时磁盘使用量。
- 任务只引用由 Rust 注册的 artifact ID，不接受任意绝对路径。
- 进程退出时清理临时文件；启动时清理已过期 job 目录。

## 10. 上传文件防护

所有用户导入文件均为不可信数据；“本地保存”不能作为信任依据。正式入口只允许用户通过 MeetingLens 的 Rust 原生文件选择器主动选择文件。Teams、Zoom、Meet、邮件或聊天附件必须先由用户下载再手动导入；不自动读取附件/云盘，不监控 Downloads 或桌面，不读取剪贴板文件，不接受任意路径拖放、远程 URL 或网络共享自动导入。

Rust 是文件选择、校验、复制和私有存储的信任边界。React/WebView 不获得任意文件系统读取权限，不接收源绝对路径，只接收 document ID、原始显示名称、安全类型、大小、状态和导入时间。Python Sidecar 当前不接收用户文件；DeepSeek 当前不接收文件内容。

导入进度通过每次调用独立的有类型 Tauri channel 发送，只包含安全显示名、阶段和有界字节计数；没有全局可订阅文件事件，也不发送路径、哈希、文件内容或内部异常。前端对未知状态和畸形 DTO fail closed，错误展示只使用稳定错误码和固定安全消息。

第一版 allowlist：

- `.pdf`、`.docx`、`.pptx`、`.xlsx`
- `.txt`、`.md`、`.csv`
- `.png`、`.jpg`、`.jpeg`

明确拒绝 `.doc`、`.xls`、`.ppt`、`.docm`、`.xlsm`、`.pptm`、`.zip`、`.rar`、`.7z`、`.exe`、`.msi`、`.dll`、`.bat`、`.cmd`、`.ps1`、`.js`、`.vbs`、`.html`、`.htm`，以及其他执行/脚本格式、双重扩展伪装、空文件和未知类型。

- 扩展名不能作为唯一依据；Rust 结合文件签名或安全类型识别，类型不一致或未知时 fail closed，不把未知格式自动当作纯文本。
- OOXML 虽是 ZIP 容器，但不因此允许普通 ZIP；导入阶段不解压，完整结构验证留给隔离 Docling Worker。
- MeetingLens 不启动或 shell-open 文件，不调用 Office 自动化，不执行宏、脚本或嵌入对象，不在 WebView 直接渲染原文件，也不自动访问文件内外部链接。
- 默认限制：单文件 100 MiB、单次选择 10 个、单会议 20 个、单会议累计 500 MiB、原始文件名 UTF-8 最大 255 bytes。Rust 最终执行且累计配额不能通过多次调用绕过。
- 限制尽可能在完整复制前检查；复制中实际大小超限时立即停止并清理临时文件。MVP 1B-2A Docling Worker 另设 4 GiB Job Object 内存、150 秒处理时限、300 页、100 MiB 输入、1 MiB 协议消息和 128 MiB 总输出限制；全局磁盘配额管理器是后续需求。
- 原始文件名只作显示元数据；内部使用 document UUID，拒绝 `..`、绝对路径、盘符、UNC、Alternate Data Streams 和路径逃逸。
- 文件在应用私有 staging 随机临时文件中流式复制并计算 SHA-256，验证最终大小/类型/配额后同卷原子 rename；失败清理临时文件。SHA-256 只用于完整性和同会议去重，不是恶意软件检测。
- 不执行、不记录文件内容，不长期保存到 Temp/Git/工作目录，也不为文件设置可执行权限。
- 文档内容解析只允许隔离的 Docling Worker 执行；MVP 1B-2A 已建立独立锁定环境、Rust 注册且带哈希的受控引用、单请求进程、取消、超时、Windows Job Object 内存限制、页数/输出限制和畸形协议测试。MVP 1B-2B 才能把该 Worker 接入文档状态与持久化。
- Worker 不监听端口、不继承凭据/proxy/session token、不访问 SQLite；Docling 的远程服务、插件、VLM 和隐式模型下载均关闭。模型只通过显式脚本下载到仓库外目录，并用固定 revision 和逐文件 SHA-256 manifest 验证。
- Worker 输出只能位于 Rust 批准的 parser staging 根并使用固定相对文件名；Rust 在持久化前重新验证路径、文件类型、大小、总量与 SHA-256。稳定错误不得包含原文、文件名、绝对路径、环境值或原生异常。

## 11. 本地存储与删除

- SQLite 默认不等同于静态加密；首版不得宣传“数据库已加密”。
- 依赖操作系统账户与应用私有目录权限作为基础保护。
- 若产品需要抵御本机磁盘读取，应在后续引入经评估的 SQLCipher/文件加密和密钥管理。
- MVP 1B-1A 已将无附件硬删除升级为持久 `deleting`/`deleted_at` tombstone 与 `deletion_jobs`，冻结编辑、录音开始、文件导入和纪要任务，再协调当前文档及未来 artifacts。
- 待删除文件必须同卷移动至应用私有 `deletion-trash` 或采用等效可恢复设计。全部清理成功后才最终移除 meeting；失败进入 `delete_failed`，保留安全错误码并允许启动恢复/用户重试。
- `draft`/`ready` 接受会议删除；`recording`/`processing`/既有保留状态拒绝；`deleting` 重复请求返回稳定 pending；`delete_failed` 允许幂等重试；未知状态 fail closed。
- 单文档删除先进入 `deleting`，文件和 artifacts 清理成功后才移除元数据，失败进入 `delete_failed`。`importing`/`parsing` 必须先取消或等待；删除不影响用户源文件。
- 不允许先删除数据库再尽力删除文件，也不允许先删除文件再尝试数据库删除；崩溃后不得留下无法识别的孤儿文件。
- 临时文件必须有 TTL 和崩溃恢复清理机制。
- 应用生命周期内独占锁定 `meeting-data/.meetinglens-storage.lock`；第二写入实例 fail closed。启动恢复与正常写操作还由同一进程内 mutation mutex 串行化，未知文件只移动到隔离区而不直接删除。

## 12. 日志与错误处理

允许记录：事件名、匿名任务 ID、状态、耗时、错误码、应用/sidecar 版本。

禁止记录：密钥、Authorization header、请求/响应正文、转录、文件名、参与者、屏幕内容、环境变量和绝对用户路径。

Sidecar 认证失败只允许记录事件类型、结果和稳定错误码；当前实现默认关闭 Python 请求日志，并禁止 Rust 输出 HeaderMap、原始 reqwest/Python 错误或子进程环境。DeepSeek API key 是未来外部服务凭据，与短期 Sidecar Session Token 完全不同，二者不得复用。

SecretStore command 不记录请求 DTO、凭据值、长度、首尾字符、Credential blob、完整 target、Windows 用户名或 Win32 原始错误。当前没有新增完整日志框架；稳定错误码和无敏感上下文是本阶段的最小脱敏边界。

生产日志默认保留时间应有限，用户可以查看大小并一键删除。

## 13. 供应链与 CI

- 提交 `package-lock.json` 和 `Cargo.lock`，CI 使用 `npm ci` 与 `cargo --locked`。
- GitHub Actions 权限设为 `contents: read`，除非任务明确需要更多权限。
- PR 中运行前端测试/构建、Python 测试和 Rust 检查。
- 定期运行 npm、Cargo 和 Python 依赖审计；升级需经过测试，不自动跳过失败。
- 发布产物后续需要签名、校验和与可追溯构建。
- 所有直接 npm、Cargo 和 Python 依赖使用精确版本；升级在独立 PR 中完成并记录兼容性验证。
- Diart 与 WhisperX 使用相互隔离的 worker 环境和 lockfile，不得通过放宽 NumPy/pyannote 约束强行共存。
- AI 模型下载记录来源与校验和；模型、缓存和 Hugging Face token 不得进入 Git。

## 14. 提交前敏感信息检查

最低流程：

1. 查看 `git diff --cached --name-status`。
2. 查看 `git diff --cached`，确认没有真实会议数据。
3. 扫描常见 token、私钥、`api_key/password/token = value` 模式。
4. 确认 `.env`、数据库、录音、截图、上传文件和日志均未被跟踪。
5. 若发现敏感内容，停止提交；从 index 移除并通知用户。已经进入历史的凭据必须撤销。

## 15. 分阶段安全交付

| 阶段        | 必须完成的安全控制                                                                     |
| ----------- | -------------------------------------------------------------------------------------- |
| MVP 1A-1    | SQLite 事务/迁移、路径规范化、删除语义、数据库与 WAL/SHM Git 忽略测试                  |
| MVP 1A-2-1  | DTO/UUID/状态校验、后端时间戳、最小 Tauri commands、稳定脱敏错误以及禁止前端路径或 SQL |
| MVP 1A-2-2  | 只用合成数据、集中调用 command、UI 错误不泄漏内部路径或正文                            |
| MVP 1A-3A   | Provider 稳定状态/错误、Mock 无设备/文件/网络访问、Registry 生命周期与清理测试         |
| MVP 1A-3B-1 | Sidecar 会话 token、Loopback、全路由认证、稳定错误、最小日志脱敏和 fail-closed 测试    |
| MVP 1A-3B-2 | SecretStore、Windows Credential Manager、安全配置与跨层日志 allowlist                  |
| MVP 1B-0    | 文件威胁边界、格式/配额、状态机、存储布局和可恢复删除规范                              |
| MVP 1B-1A   | Rust 安全导入、私有 staging、签名/配额、去重、删除 tombstone 与恢复测试                |
| MVP 1B-1B   | 最小 Document commands、原生选择器、无路径 DTO、UI 安全错误和删除/重试交互             |
| MVP 1B-2A   | 隔离 Docling Worker、锁定供应链、离线模型门槛、资源上限、取消和产物完整性校验          |
| MVP 1B-2B   | Rust-only 编排、解析状态/持久化、恢复/删除竞态和 WebView 无路径/正文边界               |
| MVP 1B-3    | 外部发送预览、同意、裁剪、脱敏、证据引用和无 key/无同意拒绝                            |
| MVP 1C      | 录音同意、sidecar supervisor、崩溃恢复、孤儿进程回收、临时文件清理和生命周期安全日志   |
| MVP 2       | voice sample 明示同意/删除、匿名标签、模型 worker 隔离                                 |
| MVP 3       | 目标窗口约束、敏感内容遮蔽、截图 TTL/删除、可选 Provider 权限边界                      |
| MVP 4       | 签名、升级/回滚、SBOM、诊断导出脱敏和卸载残留检查                                      |

Docling 的进程/依赖攻击面只能在 MVP 1B-2A 引入，数据库/状态/UI 编排只能在 MVP 1B-2B 引入；DeepSeek 的外部数据流只能在 MVP 1B-3 引入。不得在同一 PR 同时扩大文件存储、解析进程、持久化编排和外部 AI 信任边界。

## 16. 安全验收门槛

- CSP 非空且没有不必要的远程源。
- Tauri capability 中没有通用 shell/文件系统权限。
- Sidecar 绑定 loopback，健康响应不泄漏环境信息。
- 测试能证明未知 sidecar 路径返回 404。
- 删除测试覆盖数据库与文件派生物。
- 外部 AI 集成在没有 key、没有同意或发送范围为空时均 fail closed。
- 仓库和 staged 内容通过敏感信息扫描。
- Windows 凭据库、文件对话框、录音权限、窗口选择、sidecar 回收或安装/卸载等桌面安全行为必须在用户本机实际验收；CI 通过不能替代。
