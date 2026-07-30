# MeetingLens 数据模型

## 1. 设计原则

- SQLite 保存结构化状态；大文件保存在应用私有目录。
- 所有主键使用应用生成的 UUID，避免暴露自增规模。
- 时间统一保存为 UTC ISO 8601 字符串，展示时再转换到本地时区。
- 删除会议时默认级联删除其派生数据与文件引用。
- AI 输出始终带模型、模板版本和来源范围，允许重建与审计。
- API key、access token 和密码不属于业务数据模型，禁止写入数据库。

## 2. 实体关系

```mermaid
erDiagram
    MEETING ||--o{ ARTIFACT : contains
    MEETING ||--o{ PARTICIPANT : has
    MEETING ||--o{ TRANSCRIPT_SEGMENT : contains
    PARTICIPANT o|--o{ TRANSCRIPT_SEGMENT : speaks
    TRANSCRIPT_SEGMENT ||--o{ TRANSLATION : has
    MEETING ||--o{ SCREEN_EVENT : captures
    MEETING ||--o{ DOCUMENT : uploads
    DOCUMENT ||--o{ DOCUMENT_CHUNK : contains
    MEETING ||--o{ EVIDENCE_ITEM : supports
    MEETING ||--o{ SUMMARY : has
    SUMMARY ||--o{ DECISION : extracts
    SUMMARY ||--o{ ACTION_ITEM : extracts
    TRANSCRIPT_SEGMENT }o--o{ SUMMARY_SOURCE : supports
    SUMMARY ||--o{ SUMMARY_SOURCE : cites
    MEETING ||--o{ PROCESSING_JOB : runs
    MEETING ||--o{ CONSENT_EVENT : records
```

## 3. 核心表

### `meetings`

| 字段                   | 类型      | 说明                                               |
| ---------------------- | --------- | -------------------------------------------------- |
| `id`                   | TEXT PK   | UUID                                               |
| `title`                | TEXT      | 用户提供的标题                                     |
| `status`               | TEXT      | `draft/recording/processing/ready/failed/archived` |
| `primary_language`     | TEXT      | BCP 47 语言标签                                    |
| `selected_window_json` | TEXT NULL | 用户选定窗口的受控标识/标题快照，不含截图          |
| `started_at`           | TEXT NULL | 实际开始时间                                       |
| `ended_at`             | TEXT NULL | 实际结束时间                                       |
| `created_at`           | TEXT      | 创建时间                                           |
| `updated_at`           | TEXT      | 最近更新时间                                       |
| `deleted_at`           | TEXT NULL | 软删除/清理协调标记                                |

#### 会议生命周期

会议状态由 Rust service 和专用 Tauri command 控制，普通更新接口只能修改标题、主要语言等可编辑元数据，不得直接写入 `status`、`started_at` 或 `ended_at`。

| 操作                  | 前置状态    | 后置状态    | 时间规则                                    |
| --------------------- | ----------- | ----------- | ------------------------------------------- |
| 创建                  | 不适用      | `draft`     | `started_at`、`ended_at` 均为空             |
| `start_meeting`       | `draft`     | `recording` | Rust 后端写入当前 UTC ISO 8601 `started_at` |
| `end_meeting`（模拟） | `recording` | `ready`     | Rust 后端写入当前 UTC ISO 8601 `ended_at`   |

- MVP 1A-2 的合成会议没有异步处理步骤，因此结束后直接进入 `ready`。
- `processing` 保留给 MVP 1C 及后续真实异步处理，只能由受信任后端在创建相应 job 后进入，WebView 不得自行设置。
- `failed` 与 `archived` 同样只能通过后续定义的专用 service 操作进入，不属于通用会议更新字段。
- 重复开始、重复结束、`draft → ready`、`ready → recording` 等非法转换必须原子拒绝，并返回稳定错误码 `meeting_invalid_state`。
- 时间戳由后端生成；前端传入的开始/结束时间不得作为状态转换依据。
- 生命周期相关的非法操作统一使用对外错误码 `meeting_invalid_state`；不得再引入含义重复的 `invalid_state_transition`。

### `artifacts`

