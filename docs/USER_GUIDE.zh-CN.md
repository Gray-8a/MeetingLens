# MeetingLens 开发预览使用手册

> 开发预览版：本手册描述 `main` 截至 PR #25 以及当前 `feat/audio-file-transcription` 交付提供的本地桌面流程。它不是正式产品手册；音频转写新增 schema v6，同时保持既有 Summary/DeepSeek 行为。

## 1. MeetingLens 简介

MeetingLens 是面向 Windows 11 的本地优先 Personal Edition / 自用版会议工作台。它把粘贴、导入或本地转写的会议内容转化为可编辑、带 evidence 且可复制或保存为 Markdown 的会议纪要。当前功能实现可以在本机转写用户选择的录音，持久化并修正规范化转录，明确授权 Rust-only DeepSeek 路径，查看生成结果，保存正文修改，检查 evidence，并以新 revision 整份重新生成。

同一个应用内置简体中文（`zh-CN`）和 English（`en-US`），不需要安装两个版本。

## 2. 当前版本状态

当前版本是开发预览版。现有能力包括：

- 创建、编辑、开始、结束和删除本地会议；
- 在首页看到总结、翻译和会议三个模式入口；当前只有总结模式可进入，翻译和会议模式路线为 Frozen/Deferred；
- 粘贴 UTF-8 会议记录，或通过系统文件选择器导入 TXT、SRT、VTT；
- 选择单个 WAV、MP3、M4A 或含音轨 MP4，由本地 faster-whisper CPU int8 转写，并安全显示进度和取消；
- 查看转录来源、分页检查片段、修正正文与说话人显示名称，并删除来源；
- 在 Minutes 工作区选择转录来源、三种模板之一和中文/英文/双语输出；
- 明确发起或继续 DeepSeek run、查看 queued/running/completed/failed/cancelled 状态并取消 active run；
- 查看固定的 `overview`、`decisions`、`actionItems`、`risks`、`openQuestions` 五区结果、精确引用的 evidence 和 revision 历史；
- 在并发保护下编辑允许的正文，一键复制 canonical Markdown，通过 Rust 原生对话框保存 `.md`，并把整份结果重新生成为新 revision；
- 运行纯合成、仅内存中的模拟字幕流程；
- 通过系统文件选择器安全导入、去重、列出和删除会议文件副本；
- 查看 Mock Provider、本地 Sidecar 和数据库状态；
- 将 DeepSeek API Key 保存到 Windows Credential Manager，且 WebView 无法读回；
- 运行独立离线 Docling Worker 的开发验证，但该 Worker 尚未接入产品界面；
- 验证 schema v4 Summary series/revision、不可变转录快照与 run-owned evidence、schema v5 最小 remembered Consent，以及 schema v6 `imported_audio` 转录来源；
- 在设置中查看固定 DeepSeek host/model、remembered Consent 状态与时间，并撤销未来授权；
- 使用合成 transport 验证 Rust DeepSeek 协议、严格 JSON/evidence、失败和取消；确定性 Mock 仅用于测试/debug，历史 Mock run 可查看/导出但严格只读。

当前普通用户界面只有在明确点击“生成”或“继续生成”并通过 disclosure/Consent 检查后才能发起 DeepSeek 生成。remembered Consent 不授权页面加载、应用启动或后台重试自动发送。录音文件本地转写已在源码 checkout 的开发预览中提供；实时录音和实时转录仍不可用。翻译模式和会议模式显示“即将推出”且路线状态为 Frozen/Deferred。Rust production Provider 固定为 DeepSeek；Mock 隔离在测试/debug 且不访问网络。PR #25 基线上已有一次单独授权、极短纯合成转录的真实请求通过；这不保证未来账户余额、服务可用性或模型别名行为。

## 3. 系统要求

- Windows 11；
- WebView2；
- 从源码运行时需要 Node.js 22+、npm 10+、Python 3.12.x、uv 0.11.21、Rust stable 和 Visual Studio Build Tools 2022；
- Build Tools 需要 Desktop development with C++、MSVC v143、Windows 10/11 SDK 和 C++ CMake tools。

Windows 11 与 Python 3.12 是项目基线，其他系统或 Python 版本不能替代正式验收。

## 4. 启动应用

从仓库根目录安装锁定依赖并启动桌面开发版：

```powershell
npm ci
.\scripts\setup-transcription-worker.ps1
.\scripts\provision-transcription-model.ps1
npm run tauri dev
```

