# MeetingLens 安全与隐私设计

## 1. 安全目标

MeetingLens 处理录音、转录、文档、屏幕上下文和 API 凭据，这些都属于高敏感数据。安全目标是：未经同意不采集、未经确认不外发、最小化本地暴露、允许完整删除，并让每个权限边界可检查。

MeetingLens 当前采用 Personal Edition 基线，近期只推进总结模式；翻译、实时能力和独立会议模式工作区为 Frozen / Deferred。未来如解冻其他模式，仍必须复用本文件的同意、外发、保留、删除和安全降级边界，不得另建安全例外。

## 2. 保护资产

- DeepSeek API key 和未来的 provider credentials
- 原始会议录音、voice samples 和上传文档
- 转录、翻译、摘要、行动项及参会者信息
- 屏幕文字上下文和可能包含的账号/财务/健康信息
- SQLite 数据库、导出文件和本地备份
- 应用更新、构建依赖和 CI 凭据

## 3. 威胁模型

| 威胁                 | 示例                                  | 主要控制                                            |
| -------------------- | ------------------------------------- | --------------------------------------------------- |
| 意外提交             | `.env`、数据库或录音进入 Git          | 严格 `.gitignore`、staged 扫描、PR 检查             |
| WebView 被利用       | XSS 调用本地命令                      | CSP、无远程内容、最小 Tauri capabilities            |
| 本机进程访问 sidecar | 猜测端口并读取任务                    | loopback、临时令牌、短超时；随机端口当前 Deferred   |
| 恶意上传文件         | 路径穿越、解析器漏洞、压缩炸弹        | MIME/大小/扩展联合校验、隔离目录、资源限制          |
| 恶意转录文本         | 超大输入、控制字符、提示/URL/代码诱导 | 严格 UTF-8、有界同步解析、内容只作数据、不执行/联网 |
| 外部数据泄漏         | 完整转录被默认发送                    | 明示确认、用户所选范围、Rust 最小化和 fail closed   |
| 日志泄漏             | 错误堆栈包含正文或 key                | 结构化 allowlist 日志、集中脱敏                     |
| 供应链风险           | npm/Python/Rust 依赖被劫持            | lockfile、依赖审计、最小依赖、CI 可复现安装         |
| 不完整删除           | WAL、临时文件仍保留                   | 删除协调、WAL checkpoint、临时目录清理测试          |

MVP 1A-3B-1 的本地会话认证防止局域网设备、普通网页和未持有当前 token 的本机客户端直接调用 Sidecar，并防止错误配置将服务绑定到非 loopback 地址。它不声称防御已取得当前 Windows 用户同等权限、可读取进程内存/环境的恶意软件，也不防御管理员或内核级攻击者。

## 4. 凭据管理

### 当前与开发阶段

- Rust DeepSeek Provider 已接入固定官方 host/model；只有受信任 Rust 执行路径可以消费真实 API key 和发起请求，正式用户生成入口仍未接线。
- 真实 DeepSeek 凭据不得保存在 `.env` 或环境变量中；Rust Provider 不提供明文开发回退，只能通过 `SecretStore` 消费 key。
- `.env.example` 只能包含空值或明显的非秘密占位符，不得暗示 Provider 会从环境中读取 key。
- 测试使用 fake provider，不调用真实付费 API。

### SecretStore 基线

- API key 存入当前 Windows 用户的 Credential Manager Generic Credential，并通过可替换的 Rust `SecretStore` 接口访问；正式 target 固定为 `com.gray.meetinglens/secret/deepseek-api-key`。
- WebView 只可 set/status/delete，不能读回、显示、复制或回填 key；`get_for_backend` 不注册为 Tauri command，当前也不把 key 传给 Python Sidecar。
- key 不写入 SQLite、配置 JSON、日志、崩溃报告、遥测或导出。
- Rust 使用零化包装减少临时明文驻留；JavaScript 输入期间仍会短暂持有明文，本项目不声称能清除交换文件或全部进程内存。
- 撤销/更新 key 后清空输入和应用持有的临时缓冲区，并只提供配置状态而不是显示值。
- Windows 11 发布版使用 Windows Credential Manager；不得把 key 放入 WebView 的 localStorage、前端 bundle 或 sidecar 命令行参数。
- 非 Windows 平台 fail closed，不使用文件、SQLite、注册表明文或浏览器存储回退。

