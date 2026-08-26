mod database;
mod document_detection;
mod document_error;
mod document_model;
mod document_repository;
mod document_service;
mod error;
mod migration;
mod model;
mod paths;
mod provider_consent_model;
mod provider_consent_repository;
mod provider_consent_service;
mod repository;
mod service;
#[cfg(debug_assertions)]
mod summary_dev_harness;
mod summary_error;
mod summary_markdown;
mod summary_model;
mod summary_provider;
mod summary_repository;
mod summary_service;
#[cfg(debug_assertions)]
mod transcript_dev_harness;
mod transcript_error;
mod transcript_model;
mod transcript_parser;
mod transcript_repository;
mod transcript_service;
mod transcript_workflow;

pub use document_error::{DocumentError, DocumentResult};
pub use document_model::{
    ALLOWED_DOCUMENT_EXTENSIONS, Document, DocumentImportOutcome, DocumentImportPhase,
    DocumentImportPolicy, DocumentImportProgress, DocumentStatus, ReconcileReport,
};
pub use document_service::DocumentService;
pub use error::{DataError, DataResult};
pub use model::{CreateMeeting, Meeting, MeetingStatus, UpdateMeeting};
pub use paths::StoragePaths;
pub(crate) use provider_consent_model::{
    DEEPSEEK_CONSENT_DATA_CATEGORIES, DEEPSEEK_CONSENT_ENDPOINT_HOST, DEEPSEEK_CONSENT_MODEL,
    DEEPSEEK_CONSENT_POLICY_VERSION, DEEPSEEK_CONSENT_PROVIDER, ProviderConsentPolicy,
};
pub(crate) use provider_consent_service::ProviderConsentService;
pub use service::MeetingService;
#[cfg(debug_assertions)]
pub use summary_dev_harness::{SummarySmokeV3Seed, seed_summary_smoke_v3};
pub use summary_error::{SummaryError, SummaryResult};
pub(crate) use summary_markdown::RenderedSummaryMarkdown;
pub(crate) use summary_model::summary_content_version;
pub use summary_model::{
    CreateSummaryRevision, CreateSummaryRun, DEEPSEEK_SUMMARY_MAX_INPUT_BYTES,
    EditableSummaryActionItem, EditableSummaryContent, SUMMARY_EVIDENCE_DEFAULT_LIMIT,
    SUMMARY_EVIDENCE_EXCERPT_MAX_BYTES, SUMMARY_EVIDENCE_MAX_LIMIT, SUMMARY_LIST_DEFAULT_LIMIT,
    SUMMARY_LIST_MAX_LIMIT, SUMMARY_MAX_INPUT_BYTES, SUMMARY_MAX_SEGMENTS, SummaryActionItem,
    SummaryActionStatus, SummaryClaim, SummaryContent, SummaryDecision, SummaryDecisionStatus,
    SummaryEvidence, SummaryEvidencePage, SummaryOutputLanguage, SummaryPolicy, SummaryRun,
    SummaryRunPage, SummarySnapshotSegment, SummaryStatus, SummaryTemplate, UpdateSummaryContent,
};
#[cfg(any(test, debug_assertions))]
pub use summary_provider::DeterministicMockSummaryProvider;
pub use summary_provider::{
    DEEPSEEK_ENDPOINT_HOST, DEEPSEEK_SUMMARY_MODEL, DEEPSEEK_SUMMARY_PROMPT_VERSION,
    DEEPSEEK_SUMMARY_PROVIDER, DEEPSEEK_SUMMARY_PROVIDER_MODE, DeepSeekApiKeySource,
    DeepSeekSummaryProvider, SummaryProvider, SummaryProviderFuture, SummaryProviderOutput,
    SummaryProviderRequest,
};
#[cfg(any(test, debug_assertions))]
pub use summary_provider::{
    DeepSeekTransport, DeepSeekTransportFuture, DeepSeekTransportRequest, DeepSeekTransportResponse,
};
pub use summary_service::SummaryService;
#[cfg(debug_assertions)]
pub use transcript_dev_harness::{TranscriptSmokeV2Seed, seed_transcript_smoke_v2};
pub use transcript_error::{TranscriptError, TranscriptResult};
pub use transcript_model::{
    AUDIO_TRANSCRIPT_PARSER_VERSION, PASTED_TEXT_MAX_BYTES, StructuredAudioSegment,
    StructuredAudioTranscript, SummaryInputSegment, TRANSCRIPT_FILE_MAX_BYTES,
    TRANSCRIPT_MAX_SEGMENTS, TRANSCRIPT_PAGE_DEFAULT_LIMIT, TRANSCRIPT_PAGE_MAX_LIMIT,
    TRANSCRIPT_SEGMENT_MAX_BYTES, TranscriptImportInput, TranscriptImportOutcome, TranscriptPolicy,
    TranscriptSegment, TranscriptSegmentPage, TranscriptSource, TranscriptSourceOverview,
    TranscriptSourceType, TranscriptSpeakerGroup, TranscriptSpeakerGroupPage,
    UpdateTranscriptSegment,
};
pub use transcript_service::TranscriptService;
pub use transcript_workflow::{
    TranscriptImportWorkflow, TranscriptImportWorkflowOutcome, TranscriptWorkflowError,
    TranscriptWorkflowResult,
};
