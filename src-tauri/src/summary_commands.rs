use std::fmt;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::data::{
    CreateSummaryRevision, CreateSummaryRun, MeetingService, SummaryActionItem,
    SummaryActionStatus, SummaryClaim, SummaryContent, SummaryDecision, SummaryDecisionStatus,
    SummaryError, SummaryEvidence, SummaryEvidencePage, SummaryOutputLanguage, SummaryPolicy,
    SummaryRun, SummaryRunPage, SummaryStatus, SummaryTemplate,
};

#[cfg(test)]
const SUMMARY_COMMAND_NAMES: &[&str] = &[
    "get_summary_policy",
    "create_summary_run",
    "create_summary_revision",
    "execute_summary_run",
    "get_summary_run",
    "list_summary_runs",
    "cancel_summary_run",
    "list_summary_evidence",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSummaryRunRequest {
    pub meeting_id: String,
    pub source_ids: Vec<String>,
    pub template: SummaryTemplate,
    pub output_language: SummaryOutputLanguage,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SummaryRunRequest {
    pub meeting_id: String,
    pub summary_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListSummaryRunsRequest {
    pub meeting_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListSummaryEvidenceRequest {
    pub meeting_id: String,
    pub summary_id: String,
    pub cursor: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryPolicyResponse {
    pub templates: Vec<SummaryTemplate>,
    pub output_languages: Vec<SummaryOutputLanguage>,
    pub run_purpose: &'static str,
    pub trigger_kind: &'static str,
    pub provider: &'static str,
    pub max_segments: usize,
    pub max_input_bytes: usize,
    pub max_evidence_excerpt_bytes: usize,
    pub list_default_limit: u64,
    pub list_max_limit: u64,
    pub evidence_default_limit: u64,
    pub evidence_max_limit: u64,
}

impl From<SummaryPolicy> for SummaryPolicyResponse {
    fn from(policy: SummaryPolicy) -> Self {
        Self {
            templates: policy.templates,
            output_languages: policy.output_languages,
            run_purpose: policy.run_purpose,
            trigger_kind: policy.trigger_kind,
            provider: policy.provider,
            max_segments: policy.max_segments,
            max_input_bytes: policy.max_input_bytes,
            max_evidence_excerpt_bytes: policy.max_evidence_excerpt_bytes,
            list_default_limit: policy.list_default_limit,
            list_max_limit: policy.list_max_limit,
            evidence_default_limit: policy.evidence_default_limit,
            evidence_max_limit: policy.evidence_max_limit,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryClaimResponse {
    pub text: String,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryClaimResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryClaimResponse")
            .field("text", &"[REDACTED]")
            .field("evidence_ids", &self.evidence_ids)
            .finish()
    }
}

impl From<SummaryClaim> for SummaryClaimResponse {
    fn from(claim: SummaryClaim) -> Self {
        Self {
            text: claim.text,
            evidence_ids: claim.evidence_ids,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDecisionResponse {
    pub text: String,
    pub status: SummaryDecisionStatus,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryDecisionResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryDecisionResponse")
            .field("text", &"[REDACTED]")
            .field("status", &self.status)
            .field("evidence_ids", &self.evidence_ids)
            .finish()
    }
}

impl From<SummaryDecision> for SummaryDecisionResponse {
    fn from(decision: SummaryDecision) -> Self {
        Self {
            text: decision.text,
            status: decision.status,
            evidence_ids: decision.evidence_ids,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryActionItemResponse {
    pub text: String,
    pub owner: Option<String>,
    pub due_date: Option<String>,
    pub status: SummaryActionStatus,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryActionItemResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryActionItemResponse")
            .field("text", &"[REDACTED]")
            .field("owner", &self.owner.as_ref().map(|_| "[REDACTED]"))
            .field("due_date", &self.due_date.as_ref().map(|_| "[REDACTED]"))
            .field("status", &self.status)
            .field("evidence_ids", &self.evidence_ids)
            .finish()
    }
}

impl From<SummaryActionItem> for SummaryActionItemResponse {
    fn from(item: SummaryActionItem) -> Self {
        Self {
            text: item.text,
            owner: item.owner,
            due_date: item.due_date,
            status: item.status,
            evidence_ids: item.evidence_ids,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryContentResponse {
    pub schema_version: u32,
    pub overview: SummaryClaimResponse,
    pub decisions: Vec<SummaryDecisionResponse>,
    pub action_items: Vec<SummaryActionItemResponse>,
    pub risks: Vec<SummaryClaimResponse>,
    pub open_questions: Vec<SummaryClaimResponse>,
}

impl fmt::Debug for SummaryContentResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryContentResponse")
            .field("schema_version", &self.schema_version)
            .field("overview", &"[REDACTED]")
            .field("decision_count", &self.decisions.len())
            .field("action_item_count", &self.action_items.len())
            .field("risk_count", &self.risks.len())
            .field("open_question_count", &self.open_questions.len())
            .finish()
    }
}

impl From<SummaryContent> for SummaryContentResponse {
    fn from(content: SummaryContent) -> Self {
        Self {
            schema_version: content.schema_version,
            overview: SummaryClaimResponse::from(content.overview),
            decisions: content
                .decisions
                .into_iter()
                .map(SummaryDecisionResponse::from)
                .collect(),
            action_items: content
                .action_items
                .into_iter()
                .map(SummaryActionItemResponse::from)
                .collect(),
            risks: content
                .risks
                .into_iter()
                .map(SummaryClaimResponse::from)
                .collect(),
            open_questions: content
                .open_questions
                .into_iter()
                .map(SummaryClaimResponse::from)
                .collect(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRunResponse {
    pub id: String,
    pub meeting_id: String,
    pub series_id: String,
    pub previous_summary_id: Option<String>,
    pub revision: u64,
    pub run_purpose: String,
    pub trigger_kind: String,
    pub template: SummaryTemplate,
    pub output_language: SummaryOutputLanguage,
    pub status: SummaryStatus,
    pub provider: String,
    pub provider_mode: String,
    pub model: String,
    pub prompt_version: String,
    pub content: Option<SummaryContentResponse>,
    pub error_code: Option<String>,
    pub source_count: u64,
    pub input_segment_count: u64,
    pub input_bytes: u64,
    pub is_stale: bool,
    pub stale_reason: Option<String>,
    pub stale_at: Option<String>,
    pub is_user_edited: bool,
    pub snapshot_created_at: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub cancel_requested_at: Option<String>,
    pub updated_at: String,
}

impl fmt::Debug for SummaryRunResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryRunResponse")
            .field("id", &self.id)
            .field("meeting_id", &self.meeting_id)
            .field("series_id", &self.series_id)
            .field("previous_summary_id", &self.previous_summary_id)
            .field("revision", &self.revision)
            .field("run_purpose", &self.run_purpose)
            .field("trigger_kind", &self.trigger_kind)
            .field("template", &self.template)
            .field("output_language", &self.output_language)
            .field("status", &self.status)
            .field("provider", &self.provider)
            .field("provider_mode", &self.provider_mode)
            .field("model", &self.model)
            .field("prompt_version", &self.prompt_version)
            .field("content", &self.content.as_ref().map(|_| "[REDACTED]"))
            .field("error_code", &self.error_code)
            .field("source_count", &self.source_count)
            .field("input_segment_count", &self.input_segment_count)
            .field("input_bytes", &self.input_bytes)
            .field("is_stale", &self.is_stale)
            .field("stale_reason", &self.stale_reason)
            .field("stale_at", &self.stale_at)
            .field("is_user_edited", &self.is_user_edited)
            .field("snapshot_created_at", &self.snapshot_created_at)
            .field("created_at", &self.created_at)
            .field("started_at", &self.started_at)
            .field("finished_at", &self.finished_at)
            .field("cancel_requested_at", &self.cancel_requested_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl From<SummaryRun> for SummaryRunResponse {
    fn from(run: SummaryRun) -> Self {
        Self {
            id: run.id,
            meeting_id: run.meeting_id,
            series_id: run.series_id,
            previous_summary_id: run.previous_summary_id,
            revision: run.revision,
            run_purpose: run.run_purpose,
            trigger_kind: run.trigger_kind,
            template: run.template,
            output_language: run.output_language,
            status: run.status,
            provider: run.provider,
            provider_mode: run.provider_mode,
            model: run.model,
            prompt_version: run.prompt_version,
            content: run.content.map(SummaryContentResponse::from),
            error_code: run.error_code,
            source_count: run.source_count,
            input_segment_count: run.input_segment_count,
            input_bytes: run.input_bytes,
            is_stale: run.is_stale,
            stale_reason: run.stale_reason,
            stale_at: run.stale_at,
            is_user_edited: run.is_user_edited,
            snapshot_created_at: run.snapshot_created_at,
            created_at: run.created_at,
            started_at: run.started_at,
            finished_at: run.finished_at,
            cancel_requested_at: run.cancel_requested_at,
            updated_at: run.updated_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEvidenceResponse {
    pub id: String,
    pub summary_id: String,
    pub meeting_id: String,
    pub kind: String,
    pub source_id: String,
    pub transcript_segment_id: String,
    pub segment_revision: u64,
    pub label: String,
    pub speaker_snapshot: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub ordinal: u64,
    pub global_ordinal: u64,
    pub excerpt: String,
    pub source_deleted: bool,
    pub created_at: String,
}

impl fmt::Debug for SummaryEvidenceResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryEvidenceResponse")
            .field("id", &self.id)
            .field("summary_id", &self.summary_id)
            .field("meeting_id", &self.meeting_id)
            .field("kind", &self.kind)
            .field("source_id", &self.source_id)
            .field("transcript_segment_id", &self.transcript_segment_id)
            .field("segment_revision", &self.segment_revision)
            .field("label", &"[REDACTED]")
            .field(
                "speaker_snapshot",
                &self.speaker_snapshot.as_ref().map(|_| "[REDACTED]"),
            )
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("ordinal", &self.ordinal)
            .field("global_ordinal", &self.global_ordinal)
            .field("excerpt", &"[REDACTED]")
            .field("source_deleted", &self.source_deleted)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl From<SummaryEvidence> for SummaryEvidenceResponse {
    fn from(evidence: SummaryEvidence) -> Self {
        Self {
            id: evidence.id,
            summary_id: evidence.summary_id,
            meeting_id: evidence.meeting_id,
            kind: evidence.kind,
            source_id: evidence.source_id,
            transcript_segment_id: evidence.transcript_segment_id,
            segment_revision: evidence.segment_revision,
            label: evidence.label,
            speaker_snapshot: evidence.speaker_snapshot,
            start_offset_ms: evidence.start_offset_ms,
            end_offset_ms: evidence.end_offset_ms,
            ordinal: evidence.ordinal,
            global_ordinal: evidence.global_ordinal,
            excerpt: evidence.excerpt,
            source_deleted: evidence.source_deleted,
            created_at: evidence.created_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSummaryRunsResponse {
    pub items: Vec<SummaryRunResponse>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total_count: u64,
}

impl fmt::Debug for ListSummaryRunsResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListSummaryRunsResponse")
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .field("has_more", &self.has_more)
            .field("total_count", &self.total_count)
            .finish()
    }
}

impl From<SummaryRunPage> for ListSummaryRunsResponse {
    fn from(page: SummaryRunPage) -> Self {
        Self {
            items: page
                .items
                .into_iter()
                .map(SummaryRunResponse::from)
                .collect(),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
            total_count: page.total_count,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSummaryEvidenceResponse {
    pub items: Vec<SummaryEvidenceResponse>,
    pub next_cursor: Option<u64>,
    pub has_more: bool,
    pub total_count: u64,
}

impl fmt::Debug for ListSummaryEvidenceResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListSummaryEvidenceResponse")
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .field("has_more", &self.has_more)
            .field("total_count", &self.total_count)
            .finish()
    }
}

impl From<SummaryEvidencePage> for ListSummaryEvidenceResponse {
    fn from(page: SummaryEvidencePage) -> Self {
        Self {
            items: page
                .items
                .into_iter()
                .map(SummaryEvidenceResponse::from)
                .collect(),
            next_cursor: page.next_cursor,
            has_more: page.has_more,
            total_count: page.total_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<SummaryError> for SummaryCommandError {
    fn from(error: SummaryError) -> Self {
        Self {
            code: error.code(),
            message: error.safe_message(),
            retryable: error.retryable(),
        }
    }
}

type CommandResult<T> = Result<T, SummaryCommandError>;

#[tauri::command]
pub fn get_summary_policy(service: State<'_, MeetingService>) -> SummaryPolicyResponse {
    SummaryPolicyResponse::from(service.summaries().policy())
}

#[tauri::command]
pub fn create_summary_run(
    request: CreateSummaryRunRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<SummaryRunResponse> {
    create_summary_run_with_service(&service, request)
}

#[tauri::command]
pub fn create_summary_revision(
    request: SummaryRunRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<SummaryRunResponse> {
    create_summary_revision_with_service(&service, request)
}

#[tauri::command]
pub async fn execute_summary_run(
    request: SummaryRunRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<SummaryRunResponse> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        execute_summary_run_with_service(&service, request)
    })
    .await
    .map_err(|_| SummaryCommandError::from(SummaryError::Internal))?
}

#[tauri::command]
pub fn get_summary_run(
    request: SummaryRunRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<SummaryRunResponse> {
    get_summary_run_with_service(&service, request)
}

#[tauri::command]
pub fn list_summary_runs(
    request: ListSummaryRunsRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<ListSummaryRunsResponse> {
    list_summary_runs_with_service(&service, request)
}

#[tauri::command]
pub fn cancel_summary_run(
    request: SummaryRunRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<SummaryRunResponse> {
    cancel_summary_run_with_service(&service, request)
}

#[tauri::command]
pub fn list_summary_evidence(
    request: ListSummaryEvidenceRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<ListSummaryEvidenceResponse> {
    list_summary_evidence_with_service(&service, request)
}

fn create_summary_run_with_service(
    service: &MeetingService,
    request: CreateSummaryRunRequest,
) -> CommandResult<SummaryRunResponse> {
    service
        .summaries()
        .create_run(CreateSummaryRun {
            meeting_id: request.meeting_id,
            source_ids: request.source_ids,
            template: request.template,
            output_language: request.output_language,
        })
        .map(SummaryRunResponse::from)
        .map_err(SummaryCommandError::from)
}

fn create_summary_revision_with_service(
    service: &MeetingService,
    request: SummaryRunRequest,
) -> CommandResult<SummaryRunResponse> {
    service
        .summaries()
        .create_revision(CreateSummaryRevision {
            meeting_id: request.meeting_id,
            summary_id: request.summary_id,
        })
        .map(SummaryRunResponse::from)
        .map_err(SummaryCommandError::from)
}

fn execute_summary_run_with_service(
    service: &MeetingService,
    request: SummaryRunRequest,
) -> CommandResult<SummaryRunResponse> {
    service
        .summaries()
        .execute(&request.meeting_id, &request.summary_id)
        .map(SummaryRunResponse::from)
        .map_err(SummaryCommandError::from)
}

fn get_summary_run_with_service(
    service: &MeetingService,
    request: SummaryRunRequest,
) -> CommandResult<SummaryRunResponse> {
    service
        .summaries()
        .get(&request.meeting_id, &request.summary_id)
        .map(SummaryRunResponse::from)
        .map_err(SummaryCommandError::from)
}

fn list_summary_runs_with_service(
    service: &MeetingService,
    request: ListSummaryRunsRequest,
) -> CommandResult<ListSummaryRunsResponse> {
    service
        .summaries()
        .list(
            &request.meeting_id,
            request.cursor.as_deref(),
            request.limit,
        )
        .map(ListSummaryRunsResponse::from)
        .map_err(SummaryCommandError::from)
}

fn cancel_summary_run_with_service(
    service: &MeetingService,
    request: SummaryRunRequest,
) -> CommandResult<SummaryRunResponse> {
    service
        .summaries()
        .cancel(&request.meeting_id, &request.summary_id)
        .map(SummaryRunResponse::from)
        .map_err(SummaryCommandError::from)
}

fn list_summary_evidence_with_service(
    service: &MeetingService,
    request: ListSummaryEvidenceRequest,
) -> CommandResult<ListSummaryEvidenceResponse> {
    service
        .summaries()
        .list_evidence(
            &request.meeting_id,
            &request.summary_id,
            request.cursor,
            request.limit,
        )
        .map(ListSummaryEvidenceResponse::from)
        .map_err(SummaryCommandError::from)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use crate::data::{
        CreateMeeting, SummaryActionItem, SummaryClaim, SummaryContent, SummaryDecision,
        TranscriptImportInput, TranscriptImportOutcome,
    };

    use super::*;

    struct TestWorkspace(PathBuf);

    impl TestWorkspace {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!("meetinglens-summary-command-{}", Uuid::new_v4())),
            )
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn synthetic_content(evidence_id: &str) -> SummaryContent {
        SummaryContent {
            schema_version: 1,
            overview: SummaryClaim {
                text: "Private synthetic overview".to_string(),
                evidence_ids: vec![evidence_id.to_string()],
            },
            decisions: vec![SummaryDecision {
                text: "Private synthetic decision".to_string(),
                status: SummaryDecisionStatus::Confirmed,
                evidence_ids: vec![evidence_id.to_string()],
            }],
            action_items: vec![SummaryActionItem {
                text: "Private synthetic action".to_string(),
                owner: Some("Private synthetic owner".to_string()),
                due_date: Some("Private synthetic due date".to_string()),
                status: SummaryActionStatus::Assigned,
                evidence_ids: vec![evidence_id.to_string()],
            }],
            risks: Vec::new(),
            open_questions: Vec::new(),
        }
    }

    fn synthetic_run(content: Option<SummaryContent>) -> SummaryRun {
        SummaryRun {
            id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            series_id: Uuid::new_v4().to_string(),
            previous_summary_id: None,
            revision: 1,
            run_purpose: "final".to_string(),
            trigger_kind: "manual".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            status: SummaryStatus::Completed,
            provider: "deterministic_mock".to_string(),
            provider_mode: "mock".to_string(),
            model: "deterministic-mock-v1".to_string(),
            prompt_version: "summary-v1".to_string(),
            content,
            error_code: None,
            source_count: 1,
            input_segment_count: 1,
            input_bytes: 32,
            is_stale: false,
            stale_reason: None,
            stale_at: None,
            is_user_edited: false,
            snapshot_created_at: "2026-08-07T00:00:00.000Z".to_string(),
            created_at: "2026-08-07T00:00:00.000Z".to_string(),
            started_at: Some("2026-08-07T00:00:01.000Z".to_string()),
            finished_at: Some("2026-08-07T00:00:02.000Z".to_string()),
            cancel_requested_at: None,
            updated_at: "2026-08-07T00:00:02.000Z".to_string(),
        }
    }

    #[test]
    fn request_dtos_are_camel_case_and_reject_provider_results_evidence_and_unknown_fields() {
        let meeting_id = Uuid::new_v4().to_string();
        let summary_id = Uuid::new_v4().to_string();
        let source_id = Uuid::new_v4().to_string();
        let request: CreateSummaryRunRequest = serde_json::from_value(json!({
            "meetingId": meeting_id,
            "sourceIds": [source_id],
            "template": "detailed",
            "outputLanguage": "en"
        }))
        .expect("camelCase create request should deserialize");
        assert_eq!(request.template, SummaryTemplate::Detailed);
        let list_request: ListSummaryRunsRequest = serde_json::from_value(json!({
            "meetingId": meeting_id,
            "cursor": summary_id,
            "limit": 50
        }))
        .expect("camelCase keyset request should deserialize");
        assert_eq!(list_request.cursor.as_deref(), Some(summary_id.as_str()));

        for forbidden in [
            json!({
                "meetingId": Uuid::new_v4().to_string(),
                "sourceIds": [Uuid::new_v4().to_string()],
                "template": "detailed",
                "outputLanguage": "en",
                "providerResult": { "overview": "private" }
            }),
            json!({
                "meetingId": Uuid::new_v4().to_string(),
                "sourceIds": [Uuid::new_v4().to_string()],
                "template": "detailed",
                "outputLanguage": "en",
                "evidence": [{ "excerpt": "private" }]
            }),
            json!({
                "meeting_id": Uuid::new_v4().to_string(),
                "sourceIds": [Uuid::new_v4().to_string()],
                "template": "detailed",
                "outputLanguage": "en"
            }),
        ] {
            assert!(serde_json::from_value::<CreateSummaryRunRequest>(forbidden).is_err());
        }

        assert!(
            serde_json::from_value::<SummaryRunRequest>(json!({
                "meetingId": meeting_id,
                "summaryId": summary_id,
                "content": { "schemaVersion": 1 }
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ListSummaryEvidenceRequest>(json!({
                "meetingId": Uuid::new_v4().to_string(),
                "summaryId": Uuid::new_v4().to_string(),
                "providerOutput": "private"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ListSummaryRunsRequest>(json!({
                "meetingId": Uuid::new_v4().to_string(),
                "sourcePath": "private"
            }))
            .is_err()
        );
    }

    #[test]
    fn typed_responses_use_the_frozen_wire_casing_without_exposing_snapshot_segments() {
        let evidence_id = Uuid::new_v4().to_string();
        let response =
            SummaryRunResponse::from(synthetic_run(Some(synthetic_content(&evidence_id))));
        let serialized = serde_json::to_value(&response).expect("run response should serialize");

        assert!(serialized.get("meetingId").is_some());
        assert!(serialized.get("outputLanguage").is_some());
        assert!(serialized.get("snapshotCreatedAt").is_some());
        assert_eq!(serialized["providerMode"], "mock");
        assert_eq!(serialized["content"]["schemaVersion"], 1);
        assert!(serialized["content"].get("actionItems").is_some());
        assert!(
            serialized["content"]["overview"]
                .get("evidenceIds")
                .is_some()
        );
        for forbidden in [
            "snapshotSegments",
            "segments",
            "originalText",
            "sourceDisplayName",
            "speakerLabel",
        ] {
            assert!(serialized.get(forbidden).is_none());
        }
    }

    #[test]
    fn debug_output_redacts_summary_content_and_evidence_text() {
        let evidence_id = Uuid::new_v4().to_string();
        let run = SummaryRunResponse::from(synthetic_run(Some(synthetic_content(&evidence_id))));
        let evidence = SummaryEvidenceResponse::from(SummaryEvidence {
            id: evidence_id,
            summary_id: run.id.clone(),
            meeting_id: run.meeting_id.clone(),
            kind: "transcript".to_string(),
            source_id: Uuid::new_v4().to_string(),
            transcript_segment_id: Uuid::new_v4().to_string(),
            segment_revision: 1,
            label: "Private synthetic source label".to_string(),
            speaker_snapshot: Some("Private synthetic speaker".to_string()),
            start_offset_ms: None,
            end_offset_ms: None,
            ordinal: 0,
            global_ordinal: 0,
            excerpt: "Private synthetic evidence excerpt".to_string(),
            source_deleted: false,
            created_at: "2026-08-07T00:00:00.000Z".to_string(),
        });
        let debug = format!("{run:?} {evidence:?}");

        for private in [
            "Private synthetic overview",
            "Private synthetic decision",
            "Private synthetic action",
            "Private synthetic owner",
            "Private synthetic due date",
            "Private synthetic source label",
            "Private synthetic speaker",
            "Private synthetic evidence excerpt",
        ] {
            assert!(!debug.contains(private));
        }
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn command_helpers_enforce_meeting_ownership_and_cover_the_run_lifecycle() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.0).expect("service should initialize");
        let meeting = service
            .create(CreateMeeting {
                title: "Synthetic summary command meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create");
        let other_meeting = service
            .create(CreateMeeting {
                title: "Synthetic unrelated meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("other meeting should create");
        let imported = service
            .transcripts()
            .import_pasted_text(
                &meeting.id,
                TranscriptImportInput {
                    display_name: "Synthetic transcript".to_string(),
                    original_filename: None,
                    content: b"Speaker One: Synthetic command boundary point".to_vec(),
                    language: Some("en-AU".to_string()),
                    base_started_at: None,
                },
            )
            .expect("transcript should import");
        let source_id = match imported {
            TranscriptImportOutcome::Imported { source, .. } => source.id,
            TranscriptImportOutcome::Duplicate { .. } => panic!("new transcript must import"),
        };

        let queued = create_summary_run_with_service(
            &service,
            CreateSummaryRunRequest {
                meeting_id: meeting.id.clone(),
                source_ids: vec![source_id],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            },
        )
        .expect("run should queue");
        assert_eq!(queued.status, SummaryStatus::Queued);
        assert_eq!(queued.content, None);

        let wrong_owner = get_summary_run_with_service(
            &service,
            SummaryRunRequest {
                meeting_id: other_meeting.id,
                summary_id: queued.id.clone(),
            },
        )
        .expect_err("another meeting must not access the run");
        assert_eq!(wrong_owner.code, "summary_run_not_found");

        let completed = execute_summary_run_with_service(
            &service,
            SummaryRunRequest {
                meeting_id: meeting.id.clone(),
                summary_id: queued.id.clone(),
            },
        )
        .expect("mock run should complete");
        assert_eq!(completed.status, SummaryStatus::Completed);
        assert!(completed.content.is_some());

        let evidence = list_summary_evidence_with_service(
            &service,
            ListSummaryEvidenceRequest {
                meeting_id: meeting.id.clone(),
                summary_id: completed.id.clone(),
                cursor: None,
                limit: None,
            },
        )
        .expect("evidence should list");
        assert!(!evidence.items.is_empty());
        assert!(
            evidence
                .items
                .iter()
                .all(|item| item.meeting_id == meeting.id && item.summary_id == completed.id)
        );

        let revision = create_summary_revision_with_service(
            &service,
            SummaryRunRequest {
                meeting_id: meeting.id.clone(),
                summary_id: completed.id,
            },
        )
        .expect("revision should queue");
        assert_eq!(revision.revision, 2);
        assert_eq!(revision.status, SummaryStatus::Queued);
        let cancelled = cancel_summary_run_with_service(
            &service,
            SummaryRunRequest {
                meeting_id: meeting.id.clone(),
                summary_id: revision.id,
            },
        )
        .expect("queued revision should cancel");
        assert_eq!(cancelled.status, SummaryStatus::Cancelled);

        let page = list_summary_runs_with_service(
            &service,
            ListSummaryRunsRequest {
                meeting_id: meeting.id,
                cursor: None,
                limit: None,
            },
        )
        .expect("runs should list");
        assert_eq!(page.total_count, 2);
        assert_eq!(page.items.len(), 2);
    }

    #[test]
    fn error_registration_and_minimum_capability_are_safe_and_stable() {
        let errors = [
            SummaryError::EvidenceInvalid,
            SummaryError::InputInsufficient,
            SummaryError::InputTooLarge,
            SummaryError::Internal,
            SummaryError::InvalidInput,
            SummaryError::InvalidState,
            SummaryError::MeetingInvalidState,
            SummaryError::MeetingNotFound,
            SummaryError::PersistenceFailed,
            SummaryError::ProviderFailed,
            SummaryError::ProviderOutputInvalid,
            SummaryError::RevisionConflict,
            SummaryError::RunNotFound,
            SummaryError::SegmentLimitExceeded,
            SummaryError::SnapshotMissing,
            SummaryError::SourceNotFound,
        ];
        for source_error in errors {
            let error = SummaryCommandError::from(source_error);
            assert_eq!(error.code, source_error.code());
            assert_eq!(error.message, source_error.safe_message());
            assert_eq!(error.retryable, source_error.retryable());
            let serialized = serde_json::to_string(&error).expect("error should serialize");
            for forbidden in [
                "c:\\users\\",
                "select ",
                "sqlite",
                "rusqlite",
                "backtrace",
                "authorization",
                "bearer ",
            ] {
                assert!(!serialized.to_ascii_lowercase().contains(forbidden));
            }
        }

        let source = include_str!("summary_commands.rs").replace("\r\n", "\n");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source should exist")
            .to_ascii_lowercase();
        for forbidden in [
            "rusqlite",
            "insert ",
            "delete from",
            "std::fs",
            "reqwest",
            "secret_store",
            "summaryprovideroutput",
            "summaryproviderrequest",
            "execute_with_provider",
            "summarysnapshotsegment",
            "original_text",
        ] {
            assert!(!production.contains(forbidden));
        }
        assert!(production.contains("pub async fn execute_summary_run"));
        assert!(production.contains("tauri::async_runtime::spawn_blocking"));

        let app_builder = include_str!("lib.rs");
        let build_script = include_str!("../build.rs");
        let permissions = include_str!("../permissions/ui-commands.toml");
        for command in SUMMARY_COMMAND_NAMES {
            assert!(app_builder.contains(command));
            assert!(build_script.contains(command));
            assert!(permissions.contains(&format!("allow-{}", command.replace('_', "-"))));
        }
        let summary_set = permissions
            .split("[[set]]")
            .find(|set| set.contains("summary-run-commands"))
            .expect("summary permission set should exist");
        assert_eq!(summary_set.matches("\"allow-").count(), 8);

        let capability = include_str!("../capabilities/default.json");
        assert!(capability.contains("summary-run-commands"));
        for forbidden_permission in [
            "dialog:allow-open",
            "fs:",
            "shell:",
            "process:",
            "http:",
            "dialog:default",
        ] {
            assert!(!capability.contains(forbidden_permission));
        }
    }
}