真实 Provider 只在已授权 run 准备执行时由 Rust 通过 `get_for_backend` 从 SecretStore 取得 key。API key 不得进入 React 读取面、SQLite、日志、Python、Sidecar 或命令行；缺少 key 时请求在 transport 调用前 fail closed。

界面语言是非敏感本地偏好。WebView 只可在命名空间键 `meetinglens.uiLocale` 保存允许的 `zh-CN` 或 `en-US` 枚举；不得在该存储中写入会议、文档、转录、纪要、凭据或路径。未知、损坏或不可访问的值安全回退到系统语言规则和 `en-US`，不得用任意值构造语言包路径、远程请求或动态 import。清除 WebView 存储只重置界面语言，不影响 SQLite、私有文件或 SecretStore。

当前边界不防御取得同等 Windows 用户权限、可读取进程内存或用户 Credential Manager 的恶意软件，也不防御管理员/内核级攻击者。Provider 协议、错误和取消使用本地 fake transport 验证；真实付费 DeepSeek API smoke 未执行，因此不得声称已经验证真实账户、余额或端到端联网结果。

任何已经出现在聊天、日志或提交中的 key 都应视为已泄漏并立即撤销，而不是仅删除文件。

## 5. 实时录音与设备同意（Frozen / Deferred）

当前 Personal Edition 的近期音频路线是用户主动导入录音文件后执行本地一次性批量转录，不申请麦克风或系统音频权限。以下规则保留给未来可能解冻的实时采集能力：

- 应用启动或仅选择模式时不得自动申请、打开或探测音频/屏幕内容。
- 麦克风、系统音频和保存原始音频是三个独立选择；开始采集必须由明确点击触发，并持续显示不可忽略的来源指示。
- UI 在首次录音前提示用户遵守所在地法律、组织政策和参会者同意要求。
- 撤销任一采集同意后立即停止对应来源并完成文件句柄关闭；其他已同意来源可以继续。
- 不实现隐藏录音、静默自启或远程触发录音。
- 录音设备 ID 属于本地设置，不写入诊断日志。
- “不保存原始音频”不等于“没有处理音频”；界面必须分别说明临时处理、规范化转录保留和文件导出。
- 权限或设备不可用时安全降级：无屏幕可继续音频会议，无音频可进入总结模式，不得诱导用户扩大权限。

MVP 1A-3A 的 `MockCaptureProvider` 不访问真实设备、不申请权限，也不生成媒体文件；“microphone/systemAudio/window/screen”只作为未来契约类型和显式 Mock 来源存在。真实权限与同意流程只能在对应后续阶段实现。

## 6. 屏幕上下文（可选）

- 默认关闭，当前可选阶段只读取用户明确选定的具体窗口。
- 当前可选阶段只允许用户明确选择具体窗口。全屏捕获和持续监控整个桌面属于 Frozen / Deferred，不属于 `feat/window-context`。
- 当前首选受控 accessibility text，只保存用户确认的文字片段或 screen evidence，默认不保存屏幕关键帧。
- screen OCR、区域/全屏捕获、截图缓存和自动敏感区域遮蔽属于 Frozen / Deferred；未来解冻时仍须在保存前提供预览、编辑和取消。
- 不对屏幕数据执行后台连续上传。
- 只允许捕获用户显式选定的窗口；以稳定窗口句柄和进程身份复核目标，窗口关闭、句柄/进程身份失效或权限撤销时立即暂停。普通标题变化不能单独撤销捕获。
- 屏幕变化本身不能直接证明话题变化、决定或行动项，也不能单独触发最终总结。
- Screenpipe 只保留为未来解冻时可重新评估的 Capture Provider 候选。采用前必须先复核商业授权、版本、默认采集行为、本地 API 暴露和资源占用；未经批准不得把它作为常驻全局采集器或核心依赖。

