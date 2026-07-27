# MeetingLens 开发计划

## 1. 分支策略

`main` 始终保持可构建、可演示。每个阶段在独立分支完成，通过 Pull Request 审查和 CI 后由用户决定是否合并。

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

不得自动合并到 `main`，不得 force-push。若某项工作需要跨多个阶段，优先拆成小 PR，而不是长期巨型分支。

## 2. 阶段 0：规格与可运行基线

分支：`docs/project-specification`

交付：

- `PRODUCT_SPEC.md`
- `ARCHITECTURE.md`
- `DATA_MODEL.md`
- `SECURITY.md`
- `DEVELOPMENT_PLAN.md`
- `AGENTS.md`
- Tauri 2 + React + TypeScript 最小脚手架
- Python sidecar `/health` 与自动化测试
- 基础 GitHub CI

验收：前端 lint、类型检查、测试和 build 通过；Python 测试通过；Rust 工具链可用时 fmt/check 通过；无真实凭据或会议数据；分支推送并创建 PR，但不合并。

## 3. 阶段 1：应用骨架

分支：`feat/app-scaffold`

目标：把最小模板扩展为可持续开发的应用壳，而不是重复生成脚手架。

- 路由、应用布局、错误边界和状态管理
- Rust command 类型契约和前端 client
- SQLite migration 基础设施
- Sidecar supervisor、随机端口、会话 token 和健康状态
- 应用数据目录与 artifact path guard
- 合成数据驱动的会议列表/详情

依赖：阶段 0 的架构、安全边界和数据模型获确认。

## 4. 阶段 2：音频采集

分支：`feat/audio-capture`

- 枚举输入设备与权限状态
- 显式开始/停止、持续录音指示和异常恢复
- 分块写入应用私有目录
- 设备断开、磁盘不足和应用崩溃处理
- 同意事件与完整删除

验收重点：不得后台静默录音；使用合成音频测试；录音文件不进入 Git 或日志。

## 5. 阶段 3：文档处理

分支：`feat/document-processing`

- 文件选择、签名/MIME/大小校验
- artifact 登记、哈希与受控路径
- PDF/DOCX/TXT 提取适配器
- 资源限制、任务进度和失败恢复
- 提取结果与来源页/段落映射

依赖：artifact path guard 和 processing job 模型。

## 6. 阶段 4：DeepSeek 摘要

分支：`feat/deepseek-summary`

- `SummaryProvider` 接口与 fake provider
- OS 凭据库接入；开发 `.env` 只作本地回退
- 用户发送预览、同意、裁剪和脱敏
- DeepSeek adapter、超时、限流、重试和取消
- 结构化摘要、行动项和来源片段映射

验收重点：无 key/无同意时 fail closed；测试默认不调用真实 API；引入任何新增付费能力前获得用户批准。

## 7. 阶段 5：实时翻译

分支：`feat/realtime-translation`

- 流式片段协议、顺序与去重
- 原文/译文对照 UI
- 延迟、失败和语言切换处理
- 本地/外部 provider 接口

依赖：稳定转录片段模型；如依赖新的外部服务，需要先批准成本与数据范围。

## 8. 阶段 6：说话人分离

分支：`feat/speaker-diarization`

- 匿名 speaker label
- 时间片段合并和冲突处理
- 用户重命名、纠错与回滚
- 不默认执行真实身份/声纹识别

验收重点：说话人错误不改变原始音频；所有自动标签可被人工修订。

## 9. 阶段 7：屏幕上下文

分支：`feat/screen-context`

- 明确开启、窗口/区域选择和持续状态提示
- 本地 OCR、敏感模式遮蔽和保存前预览
- 默认只保存确认后的文字，不保留截图
- 随时暂停、撤销和完整删除

该阶段隐私风险最高，开始实现前需要单独复审 `SECURITY.md` 和平台权限行为。

## 10. 测试策略

| 层级 | 工具/方式 | 覆盖重点 |
| --- | --- | --- |
| React unit | Vitest + Testing Library | 状态、可访问性、用户操作 |
| Type/lint | TypeScript + ESLint | 边界类型、未使用代码、常见错误 |
| Python unit | `unittest` | API、任务状态、错误与清理 |
| Rust unit/integration | Cargo | command 校验、路径边界、迁移 |
| Contract | JSON fixtures | UI/Rust/sidecar 版本兼容 |
| E2E | Tauri WebDriver（后续） | 关键桌面流程 |
| Security | secret/path/upload tests | fail-closed、路径逃逸、敏感日志 |

测试数据全部使用合成会议、占位参与者和生成音频，不使用真实会议文件。

## 11. CI 与发布门槛

每个 PR 至少执行：

- `npm ci`
- `npm run lint`
- `npm run typecheck`
- `npm run test`
- `npm run build`
- `python -m unittest discover -s sidecar/tests -v`
- `python -m compileall -q sidecar`
- `cargo fmt --check` 和 `cargo check --locked`

required checks 未通过时不得合并。发布阶段再加入多平台 Tauri build、签名、SBOM、依赖审计和安装包冒烟测试。

## 12. Definition of Done

一项任务只有在以下条件全部满足时才算完成：

1. 验收标准有可观察结果。
2. 正常、失败和取消路径均有测试或明确人工验证。
3. 文档、类型和迁移与实现同步。
4. 没有真实凭据、会议数据或被忽略产物进入 staged files。
5. diff 已人工检查，提交单一且清晰。
6. 分支已 push，PR 已创建/更新，CI 全部通过。
7. 已报告已知限制；分支未自动合并到 `main`。

## 13. 当前风险与决策点

- 当前开发机缺少 Rust 工具链，Tauri/Rust 本地检查需安装 prerequisites 后执行；CI 提供独立验证。
- 实时转录/翻译和 diarization 模型尚未选型，应先用接口和合成数据验证延迟预算。
- SQLite 首版未承诺静态加密；若威胁模型要求抵御本机磁盘读取，需要评估 SQLCipher 与密钥存储。
- DeepSeek 的生产数据处理条款、成本和模型版本需在功能分支开始前重新确认。
- 屏幕上下文必须以本地 OCR 和保存前确认作为默认方案。