| 字段            | 类型    | 说明                                 |
| --------------- | ------- | ------------------------------------ |
| `id`            | TEXT PK | UUID                                 |
| `meeting_id`    | TEXT FK | 所属会议                             |
| `kind`          | TEXT    | `audio/upload/extracted_text/export` |
| `relative_path` | TEXT    | 应用私有目录内的受控相对路径         |
| `media_type`    | TEXT    | 经检测的 MIME 类型                   |
| `byte_size`     | INTEGER | 文件大小                             |
| `sha256`        | TEXT    | 内容完整性哈希                       |
| `retention`     | TEXT    | `session/default/pinned`             |
| `created_at`    | TEXT    | 创建时间                             |

`relative_path` 不接受 `..`、盘符、UNC 或绝对路径。原始文件名只用于用户界面显示，并经过长度与控制字符清理。

### `participants`

| 字段                      | 类型      | 说明                          |
| ------------------------- | --------- | ----------------------------- |
| `id`                      | TEXT PK   | UUID                          |
| `meeting_id`              | TEXT FK   | 所属会议                      |
| `diarization_label`       | TEXT      | 例如 `speaker_01`，会议内唯一 |
| `display_name`            | TEXT NULL | 用户主动设置或修正的名称      |
| `voice_sample_path`       | TEXT NULL | Phase 2 可选的受控相对路径    |
| `voice_sample_consent_at` | TEXT NULL | 语音样本授权时间              |
| `created_at`              | TEXT      | 创建时间                      |

语音样本不用于跨会议自动识别，必须允许重录和删除。系统不得在没有独立同意与安全设计时通过声纹映射真实身份。

### `transcript_segments`

| 字段             | 类型         | 说明                 |
| ---------------- | ------------ | -------------------- |
| `id`             | TEXT PK      | UUID                 |
| `meeting_id`     | TEXT FK      | 所属会议             |
| `participant_id` | TEXT FK NULL | 可为空的说话人       |
| `start_ms`       | INTEGER      | 相对会议开始的毫秒数 |
| `end_ms`         | INTEGER      | 结束毫秒数           |
| `language`       | TEXT         | BCP 47 标签          |
| `text`           | TEXT         | 转录正文             |
| `confidence`     | REAL NULL    | 0–1，若引擎提供      |
| `revision`       | INTEGER      | 用户修订版本         |
| `created_at`     | TEXT         | 创建时间             |
| `updated_at`     | TEXT         | 修改时间             |

约束：`start_ms >= 0`、`end_ms > start_ms`、同一会议内按时间建立索引。

### `screen_events`

| 字段                     | 类型         | 说明                                           |
| ------------------------ | ------------ | ---------------------------------------------- |
| `id`                     | TEXT PK      | UUID                                           |
| `meeting_id`             | TEXT FK      | 所属会议                                       |
| `captured_at_ms`         | INTEGER      | 相对会议开始的毫秒数                           |
| `window_ref`             | TEXT         | 用户选定窗口的稳定本地引用                     |
| `source_kind`            | TEXT         | `accessibility/ocr/user_note`                  |
| `event_type`             | TEXT         | `slide_change/code_change/decision_text/other` |
| `text`                   | TEXT         | 经确认/脱敏的结构化文字                        |
| `screenshot_artifact_id` | TEXT FK NULL | 临时截图引用，默认在提取后删除                 |
| `created_at`             | TEXT         | 创建时间                                       |

### `documents`

| 字段            | 类型         | 说明                          |
| --------------- | ------------ | ----------------------------- |
| `id`            | TEXT PK      | UUID                          |
| `meeting_id`    | TEXT FK      | 所属会议                      |
| `artifact_id`   | TEXT FK      | 原始上传文件                  |
| `display_name`  | TEXT         | 清理后的展示名称              |
| `document_type` | TEXT         | 经检测的格式                  |
| `parse_status`  | TEXT         | `queued/parsing/ready/failed` |
| `page_count`    | INTEGER NULL | 若格式提供                    |
| `created_at`    | TEXT         | 创建时间                      |

### `document_chunks`