## 7. 外部 AI 数据流

PR #21 的 Rust `DeterministicMockSummaryProvider` 仍只读取冻结 snapshot，不读取 SecretStore、环境凭据、文件、Sidecar 或网络；它现在只允许测试/debug 隔离运行，历史 Mock run 继续可读，release WebView 不可选择或执行。production `DeepSeekSummaryProvider` 是唯一真实 Summary 外发路径。这里的“Mock 零网络”只表示 MeetingLens Mock 不外发数据，不代表 WebView2 Runtime 在物理层面完全断网。

调用 DeepSeek 前必须：

1. 界面显示 Provider、用途以及将发送的用户所选转录范围，明确数据会发送给 DeepSeek；Rust 对该范围和确认状态作最终校验。
2. 第一次准备外发时让用户选择“本次同意”“记住我的选择”或“取消”；没有有效确认时 fail closed。
3. 只发送不可变输入快照中的用户所选规范化转录片段，不发送音频、截图、原始转录文件或原始参考文档。
4. 只有 Rust Provider 可以从 SecretStore 读取 key 和发起 HTTPS 请求；React/WebView、Python 和 Sidecar 不持有 key，也不直接请求 DeepSeek。
5. 设置 10 秒连接/180 秒总超时、有界 request/response、无自动重试和可唤醒取消；错误、日志和 command response 不得包含请求正文、响应全文或 key。
6. 严格解析和验证 Provider 响应，继续由 Rust 验证 evidence allowlist，再原子提交结果。

Rust transport 固定使用 rustls HTTPS 和上述单一 endpoint，禁止 redirects 与环境 proxy，不接受 WebView URL。每个 run 最多一轮非流式、thinking disabled、JSON-object 请求，不自动重试；请求体上限 2 MiB、响应体上限 1 MiB，读取过程持续执行上限。DeepSeek snapshot 额外限制为 1 MiB；schema v4 的 10 MiB 通用历史快照仍可读取，但超出 Provider 上限的执行在发送前返回 `summary_provider_input_too_large`。

Disclosure 固定覆盖 `transcript_text`、`speaker_labels_and_names`、`time_offsets`、`evidence_ids`、`summary_template` 和 `output_language`；界面在 evidence 类别中明确包含 source/segment 顺序与 run-scoped evidence IDs。除这些 run-scoped evidence IDs 外，请求不包含其他数据库标识，也不包含 meeting title、source display name/path、language/confidence、原始文件、Prompt 或 Key。关闭 Dialog、Escape 或取消都不得产生授权；loading 防重复确认，错误后只能安全重试或返回。

持续实时翻译和定时 checkpoint 当前为 Frozen / Deferred。若未来解冻，它们仍是区别于一次性纪要生成的外发目的，必须分别同意并提供持续状态与停止入口；任何阶段都不能把原始音频、截图或用户文件直接发送给外部 Provider。

未来设备采集和持续外发的同意仍只对当前应用会话有效，不能因历史状态自动重新打开设备或恢复发送。Personal Edition 的一次性 DeepSeek Summary 确认按 run 在 Rust 内存中保存 10 分钟并单次消费；“记住我的选择”只在 schema v5 保存 provider、policy version/fingerprint 和 UTC 时间，不建立通用 Consent 审计体系。

提供商的保留政策和地区处理条款必须在启用生产集成前重新审查。

DeepSeek production identity 固定为 `api.deepseek.com`、`deepseek-v4-pro` 与 `https://api.deepseek.com/chat/completions`；不允许用户或 WebView 配置 base URL、model、代理或重定向目标。价格和数据处理条款仍须由用户在实际付费使用前确认。

这套最小 Consent 只适用于单人自用版本，不足以作为向团队、客户或公众分发时的企业合规方案。

## 8. Tauri 安全基线

