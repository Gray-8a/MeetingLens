# MeetingLens 开发计划

## 1. 工作流与阶段规则

`main` 始终保持可构建、可演示。Codex 可以自主开发、测试、修复 CI、提交、推送和创建/更新 PR，但不得启用 auto-merge，也不得自动合并；最终 Merge 由用户决定。

每个实现 PR 必须：

1. 只覆盖一个可验收范围，并使用本计划指定的分支名。
2. 有独立的正常、失败和安全验收条件。
3. 通过 formatter、lint、类型检查、测试、构建和敏感信息门禁。
4. 涉及 Windows 桌面行为时，由用户本机实际运行验收，不能只凭 CI 声称完成。
5. 合并后再从最新 `main` 创建下一个分支，不堆叠未审查的大型 PR。

MVP 1A-2 实现前已使用 `docs/define-meeting-lifecycle` 固定会议状态转换、删除演进策略和 PR 边界。Provider 与安全基础继续拆成 1A-3A、1A-3B-1 和 1A-3B-2 三个顺序 PR，避免把能力契约、Sidecar 会话认证与操作系统凭据存储放入同一审查单元。

## 2. 统一路线图

| 阶段        | 目标                                           | 计划分支/PR                                             | 状态         |
| ----------- | ---------------------------------------------- | ------------------------------------------------------- | ------------ |
| Phase 0     | 规格与技术基线                                 | `docs/project-specification` / PR #1                    | 已完成并合并 |
| MVP 1A-1    | SQLite、迁移、会议 CRUD、删除语义和持久化      | `feat/meeting-data-core` / PR #3                        | 已完成并合并 |
| MVP 1A-2-1  | Meeting Tauri Command API 与 TypeScript 客户端 | `feat/meeting-command-api` / PR #6                      | 已完成并合并 |
| MVP 1A-2-2  | 桌面 UI、模拟转写和完整模拟会议流程            | `feat/meeting-ui-mock` / PR #7                          | 已完成并合并 |
| MVP 1A-3A   | Provider 契约、Registry 和内存 Mock            | `feat/provider-contracts-mocks` / PR #8                 | 已完成并合并 |
| MVP 1A-3B-1 | Sidecar 会话认证、Loopback 和基础日志脱敏      | `feat/local-sidecar-auth` / PR #9                       | 当前阶段     |
| MVP 1A-3B-2 | SecretStore、Windows Credential Manager        | `feat/secret-store` / PR #10                            | 待开始       |
| MVP 1B-1    | 文档上传、验证、隔离和 Docling 解析            | `feat/document-processing`                              | 待开始       |
| MVP 1B-2    | DeepSeek 分阶段总结、证据引用和纪要导出        | `feat/deepseek-summary`                                 | 待开始       |
| MVP 1C      | Windows 音频、实时转写、sidecar 生命周期       | 见 §8 的三个小 PR                                       | 待开始       |
| MVP 2       | 实时翻译与说话人                               | `feat/realtime-translation`、`feat/speaker-diarization` | 待开始       |
| MVP 3       | 会议窗口、OCR 和屏幕上下文                     | `feat/window-context`、`feat/screen-ocr`                | 待开始       |
| MVP 4       | 安装包与产品化                                 | `chore/windows-packaging`、`feat/product-hardening`     | 待开始       |

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

分支：`feat/secret-store`

范围：

- Windows Credential Manager `SecretStore` 抽象；测试不得读取真实凭据
- 凭据安全写入、读取状态、替换和删除接口
- 跨层 allowlist 安全日志基础：事件、匿名任务 ID、状态、耗时、错误码

不得调用真实付费 API，也不得实现 Docling、DeepSeek、音频或屏幕采集。

验收：无凭据和空范围全部失败关闭；WebView/SQLite/日志不出现密钥；用户本机验证凭据写入/读取/替换/删除状态但不显示值。

## 7. MVP 1B：文档与纪要（两个独立 PR）

### 7.1 MVP 1B-1：文档处理

分支：`feat/document-processing`

- 上传类型、签名/MIME/大小校验和隔离目录
- artifact/document/document_chunk 持久化
- 独立 Docling worker、固定依赖、模型下载策略和资源上限
- 页码/章节来源映射、取消、失败恢复和完整删除
- 畸形文件、路径逃逸、压缩炸弹和不支持格式测试

验收：只使用合成文档；Windows/Python 3.12 本机验证解析、内存/冷启动和删除；本 PR 不接入任何总结模型。

### 7.2 MVP 1B-2：DeepSeek 会议纪要

分支：`feat/deepseek-summary`

- 在 1B-1 的稳定结构化文档管线上接入 DeepSeek adapter
- 分段事实提取 → 主题归并 → 决策/行动项/未决问题 → 最终合并
- 时间戳、文档块和 evidence item 引用
- 用户发送预览、明确同意、裁剪、脱敏、超时、重试、取消
- Markdown 和 DOCX 纪要导出

验收：默认使用 Mock Provider；真实 API 调用必须由用户单独授权；无 key/无同意/空范围 fail closed；长会议不得一次发送全部数据；Docling 与 DeepSeek 不在同一 PR 引入。

## 8. MVP 1C：真实音频、转写与 sidecar 生命周期

该阶段风险较高，固定拆为三个顺序 PR：

1. `feat/sidecar-supervisor`：sidecar 打包、随机端口接线、健康恢复、崩溃重启、退出清理、孤儿进程回收和生命周期安全日志。
2. `feat/audio-capture`：WASAPI 系统音频与麦克风、设备枚举、明确开始/停止、持续录音指示、分块文件和异常恢复。
3. `feat/realtime-transcription`：实时 ASR、片段顺序/去重、时间戳持久化、CPU 回退、取消与会后校正入口。

验收：每个 PR 使用合成音频自动测试，并在 Windows 本机验证真实设备/权限/窗口/进程行为；应用退出或崩溃后不得留下 sidecar 或文件句柄；日志不得包含音频、转录正文、设备 ID 或绝对用户路径。

## 9. MVP 2：实时翻译与说话人

- `feat/realtime-translation`：流式片段协议、`deepseek-v4-flash` 中英翻译、原文/译文对照、延迟和失败处理。
- `feat/speaker-diarization`：匿名实时 diarization、可选短 voice sample、人工命名/纠错，以及隔离的 WhisperX 会后校正 worker。

Diart 与 WhisperX 必须保持独立环境、进程和 lockfile；不得默认做跨会议真实身份识别。

## 10. MVP 3：会议窗口、OCR 与屏幕上下文

- `feat/window-context`：用户显式选择目标窗口、窗口身份验证、暂停/撤销和 accessibility tree 提取。
- `feat/screen-ocr`：本地 OCR 回退、敏感内容遮蔽、关键事件/evidence 生成和截图短期缓存删除。

Screenpipe 只能是可替换的可选 Capture Provider，不得成为核心强依赖。采用前必须获得用户批准，并复审商业许可、默认采集范围、本地 API 暴露和 Windows 资源占用。

## 11. MVP 4：安装包与产品化

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

PR #8 已合并，当前只能开始 `feat/local-sidecar-auth`。该 PR 仅实现 MVP 1A-3B-1 的 Sidecar 会话认证、Loopback 边界、最小启动所有权、稳定错误和基础日志脱敏；不得提前实现 SecretStore、API Key、真实 Provider、文档、DeepSeek 或音频。该 PR 由用户决定合并后，才能从最新 `main` 创建 `feat/secret-store`。
