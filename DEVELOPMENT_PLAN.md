# MeetingLens 开发计划

## 1. 分支策略

`main` 始终保持可构建、可演示。每个可验收任务在独立分支完成，通过 Pull Request 和 CI 后由用户决定是否合并。

```text
main
├── docs/project-specification
├── feat/app-scaffold
├── feat/audio-capture
├── feat/document-processing
├── feat/deepseek-summary
├── feat/realtime-translation
├── feat/speaker-diarization
└── feat/screen-context
```

不得自动合并到 `main`，不得 force-push，不得绕过分支保护。较大阶段继续拆分小 PR，避免长期巨型分支。

## 2. 第一轮：规格与可运行基线（当前工作）

分支：`docs/project-specification`

交付：

- `PRODUCT_SPEC.md`、`ARCHITECTURE.md`、`DATA_MODEL.md`、`SECURITY.md`、`DEVELOPMENT_PLAN.md`、`AGENTS.md`
- Tauri 2 + React + TypeScript + Vite 最小脚手架
- Python 3.12 dependency-free runtime sidecar `/health`
- React → Tauri command → Rust HTTP client → Python `/health` 的真实验证链路
- 前端、Python 和 Rust 单元测试；ESLint、Ruff、Prettier、类型检查和构建
- GitHub Actions（Linux 前端/Python 3.12，Windows Tauri core）
- Windows 11 开发说明、精确版本依赖和 lockfile

验收：所有可运行检查通过；CI 通过；无真实凭据或会议数据；分支推送并更新 PR，但不合并。

停止线：完成以上内容后停止，不在本分支实现录音、数据库业务表、文档解析、DeepSeek 调用、模型下载或屏幕采集。

## 3. Phase 1：MVP

Phase 1 应拆成下列分支顺序；每个分支只依赖已合并的前置能力。

### 3.1 `feat/app-scaffold`

- 路由、首页/新建会议/会议中/结果/设置页面骨架
- Rust command 类型契约和前端 client
- SQLite migration、会议 CRUD 和应用私有数据目录
- Sidecar supervisor、随机端口、会话 token、崩溃恢复和 path guard
- 本地安全日志和合成数据测试

### 3.2 `feat/audio-capture`

- Windows 11 系统音频与麦克风设备枚举
- 明确开始/停止、持续录音指示、分块写入和异常恢复
- 实时转录、时间戳片段持久化和人工修正入口
- 同意事件与会议完整删除

验收重点：不得后台静默录音；只使用合成音频测试；录音不进入 Git 或日志。

### 3.3 `feat/document-processing`

- PDF/DOCX/PPTX/XLSX/TXT/Markdown/HTML/CSV/图片选择
- 签名/MIME/大小校验、artifact 登记、哈希和受控路径
- Docling worker、资源限制、进度、失败恢复和来源页/段落映射

采用门槛：固定 Docling 及其 worker 依赖，验证 Windows/Python 3.12、模型下载、冷启动、内存和恶意文件行为。

### 3.4 `feat/deepseek-summary`

- `SummaryProvider` 与 fake provider
- Windows Credential Manager；开发 `.env` 仅作本地回退
- 发送预览、同意、裁剪、脱敏、超时、限流、取消和重试
- `deepseek-v4-pro` 分阶段摘要：事实 → 主题 → 决策/行动项/未决问题 → 合并
- Markdown/DOCX 导出、来源映射、设置页和失败恢复

验收重点：无 key、无同意或范围为空时 fail closed；测试不调用真实付费 API。

Phase 1 不包含自动窗口检测、屏幕 OCR、实时名字识别、实时翻译、PDF 导出、托盘、自动更新或会议平台 API。

## 4. Phase 2：说话人识别与实时翻译

### 4.1 `feat/speaker-diarization`