- WebView 只加载打包的本地前端，不允许远程页面获得 Tauri API 权限。
- 设置非空 CSP；默认 `default-src 'self'`，只开放实际需要的来源。
- `src-tauri/capabilities` 采用最小权限；不授予任意 shell 或全文件系统访问。
- 自定义应用 commands 登记进 Tauri `AppManifest`；Secret permission set 只授予本地 `main` 窗口 status/set/delete，不包含 get/reveal，也不授予 remote WebView。
- Document permission set 只授予本地 `main` 窗口 select/import、list/get、delete/retry 和 policy 六个自定义 commands；不授予 WebView dialog、fs、shell、process 或 HTTP permission。
- Transcript permission set 只授予本地 `main` 窗口原子粘贴/原生选择、source/segment 查询、revision 修正、source 删除、导入 policy，以及当前总结输入 UI 所需的有界分页和 source-scoped speaker 命名 commands；不接受源路径，不授予 WebView dialog、fs、shell、process 或 HTTP permission。
- Summary permission set 只授予本地 `main` 窗口十二个固定 commands：既有 policy/run/evidence 八个，加上 Provider status、external disclosure、confirm disclosure 和 revoke consent。Commands 只接受 meeting/source/run IDs、模板、输出语言、有界分页参数和 confirm 的 `remember` 布尔值；不接受 Provider/model/endpoint、结果、evidence、凭据、正文、时间戳或路径，也不授予 dialog、fs、shell、process 或 HTTP permission。
- 文件对话框只由 Rust 打开；返回路径继续留在 Rust 并再次校验，不能由前端传入、拼接或读回任意路径。
- Sidecar 通过固定名称 allowlist 启动，不向前端暴露通用命令执行。
- Rust commands 对长度、枚举、UUID、路径、文件大小和超时做边界校验。

## 9. Sidecar 安全基线

- 默认只监听 `127.0.0.1`，启动失败时不得退回 `0.0.0.0`。
- 当前使用 loopback 和高熵会话 token；随机高位端口属于 Deferred 的完整 supervisor 加固。
- `/health` 只返回状态和版本；所有端点（包括 `/health`）默认要求认证。
- MVP 1A-3B-1 继续使用固定开发端口，但只允许 `127.0.0.1`，并由 Rust 每次启动生成至少 32 bytes 随机数据的临时 bearer token。
- Token 只通过子进程环境传递，不写入命令行、URL、文件、SQLite、WebView、日志或凭据库；应用重启后旧会话失效。
- Rust 是唯一授权业务调用方并集中注入 Authorization Header；React/WebView 不直接请求 Sidecar，也不持有 token。
- 随机端口、打包二进制、崩溃重启、健康监控和孤儿进程回收属于 Deferred，不是当前 Personal Edition 日常可用的前置条件。
- 限制请求体大小、并发任务数、单任务运行时间和临时磁盘使用量。
- 任务只引用由 Rust 注册的 artifact ID，不接受任意绝对路径。
- 进程退出时清理临时文件；启动时清理已过期 job 目录。

计划中的录音文件转录不通过常驻 HTTP Sidecar，而由 Rust 每次启动一个 CPU int8 faster-whisper Worker。该 Worker 不监听端口、不继承凭据，并复用 Docling 已建立的程序 allowlist、受控文件引用、资源/超时、取消、输出校验和进程树回收边界。

## 10. 文本转录与参考文档输入防护

### 10.1 转录文本输入

转录与参考 document 是不同语义来源。粘贴文本由 WebView 通过有界 DTO 交给 Rust；TXT/SRT/VTT 只能由 Rust 原生文件选择器读取，源绝对路径不得返回 React、写入 SQLite 或进入日志。转录解析是本地轻量同步操作，不调用 Docling、Sidecar、大模型或网络。

第一版固定边界：