Worker 安装和模型 provisioning 都是明确的开发者操作。锁定模型保存在仓库外；正常转写离线运行，不会下载或自动更新模型。

完整自动检查使用：

```powershell
npm run check
```

看到原生 MeetingLens 窗口并确认设置页显示本地运行状态后，才算桌面应用实际启动。CI 通过不能替代本机窗口验收。

## 5. 首页和三个模式

首页以用户任务为中心显示三个模式：

- **总结模式**：现在可用；进入转录输入/检查和生成/结果 Minutes 工作区。
- **翻译模式**：界面显示“即将推出”，路线状态为 Frozen/Deferred；不会启动麦克风、实时翻译或外部网络请求。
- **会议模式**：界面显示“即将推出”，独立工作区为 Frozen/Deferred；不会申请音频或屏幕权限。

历史会议入口继续保留。“手动创建会议”属于高级预览流程，视觉优先级低于总结模式，但不会被删除。首页不会要求普通用户理解 Provider、Docling 或内部状态机。

## 6. 总结模式输入

### 粘贴会议记录

1. 在首页选择“总结模式”。
2. 可选填写会议标题；留空时应用根据本地日期和时间生成标题。
3. 粘贴严格 UTF-8 文本。页面按真实 UTF-8 字节显示用量，最大 2 MiB。
4. 选择“保存并检查”。MeetingLens 会原子创建草稿会议和转录；失败不会留下空会议。
5. 成功后进入转录检查页，未提交文本从 React 内存中清除。

### 导入会议转录

1. 可选填写会议标题。
2. 选择“选择 TXT、SRT 或 VTT”。
3. 在 MeetingLens 打开的 Windows 系统文件选择器中选择一个严格 UTF-8 文件，单文件最大 10 MiB。
4. 取消选择是安全的无操作，不会创建空会议。
5. 相同内容在同一会议中会显示“已经添加”，不会保存第二份来源。

源绝对路径不会返回 WebView，源文件不会被复制、修改或删除。TXT、SRT、VTT 使用 Rust 轻量解析器，不经过 Docling、Python Sidecar 或外部 AI。

### 在本地转写录音

当前流程需要源码 checkout 和 debug/`tauri dev` 构建。release build 尚未打包或解析 Worker/模型，会把音频转写报告为不可用。

1. 可在总结模式输入页填写标题，也可以打开现有可编辑会议的转录检查页。
2. 选择音频转写入口，挑选一个 WAV、MP3、M4A 或 MP4 文件；MP4 必须含受支持音轨，视频帧不会被读取。
3. 查看“仅本机”说明。固定运行时使用 faster-whisper、CPU 和 int8；单文件最大 2 GiB、最长四小时。
4. 查看选择、输入验证/复制/探测、模型加载、转写、输出验证和提交进度。运行中可以取消；同一时间只运行一个转写任务。
5. 成功后 MeetingLens 会打开或刷新转录检查页，并选中新建的 `imported_audio` 来源；生成纪要前可在同一工作区修正正文。

取消文件选择是安全的无操作。Rust 在私有临时区复制并验证源文件，应用协议只接受受控 job 目录中的有界 artifact。React 不会通过音频 Command 收到原路径、临时路径、Worker artifact 或转录片段。失败或取消不会留下部分来源，并会清理 Worker 进程树和临时数据。该开发预览使用路径守卫和进程/资源限制，而不是操作系统级文件系统沙箱；一次性 Worker 仍以当前 Windows 用户权限运行。若 Worker 或精确锁定模型缺失，请运行上面的安装/provisioning 脚本；MeetingLens 不会静默下载模型。

### 生成会议纪要

1. 从转录检查页或会议详情打开 Minutes 工作区。
2. 按希望的顺序选择一个或多个转录来源；没有片段的来源不能选择。
3. 选择“详实会议纪要”“简洁会议总结”或“行动项与决定”，再选择中文、英文或双语输出。界面语言与输出语言互相独立。
4. 选择“生成”。若没有配置 API Key，先通过入口前往设置；此时不会创建 run 或网络请求。
5. 检查固定 Provider/host/model、冻结来源/片段/字节数、模板、语言和数据类别，再选择“本次允许”“记住我的选择”或“取消”。关闭或取消新 queued run 的 disclosure 会取消该 run。
6. 查看 queued/running 状态，并在需要时取消。重启恢复的 queued run 绝不会自动开始；请明确选择“继续生成”或“取消任务”。
7. 完成后查看概览、决定、行动项、风险和未决问题。点击 citation 时只读取对应的一条 evidence。

