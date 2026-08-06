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
mod repository;
mod service;
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
pub use service::MeetingService;
#[cfg(debug_assertions)]
pub use transcript_dev_harness::{TranscriptSmokeV2Seed, seed_transcript_smoke_v2};
pub use transcript_error::{TranscriptError, TranscriptResult};
pub use transcript_model::{
    PASTED_TEXT_MAX_BYTES, SummaryInputSegment, TRANSCRIPT_FILE_MAX_BYTES, TRANSCRIPT_MAX_SEGMENTS,
    TRANSCRIPT_SEGMENT_MAX_BYTES, TranscriptImportInput, TranscriptImportOutcome, TranscriptPolicy,
    TranscriptSegment, TranscriptSource, TranscriptSourceOverview, TranscriptSourceType,
    UpdateTranscriptSegment,
};
pub use transcript_service::TranscriptService;
pub use transcript_workflow::{
    TranscriptImportWorkflow, TranscriptImportWorkflowOutcome, TranscriptWorkflowError,
    TranscriptWorkflowResult,
};