- 粘贴内容最大 2 MiB；TXT/SRT/VTT 单文件最大 10 MiB。
- 每个 transcript source 最多 20,000 个 segments；单 segment 正文最多 64 KiB。
- 严格 UTF-8；拒绝 NUL 和除 TAB/LF/CR 外的危险 C0 控制字符，拒绝无效/替换解码。
- 普通文本按非空段落或明确 `姓名: 内容` 保序解析；不明确的 speaker 为空，不用模型猜测。
- SRT/VTT 保留 cue 顺序与相对时间；不自动翻译、不推断缺失时间戳。
- 输入中的 URL、HTML、Markdown、代码、命令和提示词全部只作为不可信文本数据；不得访问、执行、shell-open 或交给 WebView 做不安全渲染。
- 正文、说话人、文件名、解析失败上下文和 prompt 不进入普通日志；稳定错误只包含 code、retryable 与安全 ID。
- 规范化 segments、来源类型、显示文件名和 SHA-256 可持久化；首版不长期复制原始转录文件，不影响用户源文件。哈希只用于来源识别/去重，不是信任或恶意软件证明。
- 未提交的粘贴正文只能短暂存在于 React 组件内存；成功、取消和页面卸载时清理。正文、segment 数组、speaker 名称、文件名和路径不得写入 localStorage、sessionStorage、IndexedDB、console、遥测或错误对象；唯一允许的 WebView 持久偏好仍是受限 UI locale。开发 Smoke 如需跨重启，只能暂存合成记录的 UUID/阶段，结束后删除。
- segment 查询由后端按稳定 ordinal 提供有界分页，production 单页上限 200；不得将 20,000 条全量 DTO 发送给 WebView 后再切片。文件名、speaker 和正文只用 React 文本节点显示，禁止 `dangerouslySetInnerHTML`。
- source-level speaker 命名必须后端验证 meeting/source/label 归属，在单事务中只更新 `speaker_name`、revision 与 `corrected_at`，保留原始 `speaker_label`；清空使用明确 NULL，空名称拒绝。操作不得跨 source、建立身份档案、写入正文日志或使用 AI 猜测姓名。
- 单个 Summary run 只冻结用户选择 source 中的 final segments，合计最多 20,000 条和 10 MiB UTF-8 `original_text`；超限、空输入、跨 meeting source、重复 source 和无法原子冻结的输入全部 fail closed，不截断、不抽样。快照复制 source/segment IDs、三个层级 ordinal、revision、speaker、offset、正文、language、confidence 与创建时间；Rust Provider 只能读取这些 snapshot rows，不能在执行期间回读 live segment。
- 每个 snapshot row 的随机 UUID 是当前 run 专用 evidence allowlist。Provider 输出必须在 Rust 内验证 run ID、schema version、结果大小/数量、必要 evidence 非空、ID 无重复且全部属于该 run；未知或跨 run ID 返回稳定错误，未完成完整验证前不得写入任何 `content_json` 或 evidence。
- Summary run/evidence 查询只允许默认 50、最大 100 的 keyset 分页；wire DTO 包括嵌套 result 都使用 camelCase 并拒绝未知字段。revision 竞态和 evidence tuple 失败使用固定的 `summary_revision_conflict` / `summary_evidence_invalid`，不得返回 SQL、路径或 Provider wording。
- schema v4 独立持久化 `provider_mode`；production Service 只写/执行固定 `deepseek/external`，历史 `deterministic_mock/mock` 继续可读但只通过测试/debug 注入执行，`disabled` 或未知 mode 不能代表一次 run。

- schema v5 `provider_consent_preferences` 只保存 DeepSeek policy version/fingerprint 与 consented/updated 时间；不保存正文、run/meeting ID、Prompt、响应、路径或 Key。policy/host/model/数据类别任一变化都会让 remembered 状态失效；撤销删除持久行并清空所有未消费一次性授权。

删除 source 时同时处理其 segments。queued/running Summary run 使用该 source 时稳定拒绝 `transcript_source_in_use`；不采用隐式 pending，也不在 run 中途移除输入。terminal run 不阻塞删除：同一事务先把依赖结果标记 `source_deleted` stale，保留实际引用且最多 1,024 UTF-8 bytes 的 evidence excerpt、speaker、offset/ordinal 和 revision，清空完整 snapshot 正文与非必要显示元数据，再删除 live source/segments。segment 修正产生新 revision，并把依赖旧 revision 的 run 标记 `segment_revised` stale；旧快照和结果保持不变，不得后台静默重新生成或覆盖用户编辑。