remembered Consent 只会缩短本次由用户明确触发的 Generate 流程。页面打开、应用启动或失败/queued 任务恢复时都不会自动发送，MeetingLens 也不会自动重试 Provider。

### 编辑、复制、保存和整份重新生成

- 选择“编辑”后，只能修改现有分区正文和行动项负责人/截止日期；不能增删或修改条目、状态、evidence 链接、顺序和 Provider 元数据。
- 选择“保存修改”持久化当前 revision。若其他窗口已更新内容，并发保护会要求重新加载。第一次真实修改会把该 revision 标为用户已编辑；同一 revision 不另存 Provider 原稿或独立编辑时间线。
- 使用“复制 Markdown”或“保存 Markdown”前先保存尚未提交的编辑。两项操作使用同一份 Rust canonical Markdown；复制只在明确点击后写入，MeetingLens 不读取剪贴板。
- “保存 Markdown”打开由 Rust 控制的 Windows 原生 Save As。取消是安全的无操作，所选路径不会返回 WebView。
- “整份重新生成”只对 latest terminal DeepSeek revision 可用；它把当前 live transcript 冻结为新 revision 并保留旧 revision，绝不会把编辑后的纪要再次发给 DeepSeek。旧 revision 会引导打开最新版本，历史 Mock 结果严格只读。
- `segment_revised` 提示允许用户明确整份重新生成；`source_deleted` 的结果仍可查看、编辑、复制和保存，但不能在同一 series 重新生成。

Evidence 始终指向原始冻结转录摘录。修改 claim 不会让 AI 或 Rust 重新验证新文字。保存在 MeetingLens 外部的 Markdown 文件和剪贴板内容由用户自行控制，不会随会议删除。

## 7. 检查和修正转录

转录检查页显示会议标题、转录来源、片段数量、可用时间范围、说话人标签和输入状态。

- 单个来源最多可有 20,000 个片段；页面按稳定顺序分页读取，单页不会超过 200 条，不会一次渲染全部内容。
- 有相对时间时显示 `00:12:34`；没有时间戳时只显示段落序号，不会伪造时间。
- 显示顺序是人工设置的说话人名称、来源标签、“未知说话人”。
- 编辑正文或说话人名称时使用 revision 冲突保护；如果内容已在其他位置更新，必须重新加载，应用不会静默覆盖。
- 可以为当前来源中相同的说话人标签统一设置或清空显示名称；原始标签保持不变，名称不会跨来源或会议复用，也不使用 AI 猜测身份。
- 删除来源前需要确认；删除最后一个来源后页面会显示“还没有可用输入”。用户原始文件不会被删除，会议文件区域也不受影响。
- queued/running 的开发 Summary run 正在使用来源时不能删除；run 终态后删除只保留已引用 evidence、清理完整正文快照，并把历史结果标记 stale/来源已删除。

打开 Minutes 后可以从所选来源生成纪要。只有用户明确点击“生成/继续生成”，并且 Rust 的 Key、disclosure、Consent、冻结范围和 run 状态检查全部通过时，应用才可能发起网络请求。

## 8. 高级模拟会议与会议文件

高级“手动创建会议”流程仍可用于验证既有桌面能力：

1. 创建草稿会议并在详情页选择“开始会议”。
2. 查看纯合成、仅内存中的模拟字幕，暂停或继续，并添加临时重点标记。
3. 结束会议后返回详情页。模拟字幕和临时笔记不会保存为正式转录。

会议详情页也可以给 `draft`、`recording` 或 `ready` 会议粘贴/导入转录并打开转录检查页。`processing`、`deleting`、`delete_failed` 等状态会安全禁用修改，后端始终执行最终授权。

参考会议文件继续使用独立的“添加文件”流程：

1. 打开会议详情页，选择“添加文件”。
2. 使用 MeetingLens 打开的系统文件选择器选择允许的文件。
3. 查看验证、复制、保存、重复或失败进度。
4. 导入成功后，在会议文件列表查看安全显示名、类型、大小、时间和状态。

参考文件与转录来源是不同数据类型。参考文件会复制到应用私有存储，源文件不被修改；它们当前不会在界面中打开或解析，也不会发送给 Sidecar、Docling 或 DeepSeek。