- 独立 Diart worker 做匿名实时 diarization
- 用户可选短 voice sample、人工命名、重录、删除和纠错
- 独立 WhisperX worker 做会后时间对齐/说话人校正
- 所有自动标签可回滚，不默认做跨会议真实身份识别

硬约束：Diart 与 WhisperX 当前 NumPy/pyannote 依赖冲突，必须使用独立环境、进程和 lockfile。

### 4.2 `feat/realtime-translation`

- 稳定流式片段协议、顺序、去重和取消
- `deepseek-v4-flash` 中英实时翻译
- 原文/译文对照 UI、语言切换、延迟和失败处理

## 5. Phase 3：屏幕上下文

分支：`feat/screen-context`

- 用户显式选择目标窗口，窗口身份变化时 fail closed
- accessibility tree 优先、本地 OCR 回退、敏感内容遮蔽
- 关键事件和结构化 screen event，不做全量 24/7 捕获
- 截图仅作短期缓存，提取后默认删除；保存前预览和确认

若采用 Screenpipe，开始实现前必须单独复审商业授权、版本稳定性、Windows 资源占用、本地 API 权限和默认采集范围。

## 6. Phase 4：产品化

- 托盘、自动更新、崩溃恢复、诊断与隐私设置
- PDF 导出、安装包签名、SBOM、性能与长期稳定性
- 数据保留策略、删除验证和依赖/模型升级流程

## 7. 测试策略

| 层级             | 工具/方式                      | 覆盖重点                           |
| ---------------- | ------------------------------ | ---------------------------------- |
| React unit       | Vitest + Testing Library       | 状态、可访问性、用户操作           |
| Type/lint/format | TypeScript + ESLint + Prettier | 边界类型、一致格式、常见错误       |
| Python           | unittest + Ruff                | API、任务状态、格式、错误与清理    |
| Rust             | Cargo fmt/test/check           | command 校验、路径边界、超时、迁移 |
| Contract         | JSON fixtures                  | UI/Rust/sidecar 版本兼容           |
| E2E              | Tauri WebDriver（后续）        | Windows 11 关键桌面流程            |
| Security         | secret/path/upload tests       | fail-closed、路径逃逸、敏感日志    |

测试数据只使用合成会议、占位参与者和生成音频，不使用真实会议文件。

## 8. CI 与发布门槛

每个 PR 至少执行：

- `npm ci`
- `npm run format:check`
- `npm run lint`
- `npm run typecheck`
- `npm run test`
- `npm run test:sidecar`
- `npm run build`
- `cargo fmt --check`、`cargo test --locked` 和 `cargo check --locked`

required checks 未通过时不得合并。生产发布后续加入 Windows Tauri bundle、签名、SBOM、依赖审计和安装包冒烟测试。

## 9. Definition of Done

1. 验收标准有可观察结果。
2. 正常、失败和取消路径有测试或明确人工验证。
3. 文档、类型和迁移与实现同步。
4. 没有真实凭据、会议数据或被忽略产物进入 staged files。
5. diff 已人工检查，提交单一且清晰。
6. 分支已 push，PR 已创建/更新，CI 全部通过。
7. 已报告限制，分支未自动合并到 `main`。

## 10. 当前风险与决策点

- 本机已有 Rust 工具链，但缺少 MSVC C++ Build Tools / `link.exe`；本地可运行 `cargo fmt`，编译与测试由 Windows CI 独立验证。
- Diart `0.9.2` 与 WhisperX `3.8.6` 不能共享 Python 环境，必须隔离 worker。
- Screenpipe 上游迭代快且商业使用涉及许可，采用前需要用户批准并锁定具体发行版。
- faster-whisper GPU 路径依赖 CUDA 12/cuDNN 9；MVP 先建立 CPU 可用回退并测量性能。
- SQLite 首版不承诺静态加密；如需抵御本机磁盘读取，再评估 SQLCipher 与密钥管理。
- DeepSeek 的价格、生产数据条款和模型可用性在功能分支开始前重新核验。