PR #21 的 Summary Run Core 已使用 development-only Windows 双启动 harness：Rust 校验系统 Temp 直属隔离 app-data 根和匹配的 `com.gray.meetinglens.summarysmoke.<uuid>` 合成 Tauri identifier，验证 SQLite/WebView2 profile 隔离、completed result、revision、stale、evidence、running recovery、应用代码零外发和最终清理；release 不包含该 harness 或 debug app-data override。这是已完成高风险阶段的历史验收证据，不是以后所有普通 UI PR 的默认矩阵。

### 10.2 参考文档上传

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
- 限制尽可能在完整复制前检查；复制中实际大小超限时立即停止并清理临时文件。PR #14 的 Docling Worker 另设 4 GiB Job Object 内存、150 秒处理时限、300 页、100 MiB 输入、1 MiB 协议消息和 128 MiB 总输出限制；全局磁盘配额管理器是后续需求。
- 原始文件名只作显示元数据；内部使用 document UUID，拒绝 `..`、绝对路径、盘符、UNC、Alternate Data Streams 和路径逃逸。
- 文件在应用私有 staging 随机临时文件中流式复制并计算 SHA-256，验证最终大小/类型/配额后同卷原子 rename；失败清理临时文件。SHA-256 只用于完整性和同会议去重，不是恶意软件检测。
- 不执行、不记录文件内容，不长期保存到 Temp/Git/工作目录，也不为文件设置可执行权限。
- 文档内容解析只允许隔离的 Docling Worker 执行；PR #14 已建立独立锁定环境、Rust 注册且带哈希的受控引用、单请求进程、取消、超时、Windows Job Object 内存限制、页数/输出限制和畸形协议测试。`feat/document-parsing-orchestration` 才把该 Worker 接入文档状态与持久化。
- Worker 不监听端口、不继承凭据/proxy/session token、不访问 SQLite；Docling 的远程服务、插件、VLM 和隐式模型下载均关闭。模型只通过显式脚本下载到仓库外目录，并用固定 revision 和逐文件 SHA-256 manifest 验证。
- Worker 输出只能位于 Rust 批准的 parser staging 根并使用固定相对文件名；Rust 在持久化前重新验证路径、文件类型、大小、总量与 SHA-256。稳定错误不得包含原文、文件名、绝对路径、环境值或原生异常。
- Docling Worker Foundation 已完成；产品接线排在 `feat/audio-file-transcription` 之后，由简化的 `feat/document-parsing-orchestration` 同时完成解析、document chunks/evidence 和现有 Summary pipeline 接线。每次失败自动重试一次，再失败提示用户手动重新导入；当前不建立通用调度器、多级恢复、长期 checkpoint 或企业监控面板。Docling 只增强参考文档证据，不是粘贴/TXT/SRT/VTT 或第一份 transcript-only Summary 的前置依赖。

## 11. 本地存储与删除