## 9. 设置 DeepSeek API Key

1. 打开“设置”。
2. 在 DeepSeek API 区域输入 Key。
3. 选择“保存 API Key”。
4. 确认状态显示“已配置”，输入框保持为空。

Key 保存在当前 Windows 用户的 Credential Manager 中，而不是 SQLite、WebView 存储或仓库。界面只能查询配置状态、写入和删除，不能读回 Key。只有受信任 Rust DeepSeek Provider 可以在已授权请求执行前读取；保存 Key 不会验证它，也不会发送任何内容。

设置页还会显示固定 Provider（`deepseek`）、host（`api.deepseek.com`）、model（`deepseek-v4-pro`），以及是否已“记住我的选择”。选择“撤销外发同意”会让后续发送重新确认，并同时清除 remembered 授权与所有尚未使用的一次性确认；它不能召回已经到达 DeepSeek 的数据。设置中不会提供脱离具体冻结 run 的提前授权按钮，也不能编辑 model 或 base URL。

不要把 Key 粘贴到问题报告、日志、截图、会议记录或版本控制中。已暴露的 Key 应立即撤销。

## 10. 切换界面语言

1. 打开“设置”。
2. 找到“界面语言”。
3. 选择“简体中文”或“English”。
4. 页面文案会立即切换。

应用只在非敏感 WebView 偏好 `meetinglens.uiLocale` 中保存允许的 locale 枚举。重启后会保留用户选择；清除 WebView 存储只重置界面语言，不会删除会议、文件副本或凭据。

界面语言与 Minutes 工作区选择的会议纪要输出语言相互独立。切换 UI 语言不会改变已有 run，也不会静默改变下一次输出选择。

## 11. 查看历史会议

选择左侧“历史记录”，可以查看按创建时间排序的会议。每项显示本地化状态和创建、开始、结束时间。当前数据模型没有独立、可编辑的会议发生日期；“创建时间”不会冒充会议日期。选择“查看详情”可返回会议详情、转录入口和文件列表。

关闭并重新打开应用后，SQLite 中的会议、转录来源、规范化片段、修正 revision、Summary run/result、evidence 和已保存的正文修改仍应存在。崩溃时处于 running 的 Summary 会安全标记为 interrupted；queued Summary 保持 queued，必须明确点击“继续生成”。模拟字幕和临时笔记不属于持久化会议转录。

## 12. 删除会议、转录和文档

转录来源删除：

1. 在转录检查页选择某个来源的“删除来源”。
2. 确认后删除 MeetingLens 中的规范化 segments；用户原始 TXT/SRT/VTT 保持不变。
3. 删除失败时来源保持可见并显示稳定错误，不会提前伪装成功。

文档删除：

1. 在会议详情中选择某个文档的“删除”。
2. 确认只删除 MeetingLens 本地副本；源文件保持不变。
3. 删除失败时使用“重试删除”。

会议删除：

1. 只有“待准备”或“可继续处理”的会议可以请求删除。
2. 会议进行中必须先结束。
3. 确认删除后，MeetingLens 会协调数据库记录与受控文件副本。
4. “正在删除”或“需要处理”表示恢复流程尚未完成，不应重复创建替代数据。

删除是不可撤销操作。执行前确认选中的会议和文件正确。删除会议会移除由 MeetingLens 管理的本地 run/result/evidence，但不能召回已经送达 DeepSeek 的数据、清空系统剪贴板，或删除用户保存在 MeetingLens 外部的 Markdown 文件。

## 13. 数据存储和隐私

