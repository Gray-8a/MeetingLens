use std::fmt::{self, Write as _};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::summary_error::{SummaryError, SummaryResult};

pub const SUMMARY_MAX_SEGMENTS: usize = 20_000;
pub const SUMMARY_MAX_DOCUMENTS: usize = 20;
pub const SUMMARY_MAX_DOCUMENT_CHUNKS: usize = 20_000;
pub const SUMMARY_MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
pub const SUMMARY_EVIDENCE_EXCERPT_MAX_BYTES: usize = 1_024;
pub const SUMMARY_LIST_DEFAULT_LIMIT: u64 = 50;
pub const SUMMARY_LIST_MAX_LIMIT: u64 = 100;
pub const SUMMARY_EVIDENCE_DEFAULT_LIMIT: u64 = 50;
pub const SUMMARY_EVIDENCE_MAX_LIMIT: u64 = 100;
pub const SUMMARY_RESULT_MAX_BYTES: usize = 1024 * 1024;
pub const SUMMARY_MARKDOWN_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const DEEPSEEK_SUMMARY_MAX_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryTemplate {
    ActionsDecisions,
    Concise,
    Detailed,
}

impl SummaryTemplate {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActionsDecisions => "actions_decisions",
            Self::Concise => "concise",
            Self::Detailed => "detailed",
        }
    }

    pub(crate) fn from_database(value: &str) -> SummaryResult<Self> {
        match value {
            "actions_decisions" => Ok(Self::ActionsDecisions),
            "concise" => Ok(Self::Concise),
            "detailed" => Ok(Self::Detailed),
            _ => Err(SummaryError::PersistenceFailed),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryOutputLanguage {
    Bilingual,
    En,
    Zh,
}

impl SummaryOutputLanguage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bilingual => "bilingual",
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    pub(crate) fn from_database(value: &str) -> SummaryResult<Self> {
        match value {
            "bilingual" => Ok(Self::Bilingual),
            "en" => Ok(Self::En),
            "zh" => Ok(Self::Zh),
            _ => Err(SummaryError::PersistenceFailed),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    Cancelled,
    Completed,
    Failed,
    Queued,
    Running,
}

impl SummaryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Queued => "queued",
            Self::Running => "running",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }

    pub(crate) fn from_database(value: &str) -> SummaryResult<Self> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            _ => Err(SummaryError::PersistenceFailed),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryPolicy {
    pub templates: Vec<SummaryTemplate>,
    pub output_languages: Vec<SummaryOutputLanguage>,
    pub run_purpose: &'static str,
    pub trigger_kind: &'static str,
    pub provider: &'static str,
    pub max_segments: usize,
    pub max_documents: usize,
    pub max_document_chunks: usize,
    pub max_input_bytes: usize,
    pub max_evidence_excerpt_bytes: usize,
    pub list_default_limit: u64,
    pub list_max_limit: u64,
    pub evidence_default_limit: u64,
    pub evidence_max_limit: u64,
}

impl Default for SummaryPolicy {
    fn default() -> Self {
        Self {
            templates: vec![
                SummaryTemplate::Detailed,
                SummaryTemplate::Concise,
                SummaryTemplate::ActionsDecisions,
            ],
            output_languages: vec![
                SummaryOutputLanguage::Zh,
                SummaryOutputLanguage::En,
                SummaryOutputLanguage::Bilingual,
            ],
            run_purpose: "final",
            trigger_kind: "manual",
            provider: "deterministic_mock",
            max_segments: SUMMARY_MAX_SEGMENTS,
            max_documents: SUMMARY_MAX_DOCUMENTS,
            max_document_chunks: SUMMARY_MAX_DOCUMENT_CHUNKS,
            max_input_bytes: SUMMARY_MAX_INPUT_BYTES,
            max_evidence_excerpt_bytes: SUMMARY_EVIDENCE_EXCERPT_MAX_BYTES,
            list_default_limit: SUMMARY_LIST_DEFAULT_LIMIT,
            list_max_limit: SUMMARY_LIST_MAX_LIMIT,
            evidence_default_limit: SUMMARY_EVIDENCE_DEFAULT_LIMIT,
            evidence_max_limit: SUMMARY_EVIDENCE_MAX_LIMIT,
        }
    }
}

impl SummaryPolicy {
    pub(crate) fn deepseek() -> Self {
        Self {
            provider: "deepseek",
            max_input_bytes: DEEPSEEK_SUMMARY_MAX_INPUT_BYTES,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CreateSummaryRun {
    pub meeting_id: String,
    pub source_ids: Vec<String>,
    pub document_ids: Vec<String>,
    pub template: SummaryTemplate,
    pub output_language: SummaryOutputLanguage,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CreateSummaryRevision {
    pub meeting_id: String,
    pub summary_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EditableSummaryActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due_date: Option<String>,
}

impl fmt::Debug for EditableSummaryActionItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EditableSummaryActionItem")
            .field("text", &"[REDACTED]")
            .field("owner", &self.owner.as_ref().map(|_| "[REDACTED]"))
            .field("due_date", &self.due_date.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EditableSummaryContent {
    pub overview_text: String,
    pub decision_texts: Vec<String>,
    pub action_items: Vec<EditableSummaryActionItem>,
    pub risk_texts: Vec<String>,
    pub open_question_texts: Vec<String>,
}

impl fmt::Debug for EditableSummaryContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EditableSummaryContent")
            .field("overview_text", &"[REDACTED]")
            .field("decision_count", &self.decision_texts.len())
            .field("action_item_count", &self.action_items.len())
            .field("risk_count", &self.risk_texts.len())
            .field("open_question_count", &self.open_question_texts.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UpdateSummaryContent {
    pub meeting_id: String,
    pub summary_id: String,
    pub expected_content_version: String,
    pub editable_content: EditableSummaryContent,
}

impl fmt::Debug for UpdateSummaryContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpdateSummaryContent")
            .field("meeting_id", &self.meeting_id)
            .field("summary_id", &self.summary_id)
            .field("expected_content_version", &"[REDACTED]")
            .field("editable_content", &self.editable_content)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryDecisionStatus {
    Confirmed,
    Proposed,
    Superseded,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryActionStatus {
    Assigned,
    Suggested,
    Unconfirmed,
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryClaim {
    pub text: String,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryClaim {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryClaim")
            .field("text", &"[REDACTED]")
            .field("evidence_count", &self.evidence_ids.len())
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryDecision {
    pub text: String,
    pub status: SummaryDecisionStatus,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryDecision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryDecision")
            .field("text", &"[REDACTED]")
            .field("status", &self.status)
            .field("evidence_count", &self.evidence_ids.len())
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due_date: Option<String>,
    pub status: SummaryActionStatus,
    pub evidence_ids: Vec<String>,
}

impl fmt::Debug for SummaryActionItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryActionItem")
            .field("text", &"[REDACTED]")
            .field("owner", &self.owner.as_ref().map(|_| "[REDACTED]"))
            .field("due_date", &self.due_date.as_ref().map(|_| "[REDACTED]"))
            .field("status", &self.status)
            .field("evidence_count", &self.evidence_ids.len())
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryContent {
    pub schema_version: u32,
    pub overview: SummaryClaim,
    pub decisions: Vec<SummaryDecision>,
    pub action_items: Vec<SummaryActionItem>,
    pub risks: Vec<SummaryClaim>,
    pub open_questions: Vec<SummaryClaim>,
}

pub(crate) fn summary_content_version(content: &SummaryContent) -> SummaryResult<String> {
    let canonical = serde_json::to_vec(content).map_err(|_| SummaryError::PersistenceFailed)?;
    let digest = Sha256::digest(canonical);
    let mut version = String::with_capacity(64);
    for byte in digest {
        write!(&mut version, "{byte:02x}").map_err(|_| SummaryError::Internal)?;
    }
    Ok(version)
}

pub(crate) fn validate_content_version(value: &str) -> SummaryResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(SummaryError::InvalidInput)
    }
}

impl fmt::Debug for SummaryContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryContent")
            .field("schema_version", &self.schema_version)
            .field("overview", &"[REDACTED]")
            .field("decision_count", &self.decisions.len())
            .field("action_item_count", &self.action_items.len())
            .field("risk_count", &self.risks.len())
            .field("open_question_count", &self.open_questions.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRun {
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
    pub content: Option<SummaryContent>,
    pub error_code: Option<String>,
    pub source_count: u64,
    pub input_segment_count: u64,
    pub input_bytes: u64,
    pub document_count: u64,
    pub input_document_chunk_count: u64,
    pub document_input_bytes: u64,
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

impl fmt::Debug for SummaryRun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryRun")
            .field("id", &self.id)
            .field("meeting_id", &self.meeting_id)
            .field("series_id", &self.series_id)
            .field("revision", &self.revision)
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
            .field("document_count", &self.document_count)
            .field(
                "input_document_chunk_count",
                &self.input_document_chunk_count,
            )
            .field("document_input_bytes", &self.document_input_bytes)
            .field("is_stale", &self.is_stale)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummarySnapshotSegment {
    pub evidence_id: String,
    pub summary_id: String,
    pub meeting_id: String,
    pub source_id: String,
    pub segment_id: String,
    pub segment_revision: u64,
    pub source_ordinal: u64,
    pub segment_ordinal: u64,
    pub global_ordinal: u64,
    pub source_type: String,
    pub source_display_name: String,
    pub source_language: Option<String>,
    pub source_base_started_at: Option<String>,
    pub speaker_label: Option<String>,
    pub speaker_name: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub original_text: String,
    pub language: Option<String>,
    pub confidence: Option<f64>,
    pub is_final: bool,
    pub snapshot_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummarySnapshotDocumentChunk {
    pub evidence_id: String,
    pub summary_id: String,
    pub meeting_id: String,
    pub document_id: String,
    pub chunk_id: String,
    pub document_ordinal: u64,
    pub chunk_ordinal: u64,
    pub global_ordinal: u64,
    pub chunk_kind: String,
    pub document_display_name: String,
    pub document_sha256: String,
    pub chunk_sha256: String,
    pub page_start: Option<u64>,
    pub page_end: Option<u64>,
    pub section_path: Vec<String>,
    pub table_ordinal: Option<u64>,
    pub parser_version: String,
    pub chunk_schema_version: u32,
    pub text: String,
    pub snapshot_at: String,
}

impl fmt::Debug for SummarySnapshotDocumentChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummarySnapshotDocumentChunk")
            .field("evidence_id", &self.evidence_id)
            .field("summary_id", &self.summary_id)
            .field("meeting_id", &self.meeting_id)
            .field("document_id", &self.document_id)
            .field("chunk_id", &self.chunk_id)
            .field("document_ordinal", &self.document_ordinal)
            .field("chunk_ordinal", &self.chunk_ordinal)
            .field("global_ordinal", &self.global_ordinal)
            .field("chunk_kind", &self.chunk_kind)
            .field("document_display_name", &"[REDACTED]")
            .field("document_sha256", &"[SHA256]")
            .field("chunk_sha256", &"[SHA256]")
            .field("page_start", &self.page_start)
            .field("page_end", &self.page_end)
            .field("section_depth", &self.section_path.len())
            .field("table_ordinal", &self.table_ordinal)
            .field("parser_version", &self.parser_version)
            .field("chunk_schema_version", &self.chunk_schema_version)
            .field("text", &"[REDACTED]")
            .field("snapshot_at", &self.snapshot_at)
            .finish()
    }
}

impl fmt::Debug for SummarySnapshotSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummarySnapshotSegment")
            .field("evidence_id", &self.evidence_id)
            .field("summary_id", &self.summary_id)
            .field("meeting_id", &self.meeting_id)
            .field("source_id", &self.source_id)
            .field("segment_id", &self.segment_id)
            .field("segment_revision", &self.segment_revision)
            .field("source_ordinal", &self.source_ordinal)
            .field("segment_ordinal", &self.segment_ordinal)
            .field("global_ordinal", &self.global_ordinal)
            .field("source_type", &self.source_type)
            .field("source_display_name", &"[REDACTED]")
            .field("source_language", &self.source_language)
            .field("source_base_started_at", &self.source_base_started_at)
            .field(
                "speaker_label",
                &self.speaker_label.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("original_text", &"[REDACTED]")
            .field("language", &self.language)
            .field("confidence", &self.confidence)
            .field("is_final", &self.is_final)
            .field("snapshot_at", &self.snapshot_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEvidence {
    pub id: String,
    pub summary_id: String,
    pub meeting_id: String,
    pub kind: String,
    pub source_id: String,
    pub transcript_segment_id: Option<String>,
    pub segment_revision: Option<u64>,
    pub document_chunk_id: Option<String>,
    pub parser_version: Option<String>,
    pub label: String,
    pub speaker_snapshot: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub page_start: Option<u64>,
    pub page_end: Option<u64>,
    pub section_path: Vec<String>,
    pub table_ordinal: Option<u64>,
    pub ordinal: u64,
    pub global_ordinal: u64,
    pub excerpt: String,
    pub source_deleted: bool,
    pub created_at: String,
}

impl fmt::Debug for SummaryEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryEvidence")
            .field("id", &self.id)
            .field("summary_id", &self.summary_id)
            .field("meeting_id", &self.meeting_id)
            .field("kind", &self.kind)
            .field("source_id", &self.source_id)
            .field("transcript_segment_id", &self.transcript_segment_id)
            .field("segment_revision", &self.segment_revision)
            .field("document_chunk_id", &self.document_chunk_id)
            .field("parser_version", &self.parser_version)
            .field("label", &"[REDACTED]")
            .field(
                "speaker_snapshot",
                &self.speaker_snapshot.as_ref().map(|_| "[REDACTED]"),
            )
            .field("ordinal", &self.ordinal)
            .field("global_ordinal", &self.global_ordinal)
            .field("page_start", &self.page_start)
            .field("page_end", &self.page_end)
            .field("section_depth", &self.section_path.len())
            .field("table_ordinal", &self.table_ordinal)
            .field("excerpt", &"[REDACTED]")
            .field("source_deleted", &self.source_deleted)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRunPage {
    pub items: Vec<SummaryRun>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEvidencePage {
    pub items: Vec<SummaryEvidence>,
    pub next_cursor: Option<u64>,
    pub has_more: bool,
    pub total_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_item_debug_redacts_all_provider_text() {
        let item = SummaryActionItem {
            text: "Private synthetic action".to_string(),
            owner: Some("Private synthetic owner".to_string()),
            due_date: Some("Private synthetic due date".to_string()),
            status: SummaryActionStatus::Assigned,
            evidence_ids: vec!["00000000-0000-4000-8000-000000000001".to_string()],
        };

        let debug = format!("{item:?}");
        for private in [
            "Private synthetic action",
            "Private synthetic owner",
            "Private synthetic due date",
        ] {
            assert!(!debug.contains(private));
        }
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn content_version_is_the_sha256_of_canonical_typed_json() {
        let content = SummaryContent {
            schema_version: 1,
            overview: SummaryClaim {
                text: "Synthetic".to_string(),
                evidence_ids: vec!["00000000-0000-4000-8000-000000000001".to_string()],
            },
            decisions: Vec::new(),
            action_items: Vec::new(),
            risks: Vec::new(),
            open_questions: Vec::new(),
        };
        let version = summary_content_version(&content).expect("version should derive");
        assert_eq!(
            version,
            "b4cbdc83430dad75f2b0f6771b7fffb8dc5ee9591776bb2c5f13d58ea79fe87d"
        );
        assert!(validate_content_version(&version).is_ok());
        assert_eq!(
            validate_content_version(&version.to_ascii_uppercase()),
            Err(SummaryError::InvalidInput)
        );
    }
}