- SQLite 默认不等同于静态加密；首版不得宣传“数据库已加密”。
- 依赖操作系统账户与应用私有目录权限作为基础保护。
- 若产品需要抵御本机磁盘读取，应在后续引入经评估的 SQLCipher/文件加密和密钥管理。
- MVP 1B-1A 已将无附件硬删除升级为持久 `deleting`/`deleted_at` tombstone 与 `deletion_jobs`，冻结编辑、录音开始、文件导入和纪要任务，再协调当前文档及未来 artifacts。
- 待删除文件必须同卷移动至应用私有 `deletion-trash` 或采用等效可恢复设计。全部清理成功后才最终移除 meeting；失败进入 `delete_failed`，保留安全错误码并允许启动恢复/用户重试。
- `draft`/`ready` 接受会议删除；`recording`/`processing`/既有保留状态拒绝；`deleting` 重复请求返回稳定 pending；`delete_failed` 允许幂等重试；未知状态 fail closed。
- 单文档删除先进入 `deleting`，文件和 artifacts 清理成功后才移除元数据，失败进入 `delete_failed`。`importing`/`parsing` 必须先取消或等待；删除不影响用户源文件。
- transcript source 不长期复制原始文件；删除协调其 segments、summary snapshot 引用和 stale 状态，但不删除用户源文件。正在使用该 source 的 Summary run 读取不可变快照，不得因并发删除产生不可解释证据。
- schema v4 的完整 snapshot 正文只为 run 执行和可重建性保存在本地 SQLite；显式 source 删除会安全清理 terminal snapshot 正文，而整场 meeting 删除会通过 meeting FK/删除协调器清理 summaries、snapshots 与 evidence。schema v5 Provider 偏好是全局设置，不因删除单场会议而改变；用户可在 Settings 单独撤销。会议删除开始前 queued/running Summary runs 先转为 cancelled，避免留下孤立运行。
- 应用启动时把遗留 `running` Summary run 转为 `failed + summary_interrupted`；不得自动重放可能已经部分执行的 Provider。未来设备采集或持续外发不得因旧状态自动恢复；用户明确选择记住的 Personal Edition DeepSeek Summary 确认除外，并仍须通过 Rust 最小状态校验。
- 不允许先删除数据库再尽力删除文件，也不允许先删除文件再尝试数据库删除；崩溃后不得留下无法识别的孤儿文件。
- 临时文件必须有 TTL 和崩溃恢复清理机制。
- 应用生命周期内独占锁定 `meeting-data/.meetinglens-storage.lock`；第二写入实例 fail closed。启动恢复与正常写操作还由同一进程内 mutation mutex 串行化，未知文件只移动到隔离区而不直接删除。

## 12. 日志与错误处理

允许记录：事件名、匿名任务 ID、状态、耗时、错误码、应用/sidecar 版本。

禁止记录：密钥、Authorization header、请求/响应正文、转录、文件名、参与者、屏幕内容、环境变量和绝对用户路径。

Summary Provider request/output 的 `Debug`、错误映射、测试 assertion 和 command response 同样遵守该 allowlist；不得输出 snapshot 正文、speaker、evidence excerpt、完整 `content_json` 或底层 SQL/SQLite 异常。

Sidecar 认证失败只允许记录事件类型、结果和稳定错误码；当前实现默认关闭 Python 请求日志，并禁止 Rust 输出 HeaderMap、原始 reqwest/Python 错误或子进程环境。DeepSeek API key 是外部服务凭据，与短期 Sidecar Session Token 完全不同，二者不得复用。

SecretStore command 不记录请求 DTO、凭据值、长度、首尾字符、Credential blob、完整 target、Windows 用户名或 Win32 原始错误。当前没有新增完整日志框架；稳定错误码和无敏感上下文是本阶段的最小脱敏边界。

生产日志默认保留时间应有限，用户可以查看大小并一键删除。

## 13. 供应链与 CI

- 提交 `package-lock.json` 和 `Cargo.lock`，CI 使用 `npm ci` 与 `cargo --locked`。
- GitHub Actions 权限设为 `contents: read`，除非任务明确需要更多权限。
- PR 中运行前端测试/构建、Python 测试和 Rust 检查。
- 定期运行 npm、Cargo 和 Python 依赖审计；升级需经过测试，不自动跳过失败。
- 安装包完善、签名、SBOM、自动更新和商业分发当前为 Frozen / Deferred；未来如解冻，发布产物仍需要校验和与可追溯构建。
- 所有直接 npm、Cargo 和 Python 依赖使用精确版本；升级在独立 PR 中完成并记录兼容性验证。
- Diart、WhisperX 与实时 diarization 当前为 Deferred；未来如解冻，仍必须使用相互隔离的 Worker 环境和 lockfile，不得通过放宽 NumPy/pyannote 约束强行共存。
- AI 模型下载记录来源与校验和；模型、缓存和 Hugging Face token 不得进入 Git。

## 14. 提交前敏感信息检查

最低流程：