- 会议元数据保存在应用私有 SQLite 数据库中；
- 已提交的转录来源和规范化片段保存在同一私有数据库中；
- schema v4 在本地保存 Summary runs 与不可变转录快照；持久约束仍为最多 20,000 个 final segments 和 10 MiB UTF-8 正文，production DeepSeek policy 限制为 1 MiB，超限会在网络前失败。显式来源删除会清理终态完整正文快照，只保留已引用 evidence；
- schema v5 最多保存一行最小 DeepSeek remembered 偏好：provider、policy version/fingerprint 与同意/更新时间，不保存正文、run ID、Prompt、响应、路径或 Key；一次性同意只在 Rust 内存中保留最多 10 分钟并单次消费；
- schema v6 只在既有 transcript 与冻结 Summary 来源约束中加入 `imported_audio`；既有 transcript 表保存规范化正文/时间，不保存原录音或音频路径。Minutes 编辑仍更新既有 revision 的五区 `content_json`，Rust 派生的 `contentVersion` 仍不持久化；
- 未提交的粘贴文本只短暂存在于 React 内存，保存、取消或离开页面后清除；
- 转录正文、说话人和源路径不会写入 localStorage、sessionStorage、IndexedDB 或 console；
- 导入文件副本位于应用私有存储，React 不取得绝对路径；
- DeepSeek Key 位于 Windows Credential Manager；
- 界面语言是唯一允许写入 WebView 偏好存储的应用设置；
- 当前预览版不会实时采集音频或读取屏幕；用户选择的录音只在私有临时区由本地 Worker 处理，终态后清理，规范化 `imported_audio` 转录保存在 SQLite。面向用户的“生成/继续生成”只能请求 Rust Provider 把明确授权的冻结转录范围发送到固定 DeepSeek endpoint；转写 Worker 不接收 DeepSeek Key，也不能发起该请求；
- canonical Markdown 由 Rust 生成；原生 Save As 不把目标路径写入 SQLite 或返回 React，剪贴板和外部 `.md` 副本不受 MeetingLens 生命周期控制；
- 测试和问题报告只能使用合成数据。

详细边界见 [SECURITY.md](../SECURITY.md)。

## 14. 当前尚未支持的功能

以下 Personal Edition 近期计划尚未实现：

- 简化 Docling 产品编排与 document evidence；
- 可选的用户明确选择窗口上下文。

以下能力 Frozen/Deferred，不属于当前 Personal Edition 近期路线：

- 实时音频录制、WASAPI 和实时转写；
- 实时说话人分离和身份增强；
- 实时翻译、字幕浮窗、checkpoint 和自动话题判断；
- 独立翻译/会议模式工作区；
- 章节/段落级重新生成、编辑历史/恢复、富 HTML 结果、多人冲突合并、Structured Visuals、视频帧分析/通用视频处理、商业分发以及 DOCX/PDF 导出；
- 正式 PDF 用户手册和成品截图流程。

## 15. 常见问题

### 为什么已经保存 DeepSeek Key，却不能生成纪要？

保存 Key 只配置 SecretStore，不会验证账户或授权请求。请打开 Minutes，选择至少一个非空转录来源、模板和语言，再明确点击“生成”。Rust 仍会检查 meeting 资格、冻结范围、当前 Consent 和安全 run 状态。界面只显示稳定安全错误。PR #25 基线上已有一次单独授权的合成请求通过，但当前 Key/账户/服务状态只有在你明确授权的请求中才会检查。

### 为什么重启后 queued 结果要求我继续？

MeetingLens 不会把 remembered Consent 当作启动或后台发送授权。从 SQLite 恢复的 queued run 会保持 queued，直到用户明确选择“继续生成”或“取消任务”。异常退出前已经进入 running 的 run 会标记 interrupted，不会自动重放。

### 导入转录会复制或删除原文件吗？

不会。Rust 只在选择期间读取源文件并保存规范化片段，绝对路径不进入 WebView 或 SQLite。删除转录来源不会影响用户原文件。

### 音频转写会上传或长期保存我的录音吗？

不会。所选文件只会复制到应用私有临时区，交给本地一次性 Worker 处理；终态、取消或失败后都会清理临时副本，只提交规范化 `imported_audio` 来源和片段。模型必须由用户明确 provision；运行时不会下载模型，也不会把音频发送给 DeepSeek。

### 导入文件后为什么没有解析结果？

当前 UI 只完成安全复制和生命周期管理。离线 Docling Worker 已完成开发基础，但尚未连接数据库、命令或界面。

### 切换语言会翻译会议标题或文件名吗？

不会。标题、文件名和未来转录均是用户数据，不会被当作翻译 key 或自动翻译。

### 清除浏览器或 WebView 存储会删除会议吗？

不会，只会重置非敏感界面语言偏好。会议和文件由本地数据库与私有文件存储管理。

## 16. 故障排查