| 字段           | 类型         | 说明                         |
| -------------- | ------------ | ---------------------------- |
| `id`           | TEXT PK      | UUID                         |
| `document_id`  | TEXT FK      | 来源文档                     |
| `ordinal`      | INTEGER      | 文档内顺序                   |
| `page_number`  | INTEGER NULL | 来源页码                     |
| `section_path` | TEXT NULL    | 标题/工作表/幻灯片等结构路径 |
| `text`         | TEXT         | 提取正文                     |
| `content_hash` | TEXT         | 去重与完整性校验             |

### `evidence_items`

| 字段                    | 类型         | 说明                         |
| ----------------------- | ------------ | ---------------------------- |
| `id`                    | TEXT PK      | UUID                         |
| `meeting_id`            | TEXT FK      | 所属会议                     |
| `kind`                  | TEXT         | `transcript/document/screen` |
| `transcript_segment_id` | TEXT FK NULL | 转录来源                     |
| `document_chunk_id`     | TEXT FK NULL | 文档来源                     |
| `screen_event_id`       | TEXT FK NULL | 屏幕来源                     |
| `label`                 | TEXT         | 面向用户的证据标签           |
| `created_at`            | TEXT         | 创建时间                     |

三种来源 ID 必须且只能有一个非空，使用 `CHECK` 约束保证。

### `translations`

| 字段              | 类型      | 说明                    |
| ----------------- | --------- | ----------------------- |
| `id`              | TEXT PK   | UUID                    |
| `segment_id`      | TEXT FK   | 原始转录片段            |
| `target_language` | TEXT      | 目标语言                |
| `text`            | TEXT      | 译文                    |
| `provider`        | TEXT      | 本地/外部 provider 标识 |
| `model`           | TEXT NULL | 模型标识，不含凭据      |
| `created_at`      | TEXT      | 创建时间                |

### `summaries`

| 字段             | 类型    | 说明                                      |
| ---------------- | ------- | ----------------------------------------- |
| `id`             | TEXT PK | UUID                                      |
| `meeting_id`     | TEXT FK | 所属会议                                  |
| `kind`           | TEXT    | `overview/decisions/actions/risks/custom` |
| `content_json`   | TEXT    | 版本化结构化结果                          |
| `provider`       | TEXT    | 例如 `deepseek` 或 `local`                |
| `model`          | TEXT    | 实际模型标识                              |
| `prompt_version` | TEXT    | 提示模板版本                              |
| `is_user_edited` | INTEGER | 0/1                                       |
| `created_at`     | TEXT    | 创建时间                                  |

### `summary_sources`

| 字段         | 类型    | 说明                           |
| ------------ | ------- | ------------------------------ |
| `summary_id` | TEXT FK | 摘要                           |
| `segment_id` | TEXT FK | 支撑片段                       |
| `relation`   | TEXT    | `supports/contradicts/context` |

联合主键为 `(summary_id, segment_id, relation)`。

### `decisions`

| 字段               | 类型         | 说明                            |
| ------------------ | ------------ | ------------------------------- |
| `id`               | TEXT PK      | UUID                            |
| `summary_id`       | TEXT FK      | 来源摘要                        |
| `description`      | TEXT         | 决策内容                        |
| `status`           | TEXT         | `proposed/confirmed/superseded` |
| `evidence_item_id` | TEXT FK NULL | 可核验证据                      |
| `is_user_edited`   | INTEGER      | 0/1                             |
| `created_at`       | TEXT         | 创建时间                        |

### `action_items`

| 字段                | 类型         | 说明                         |
| ------------------- | ------------ | ---------------------------- |
| `id`                | TEXT PK      | UUID                         |
| `summary_id`        | TEXT FK      | 来源摘要                     |
| `description`       | TEXT         | 行动描述                     |
| `owner_label`       | TEXT NULL    | 文本标签，不强制映射真实身份 |
| `due_at`            | TEXT NULL    | 用户确认后的截止时间         |
| `status`            | TEXT         | `open/done/dismissed`        |
| `source_segment_id` | TEXT FK NULL | 可核验来源                   |
| `evidence_item_id`  | TEXT FK NULL | 转录/文档/屏幕的统一证据引用 |

### `processing_jobs`

