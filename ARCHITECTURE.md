# MeetingLens 架构设计

## 1. 架构目标

MeetingLens 采用“轻量桌面壳 + 本地处理 sidecar + 本地数据库 + 可选外部 AI”的结构。Tauri/Rust 控制系统权限和进程生命周期，React 负责交互，Python 承载语音、文档和 NLP 处理，SQLite 保存结构化元数据。

核心目标是明确权限边界、允许处理模块独立替换，并让原始会议数据默认不离开本机。

## 2. 系统上下文

```mermaid
flowchart LR
    U["用户"] --> UI["React WebView"]
    UI <--> CORE["Tauri / Rust Core"]
    CORE <--> SIDE["Python Sidecar"]
    CORE <--> DB["Local SQLite"]
    CORE <--> FS["Private App Data"]
    SIDE <--> FS
    SIDE -. "用户确认后的最小文本" .-> DS["DeepSeek API"]
```

## 3. 组件职责

### 3.1 React WebView

- 显示会议、录音状态、处理进度、转录、翻译和摘要。
- 只通过定义明确的 Tauri commands 访问系统能力。
- 不直接读取 `.env`、SQLite、任意文件路径或操作系统凭据。
- 不直接启动 sidecar，也不持有长期 API key。

### 3.2 Tauri / Rust Core

- 作为 WebView 与本机资源之间的权限边界。
- 管理窗口、文件选择、音频设备权限和应用生命周期。
- 启动/停止已打包 sidecar，生成每次启动的端口与会话令牌。
- 对 IPC 输入做类型、路径和大小校验。
- 负责数据库迁移、原子写入和删除协调。

### 3.3 Python Sidecar

- 提供版本化 loopback API。
- 承载转录、文档提取、翻译、说话人分离和摘要预处理。
- 任务处理器按接口解耦，单个模型失败不影响其他模块。
- 首轮只实现无第三方依赖的 `/health`，后续使用 PyInstaller 生成平台 sidecar 二进制。

### 3.4 SQLite 与本地文件

- SQLite 保存元数据、片段、任务状态、摘要和同意事件。
- 大型音频与上传文件保存于应用私有数据目录，数据库只保存受控相对路径和哈希。
- 凭据不进入 SQLite；开发阶段使用已忽略的 `.env`，发布阶段使用系统凭据库。

### 3.5 外部 AI 适配器

- DeepSeek 是首个摘要适配器，但业务层依赖统一 `SummaryProvider` 接口。
- 适配器只接收用户确认的文本范围，并记录提供商、模型和请求时间，不保存密钥。
- 外部调用失败时保留本地数据和可重试任务，不阻塞会议浏览。

## 4. 信任边界

| 边界 | 高信任侧 | 低信任侧 | 主要控制 |
| --- | --- | --- | --- |
| WebView → Rust | Rust core | UI 输入 | Tauri capabilities、command allowlist、结构校验 |
| Rust → Sidecar | Rust supervisor | 本地 HTTP 进程 | loopback、随机端口、临时 bearer token、超时 |
| Sidecar → 文件 | 应用私有目录 | 上传内容 | 规范化路径、MIME/大小限制、沙箱式工作目录 |
| 本地 → DeepSeek | 本地数据 | 外部服务 | 明示同意、最小文本、脱敏、TLS、超时 |

## 5. Sidecar API

### 5.1 首轮健康检查

`GET /health`

```json
{
  "status": "ok",
  "service": "meetinglens-sidecar",
  "version": "0.1.0",
  "api_version": "v1"
}
```

约束：

- 默认绑定 `127.0.0.1`。
- 返回值不包含主机名、路径、环境变量、模型列表或凭据信息。
- 非法路径返回 JSON 404。
- 后续除 `/health` 外的端点都要求每次启动生成的 bearer token。

### 5.2 规划端点

- `POST /v1/jobs/transcribe`
- `POST /v1/jobs/translate`
- `POST /v1/jobs/diarize`
- `POST /v1/jobs/extract-document`
- `POST /v1/jobs/summarize`
- `GET /v1/jobs/{job_id}`
- `DELETE /v1/jobs/{job_id}`

任务请求只传受控 artifact ID 或应用私有目录中的相对引用，不接受任意绝对路径。

## 6. 关键数据流

### 6.1 录音

```mermaid
sequenceDiagram
    participant User
    participant UI
    participant Rust
    participant FileStore
    participant Sidecar
    User->>UI: 明确开始录音
    UI->>Rust: start_recording(meeting_id, device_id)
    Rust->>FileStore: 写入分块音频
    Rust-->>UI: 状态与时长事件
    User->>UI: 停止录音
    UI->>Rust: stop_recording()
    Rust->>Sidecar: 创建转录任务
```

### 6.2 外部摘要

1. 用户在 UI 选择转录范围并查看发送预览。
2. Rust 读取被允许的片段并执行基础脱敏。
3. Sidecar 通过 provider adapter 调用 DeepSeek。
4. 返回结果连同模型、提示模板版本和来源片段 ID 写入 SQLite。
5. UI 将 AI 输出标记为可编辑草稿，而非事实来源。

## 7. 仓库结构

```text
MeetingLens/
├── src/                       # React + TypeScript UI
├── src-tauri/                 # Tauri/Rust core 与 capabilities
├── sidecar/
│   ├── meetinglens_sidecar/   # Python 服务
│   └── tests/                 # Sidecar 自动化测试
├── .github/workflows/         # CI
├── PRODUCT_SPEC.md
├── ARCHITECTURE.md
├── DATA_MODEL.md
├── SECURITY.md
└── DEVELOPMENT_PLAN.md
```

## 8. 进程与打包策略

1. 开发阶段可使用 `python -m sidecar.meetinglens_sidecar.server` 独立启动服务。
2. 发布阶段使用锁定版本的 PyInstaller 构建平台二进制。
3. 二进制按 Tauri target triple 命名，并配置到 `bundle.externalBin`。
4. Rust 使用 sidecar API 启动进程，不允许 WebView 获得任意 shell 执行能力。
5. 应用退出、崩溃恢复和更新时，Rust 负责回收孤儿 sidecar。

首轮不提前写入 `externalBin`，因为仓库尚无可验证的平台二进制；这样避免生成一个无法打包的虚假配置。

## 9. 可观测性

- 结构化日志只记录事件类型、任务 ID、耗时、状态码和匿名错误分类。
- 默认不记录转录正文、提示词正文、文件名、参会者、密钥或外部响应全文。
- UI 展示面向用户的错误；开发日志使用 request/correlation ID 串联 Rust 和 sidecar。
- 健康指标包括队列长度、任务耗时、重试次数和 sidecar 重启次数。

## 10. 架构决策

- **Tauri 2 + React + TypeScript**：小型桌面壳、明确 IPC 边界、前端生态成熟。
- **Python sidecar**：便于接入语音和文档处理生态，与 UI/Rust 生命周期解耦。
- **SQLite**：单机事务和迁移简单，适合本地优先元数据。
- **本地 HTTP + 会话令牌**：便于 Python 服务化测试，同时限制未授权本机进程访问。
- **Provider adapter**：避免业务逻辑与 DeepSeek 单点绑定。

## 11. 参考

- [Tauri 2 创建项目](https://v2.tauri.app/start/create-project/)
- [Tauri sidecar 指南](https://v2.tauri.app/develop/sidecar/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri security](https://v2.tauri.app/security/)