1. 查看 `git diff --cached --name-status`。
2. 查看 `git diff --cached`，确认没有真实会议数据。
3. 扫描常见 token、私钥、`api_key/password/token = value` 模式。
4. 确认 `.env`、数据库、录音、截图、上传文件和日志均未被跟踪。
5. 若发现敏感内容，停止提交；从 index 移除并通知用户。已经进入历史的凭据必须撤销。

## 15. 分阶段安全交付

| 状态             | 阶段                                     | 必须完成的安全控制                                                                                               |
| ---------------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Implemented      | 基础、文件与 Worker（MVP 1A/1B、PR #14） | SQLite/migration、路径守卫、SecretStore、Sidecar auth、安全文件导入、可恢复删除、隔离 Docling Worker             |
| Implemented      | 转录与总结入口（PR #18–#20）             | 严格 UTF-8、有界解析/分页、Rust 原生选择器、无路径 DTO、浏览器不持久化正文                                       |
| Implemented      | Summary Run Core（PR #21）               | schema v4、series/revision、不可变快照、Rust 零网络 Mock、evidence allowlist、stale/删除/取消/恢复               |
| Current delivery | `feat/summary-provider-deepseek`         | schema v5、Rust-only 固定请求、SecretStore、一次性/记住/撤销、无确认 fail closed、HTTPS/超时/取消、严格校验/脱敏 |
| Next             | `feat/summary-mode-minutes`              | 正式生成与 Consent 接线、安全结果展示、正文修改、整份重新生成、revision/stale 保护、Markdown 复制/保存           |
| Planned          | `feat/audio-file-transcription`          | Rust 原生文件边界、一次性 faster-whisper Worker、CPU int8、资源/取消/输出校验、只写既有 transcript 模型          |
| Planned          | `feat/document-parsing-orchestration`    | Rust-only Docling 调用、一次自动重试、artifact 原子提交、document evidence allowlist、再次失败后提示重新导入     |
| Optional         | `feat/window-context`                    | 用户明确选择窗口、身份复核、权限撤销即暂停、默认不保存帧、screen evidence 与其他来源隔离                         |
| Frozen/Deferred  | 实时与商业能力                           | 实时采集/ASR/翻译/diarization、checkpoint、浮窗、独立会议工作区、structured visuals、DOCX/PDF、签名和商业分发    |

Docling 的进程/依赖攻击面已由 PR #14 隔离；它不成为第一份 transcript-only 纪要的前置条件。真实外部数据流已被限制在合并最小 Consent 的 Rust DeepSeek 边界内，正式纪要页面由下一阶段 `feat/summary-mode-minutes` 集成。一个功能 PR 可以同时包含必要的 Data/API/UI，但不得把无关的转录持久化、外部 AI、文件解析或其他信任边界混入同一能力。

## 16. 安全验收门槛

验收按风险分级：

- 文档 PR：Markdown/格式、链接、路线一致性、敏感信息扫描和 CI。
- 不修改 schema 的普通功能 PR：一次本机启动、主要用户流程、相关自动测试、正文/密钥/路径/禁止文件泄漏检查和干净工作区。
- 修改 schema、migration、安全边界或真实网络的高风险 PR：升级与 checksum、integrity/foreign-key、必要的双启动或隔离 app-data、故障恢复、敏感信息和网络边界验证。

以下硬性安全门槛按改动范围继续适用：

- CSP 非空且没有不必要的远程源。
- Tauri capability 中没有通用 shell/文件系统权限。
- Sidecar 绑定 loopback，健康响应不泄漏环境信息。
- 测试能证明未知 sidecar 路径返回 404。
- 删除测试覆盖数据库与文件派生物。
- 外部 AI 集成在没有 key、没有同意或发送范围为空时均 fail closed。
- 仓库和 staged 内容通过敏感信息扫描。
- PR 实际触及 Windows 凭据库、文件对话框、权限、窗口选择、Worker/Sidecar 回收等桌面安全行为时，必须在用户本机验收；CI 通过不能替代。双隔离和完整双启动不再是普通 UI PR 的默认要求。