| 字段            | 类型      | 说明                                             |
| --------------- | --------- | ------------------------------------------------ |
| `id`            | TEXT PK   | UUID                                             |
| `meeting_id`    | TEXT FK   | 所属会议                                         |
| `job_type`      | TEXT      | `transcribe/translate/diarize/extract/summarize` |
| `status`        | TEXT      | `queued/running/succeeded/failed/cancelled`      |
| `progress`      | REAL      | 0–1                                              |
| `attempt_count` | INTEGER   | 已尝试次数                                       |
| `input_json`    | TEXT      | 只含 ID 和安全参数                               |
| `error_code`    | TEXT NULL | 稳定错误分类，不含敏感正文                       |
| `created_at`    | TEXT      | 创建时间                                         |
| `started_at`    | TEXT NULL | 开始时间                                         |
| `finished_at`   | TEXT NULL | 结束时间                                         |

### `consent_events`

| 字段          | 类型    | 说明                                   |
| ------------- | ------- | -------------------------------------- |
| `id`          | TEXT PK | UUID                                   |
| `meeting_id`  | TEXT FK | 所属会议                               |
| `capability`  | TEXT    | `recording/screen_context/external_ai` |
| `decision`    | TEXT    | `granted/revoked`                      |
| `scope_json`  | TEXT    | 数据范围、用途和 provider              |
| `recorded_at` | TEXT    | 记录时间                               |

## 4. SQLite 约束与索引

- 启用 `PRAGMA foreign_keys = ON`。
- 使用 WAL 时，`.db-wal` 和 `.db-shm` 必须保持本地且被 Git 忽略。
- 对 `transcript_segments(meeting_id, start_ms)`、`screen_events(meeting_id, captured_at_ms)`、`document_chunks(document_id, ordinal)`、`processing_jobs(status, created_at)` 和 `artifacts(meeting_id, kind)` 建索引。
- 状态字段使用 `CHECK` 约束或迁移层枚举校验。
- 所有迁移在事务中执行，并在 schema 版本表中记录。

## 5. 文件布局

```text
app-data/
├── meetinglens.sqlite3
├── meetings/<meeting-id>/
│   ├── audio/
│   ├── uploads/
│   ├── voice-samples/
│   ├── screen-cache/
│   ├── extracted/
│   └── exports/
└── tmp/<job-id>/
```

数据库保存相对于 `app-data` 的路径。任何文件访问必须先解析、规范化并确认最终路径仍位于该根目录内。

## 6. 删除与保留

MVP 1A-1 尚无会议所有的 artifact、后台 job 或文件句柄。当前实现可在同一事务内设置 `deleted_at` 后硬删除 meeting；任一步失败则整笔回滚。此时 `deleted_at` 是删除协调边界的预留字段，不要求留下持久 tombstone。

MVP 1A-2-1 的删除状态契约固定如下：

- `draft` 和 `ready` 允许事务内硬删除。
- `recording` 禁止删除；用户必须先通过 `end_meeting` 正常结束，拒绝时返回 `meeting_invalid_state`。
- `processing`、`failed` 和 `archived` 当前不能由 Meeting Command API 进入，也不允许删除；后续阶段必须先在文档中定义相应清理语义。
- 删除格式正确但不存在的会议返回 `meeting_not_found`，不得将其伪装为删除成功。

从首次引入 artifact、后台 job 或会议文件的阶段开始，删除必须升级为可重试的协调流程：

1. 用户发起删除后，持久化 `deleted_at` tombstone，禁止新任务。
2. 取消/等待正在运行的任务，并关闭文件句柄。
3. 在事务中删除派生表记录，再删除 artifact 文件。
4. 若文件删除失败，保留最小 tombstone 供下次启动重试。
5. 完成后删除 meeting 记录和 tombstone。
6. 外部 provider 已接收的数据无法通过本地删除撤回，发送前必须明确提示。

引入 artifact 的 PR 必须同时迁移删除实现与测试，不得继续把无文件阶段的同事务硬删除语义当作完整删除流程。

默认保留策略在实现前由产品决策确认；首个版本不进行静默自动删除。

## 7. 数据模型版本化

- `schema_migrations(version, applied_at, checksum)` 记录数据库迁移。
- Sidecar API 和 `content_json` 都包含独立 `api_version/schema_version`。
- 破坏性迁移必须先备份并获得用户批准，不得自动丢弃未知字段。