- 应用无法启动：确认 C++ Build Tools、WebView2、Rust、Node.js 和 Python 3.12 可用，再运行 `npm run check`。
- 本地服务不可用：关闭残留开发进程后重新启动应用；不要自行降低认证或安全检查。
- 数据库不可用：停止写入操作并保留错误代码，不要直接修改或删除数据库文件。
- 文件导入失败：检查格式、大小、配额和文件完整性；不要通过改扩展名绕过校验。
- 转录导入失败：确认文件是严格 UTF-8 TXT/SRT/VTT、大小不超过 10 MiB，并且 cue/时间格式完整；不要把路径或转录正文放入问题报告。
- 音频转写不可用：运行锁定的 Worker 安装与显式模型 provisioning 脚本，再重启应用；不要把模型复制进仓库或削弱 manifest/revision 校验。
- 音频转写失败：使用含有效音轨且不超过 2 GiB/四小时的 WAV、MP3、M4A 或 MP4；只改扩展名不能绕过媒体校验。等现有任务进入终态后再重试；问题报告中不要附文件名、路径、音频、转录正文或 Worker stderr。
- 修正冲突：重新加载当前片段再编辑，不要尝试绕过 revision 保护。
- 纪要生成被阻止：配置 Key、选择非空来源、检查 disclosure，并明确点击“生成/继续生成”；不要修改 Provider URL 或绕过 Consent 来重试。
- 纪要编辑冲突：重新加载当前 revision 后再应用修改；过期 `contentVersion` 会被主动拒绝。
- 剪贴板不可用：改用“保存 Markdown”。MeetingLens 不读取剪贴板，也不使用旧式复制 fallback。
- Markdown 保存失败：选择可写的本地 `.md` 目标，不能是目录、symlink 或 reparse point 逃逸；问题报告中不要附上路径或纪要正文。
- 安全存储不可用：确认在受支持的 Windows 用户会话中运行；不要改用明文文件。
- 报告问题时提供操作步骤、应用版本和稳定错误代码，不要附上 Key、会议内容、录音、数据库、日志、私有路径或上传文件。

## 17. 后续计划

Personal Edition 已完成和下一阶段交付路径是：

1. `feat/transcript-data-core`（PR #18 已合并）：schema v3、转录来源和片段、粘贴/TXT/SRT/VTT 本地解析；
2. `feat/transcript-command-api`（PR #19 已合并）：安全转录 Commands、Rust 原生选择器和原子 meeting + transcript 创建；
3. `feat/summary-mode-input-ui`（PR #20 已合并）：总结模式入口、粘贴、原生导入、分页预览和基础说话人/正文修正；
4. `feat/summary-run-core`（PR #21 已合并）：schema v4、完整正文快照、series/revision、evidence allowlist、恢复和 Rust zero-network Mock；
5. `docs/personal-edition-descope`（PR #23 已合并）：Personal Edition、Frozen/Deferred 范围和开发流程重定基线；
6. `feat/summary-provider-deepseek`（PR #24 已合并）：schema v5 最小偏好、Rust DeepSeek/SecretStore、严格 JSON/evidence、本次同意/记住/撤销、取消和 fail closed；不建立通用 Consent Data Core；
7. `feat/summary-mode-minutes`（PR #25 已合并，日常可用里程碑）：正式生成/Consent/状态、中文/英文/双语五区结果和精确 evidence、受保护正文编辑、canonical Markdown 复制/原生保存，以及保留旧 revision 的整份重新生成；另有一次单独授权的纯合成真实 smoke 通过；
8. `feat/audio-file-transcription`（当前功能交付，预计 PR #26）：schema v6、显式 Worker/模型 provisioning，以及本地一次性 CPU-int8 Worker 对 WAV/MP3/M4A/含音轨 MP4 的批量转写；不做实时音频、WASAPI 或实时 ASR；
9. `feat/document-parsing-orchestration`（仅在当前功能经审查并合并后的下一阶段，预计 PR #27）：简化接入已有 Docling Worker、document chunks/evidence、失败自动重试一次，再失败提示重新导入；
10. `feat/window-context`（可选，预计 PR #28 或更后）：仅在确认真实需要后读取用户明确选择的具体窗口。

当前 `feat/audio-file-transcription` 在已经完成的日常 Summary 闭环上增加高价值本地录音输入。实时采集/ASR、实时翻译、说话人分离、checkpoint、独立翻译/会议工作区、Structured Visuals、商业分发及 DOCX/PDF 保持 Frozen/Deferred。最小 Consent 只适用于单人自用版，不是企业合规审计系统；未确认时必须 fail closed。未来阶段和分支以 [DEVELOPMENT_PLAN.md](../DEVELOPMENT_PLAN.md) 为准，预期 PR 编号不是权威标识。
