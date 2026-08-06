use std::{
    ffi::OsStr,
    fmt,
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    data::{
        CreateMeeting, DataError, MeetingService, TranscriptError, TranscriptImportInput,
        TranscriptImportWorkflowOutcome, TranscriptPolicy, TranscriptSegment, TranscriptSource,
        TranscriptSourceType, TranscriptWorkflowError, UpdateTranscriptSegment,
    },
    meeting_commands::MeetingResponse,
};

const ALLOWED_TRANSCRIPT_EXTENSIONS: &[&str] = &["txt", "srt", "vtt"];

#[cfg(test)]
const TRANSCRIPT_COMMAND_NAMES: &[&str] = &[
    "create_meeting_from_pasted_transcript",
    "select_transcript_and_create_meeting",
    "add_pasted_transcript_to_meeting",
    "select_and_add_transcript_to_meeting",
    "list_transcript_sources",
    "get_transcript_source",
    "list_transcript_segments",
    "update_transcript_segment",
    "delete_transcript_source",
    "get_transcript_import_policy",
];

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateMeetingFromPasteRequest {
    pub title: String,
    pub primary_language: String,
    pub pasted_text: String,
    pub display_name: String,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
}

impl fmt::Debug for CreateMeetingFromPasteRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateMeetingFromPasteRequest")
            .field("title", &"[REDACTED]")
            .field("primary_language", &self.primary_language)
            .field("pasted_text", &"[REDACTED]")
            .field("display_name", &"[REDACTED]")
            .field("language", &self.language)
            .field("base_started_at", &self.base_started_at)
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectTranscriptAndCreateMeetingRequest {
    pub title: String,
    pub primary_language: String,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddPastedTranscriptToMeetingRequest {
    pub meeting_id: String,
    pub pasted_text: String,
    pub display_name: String,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
}

impl fmt::Debug for AddPastedTranscriptToMeetingRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AddPastedTranscriptToMeetingRequest")
            .field("meeting_id", &self.meeting_id)
            .field("pasted_text", &"[REDACTED]")
            .field("display_name", &"[REDACTED]")
            .field("language", &self.language)
            .field("base_started_at", &self.base_started_at)
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectAndAddTranscriptToMeetingRequest {
    pub meeting_id: String,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingTranscriptRequest {
    pub meeting_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptSourceRequest {
    pub meeting_id: String,
    pub source_id: String,
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTranscriptSegmentRequest {
    pub meeting_id: String,
    pub segment_id: String,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub speaker_name: Option<Option<String>>,
    pub original_text: Option<String>,
    pub expected_revision: u64,
}

impl fmt::Debug for UpdateTranscriptSegmentRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpdateTranscriptSegmentRequest")
            .field("meeting_id", &self.meeting_id)
            .field("segment_id", &self.segment_id)
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "original_text",
                &self.original_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expected_revision", &self.expected_revision)
            .finish()
    }
}

fn deserialize_nullable_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSourceResponse {
    pub id: String,
    pub meeting_id: String,
    pub source_type: TranscriptSourceType,
    pub display_name: String,
    pub original_filename: Option<String>,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
    pub segment_count: u64,
    pub imported_at: String,
    pub created_at: String,
    pub updated_at: String,
}

impl fmt::Debug for TranscriptSourceResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSourceResponse")
            .field("id", &self.id)
            .field("meeting_id", &self.meeting_id)
            .field("source_type", &self.source_type)
            .field("display_name", &"[REDACTED]")
            .field(
                "original_filename",
                &self.original_filename.as_ref().map(|_| "[REDACTED]"),
            )
            .field("language", &self.language)
            .field("base_started_at", &self.base_started_at)
            .field("segment_count", &self.segment_count)
            .field("imported_at", &self.imported_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl TranscriptSourceResponse {
    fn from_parts(source: TranscriptSource, segment_count: u64) -> Self {
        Self {
            id: source.id,
            meeting_id: source.meeting_id,
            source_type: source.source_type,
            display_name: source.display_name,
            original_filename: source.original_filename,
            language: source.language,
            base_started_at: source.base_started_at,
            segment_count,
            imported_at: source.imported_at,
            created_at: source.created_at,
            updated_at: source.updated_at,
        }
    }
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegmentResponse {
    pub id: String,
    pub source_id: String,
    pub meeting_id: String,
    pub ordinal: u64,
    pub speaker_label: Option<String>,
    pub speaker_name: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub original_text: String,
    pub translated_text: Option<String>,
    pub language: Option<String>,
    pub confidence: Option<f64>,
    pub is_final: bool,
    pub revision: u64,
    pub corrected_at: Option<String>,
}

impl fmt::Debug for TranscriptSegmentResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSegmentResponse")
            .field("id", &self.id)
            .field("source_id", &self.source_id)
            .field("meeting_id", &self.meeting_id)
            .field("ordinal", &self.ordinal)
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
            .field(
                "translated_text",
                &self.translated_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("language", &self.language)
            .field("confidence", &self.confidence)
            .field("is_final", &self.is_final)
            .field("revision", &self.revision)
            .field("corrected_at", &self.corrected_at)
            .finish()
    }
}

impl From<TranscriptSegment> for TranscriptSegmentResponse {
    fn from(segment: TranscriptSegment) -> Self {
        Self {
            id: segment.id,
            source_id: segment.source_id,
            meeting_id: segment.meeting_id,
            ordinal: segment.ordinal,
            speaker_label: segment.speaker_label,
            speaker_name: segment.speaker_name,
            start_offset_ms: segment.start_offset_ms,
            end_offset_ms: segment.end_offset_ms,
            original_text: segment.original_text,
            translated_text: segment.translated_text,
            language: segment.language,
            confidence: segment.confidence,
            is_final: segment.is_final,
            revision: segment.revision,
            corrected_at: segment.corrected_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTranscriptSourcesResponse {
    pub sources: Vec<TranscriptSourceResponse>,
}

impl fmt::Debug for ListTranscriptSourcesResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListTranscriptSourcesResponse")
            .field("source_count", &self.sources.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTranscriptSegmentsResponse {
    pub segments: Vec<TranscriptSegmentResponse>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptImportResult {
    pub cancelled: bool,
    pub duplicate: bool,
    pub meeting: Option<MeetingResponse>,
    pub source: Option<TranscriptSourceResponse>,
    pub segment_count: u64,
}

impl fmt::Debug for TranscriptImportResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptImportResult")
            .field("cancelled", &self.cancelled)
            .field("duplicate", &self.duplicate)
            .field(
                "meeting_id",
                &self.meeting.as_ref().map(|meeting| meeting.id.as_str()),
            )
            .field(
                "source_id",
                &self.source.as_ref().map(|source| source.id.as_str()),
            )
            .field("segment_count", &self.segment_count)
            .finish()
    }
}

impl TranscriptImportResult {
    fn cancelled() -> Self {
        Self {
            cancelled: true,
            duplicate: false,
            meeting: None,
            source: None,
            segment_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTranscriptSourceResponse {
    pub source_id: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptImportPolicyResponse {
    pub allowed_extensions: Vec<String>,
    pub max_paste_bytes: usize,
    pub max_file_bytes: usize,
    pub max_segments_per_source: usize,
    pub max_segment_bytes: usize,
}

impl From<TranscriptPolicy> for TranscriptImportPolicyResponse {
    fn from(policy: TranscriptPolicy) -> Self {
        Self {
            allowed_extensions: ALLOWED_TRANSCRIPT_EXTENSIONS
                .iter()
                .map(|extension| (*extension).to_string())
                .collect(),
            max_paste_bytes: policy.max_pasted_bytes,
            max_file_bytes: policy.max_file_bytes,
            max_segments_per_source: policy.max_segments,
            max_segment_bytes: policy.max_segment_bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl TranscriptCommandError {
    const fn new(code: &'static str, message: &'static str, retryable: bool) -> Self {
        Self {
            code,
            message,
            retryable,
        }
    }

    const fn picker_unavailable() -> Self {
        Self::new(
            "transcript_picker_unavailable",
            "The transcript picker is unavailable.",
            true,
        )
    }
}

impl From<TranscriptError> for TranscriptCommandError {
    fn from(error: TranscriptError) -> Self {
        Self::new(error.code(), error.safe_message(), error.retryable())
    }
}

impl From<TranscriptWorkflowError> for TranscriptCommandError {
    fn from(error: TranscriptWorkflowError) -> Self {
        match error {
            TranscriptWorkflowError::Transcript(error) => error.into(),
            TranscriptWorkflowError::Meeting(error) => match error {
                DataError::InvalidId | DataError::InvalidInput => {
                    TranscriptError::InvalidInput.into()
                }
                DataError::Database
                | DataError::MigrationConflict
                | DataError::StorageUnavailable => TranscriptError::PersistenceFailed.into(),
                DataError::InvalidState
                | DataError::MeetingDeleteFailed
                | DataError::MeetingDeletePending => TranscriptError::MeetingInvalidState.into(),
                DataError::NotFound => TranscriptError::MeetingNotFound.into(),
                DataError::UnsafePath => TranscriptError::Internal.into(),
            },
        }
    }
}

type CommandResult<T> = Result<T, TranscriptCommandError>;

trait TranscriptPicker {
    fn pick_file(&self) -> CommandResult<Option<PathBuf>>;
}

struct NativeTranscriptPicker<'a> {
    app: &'a AppHandle,
}

impl TranscriptPicker for NativeTranscriptPicker<'_> {
    fn pick_file(&self) -> CommandResult<Option<PathBuf>> {
        self.app
            .dialog()
            .file()
            .set_title("Select a meeting transcript")
            .add_filter("Meeting transcripts", ALLOWED_TRANSCRIPT_EXTENSIONS)
            .blocking_pick_file()
            .map(|file| {
                file.into_path()
                    .map_err(|_| TranscriptCommandError::picker_unavailable())
            })
            .transpose()
    }
}

struct SelectedTranscript {
    source_type: TranscriptSourceType,
    display_name: String,
    original_filename: String,
    content: Vec<u8>,
}

#[tauri::command]
pub fn create_meeting_from_pasted_transcript(
    request: CreateMeetingFromPasteRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptImportResult> {
    create_meeting_from_paste_with_service(&service, request)
}

#[tauri::command]
pub async fn select_transcript_and_create_meeting(
    request: SelectTranscriptAndCreateMeetingRequest,
    app: AppHandle,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptImportResult> {
    let picker = NativeTranscriptPicker { app: &app };
    select_and_create_with_picker(&service, request, &picker)
}

#[tauri::command]
pub fn add_pasted_transcript_to_meeting(
    request: AddPastedTranscriptToMeetingRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptImportResult> {
    add_paste_with_service(&service, request)
}

#[tauri::command]
pub async fn select_and_add_transcript_to_meeting(
    request: SelectAndAddTranscriptToMeetingRequest,
    app: AppHandle,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptImportResult> {
    let picker = NativeTranscriptPicker { app: &app };
    select_and_add_with_picker(&service, request, &picker)
}

#[tauri::command]
pub fn list_transcript_sources(
    request: MeetingTranscriptRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<ListTranscriptSourcesResponse> {
    list_sources_with_service(&service, request)
}

#[tauri::command]
pub fn get_transcript_source(
    request: TranscriptSourceRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptSourceResponse> {
    get_source_with_service(&service, request)
}

#[tauri::command]
pub fn list_transcript_segments(
    request: TranscriptSourceRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<ListTranscriptSegmentsResponse> {
    list_segments_with_service(&service, request)
}

#[tauri::command]
pub fn update_transcript_segment(
    request: UpdateTranscriptSegmentRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<TranscriptSegmentResponse> {
    update_segment_with_service(&service, request)
}

#[tauri::command]
pub fn delete_transcript_source(
    request: TranscriptSourceRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<DeleteTranscriptSourceResponse> {
    delete_source_with_service(&service, request)
}

#[tauri::command]
pub fn get_transcript_import_policy(
    service: State<'_, MeetingService>,
) -> TranscriptImportPolicyResponse {
    TranscriptImportPolicyResponse::from(service.transcripts().policy())
}

fn create_meeting_from_paste_with_service(
    service: &MeetingService,
    request: CreateMeetingFromPasteRequest,
) -> CommandResult<TranscriptImportResult> {
    let transcript = TranscriptImportInput {
        display_name: request.display_name,
        original_filename: None,
        content: request.pasted_text.into_bytes(),
        language: request.language,
        base_started_at: request.base_started_at,
    };
    let outcome = service
        .transcript_imports()
        .create_meeting_from_transcript(
            CreateMeeting {
                title: request.title,
                primary_language: request.primary_language,
            },
            TranscriptSourceType::PastedText,
            transcript,
        )
        .map_err(TranscriptCommandError::from)?;
    complete_import_result(outcome)
}

fn add_paste_with_service(
    service: &MeetingService,
    request: AddPastedTranscriptToMeetingRequest,
) -> CommandResult<TranscriptImportResult> {
    let transcript = TranscriptImportInput {
        display_name: request.display_name,
        original_filename: None,
        content: request.pasted_text.into_bytes(),
        language: request.language,
        base_started_at: request.base_started_at,
    };
    let outcome = service
        .transcript_imports()
        .add_transcript_to_meeting(
            &request.meeting_id,
            TranscriptSourceType::PastedText,
            transcript,
        )
        .map_err(TranscriptCommandError::from)?;
    complete_import_result(outcome)
}

fn select_and_create_with_picker(
    service: &MeetingService,
    request: SelectTranscriptAndCreateMeetingRequest,
    picker: &dyn TranscriptPicker,
) -> CommandResult<TranscriptImportResult> {
    let Some(path) = picker.pick_file()? else {
        return Ok(TranscriptImportResult::cancelled());
    };
    let selected = read_selected_transcript(&path, service.transcripts().policy().max_file_bytes)?;
    let outcome = service
        .transcript_imports()
        .create_meeting_from_transcript(
            CreateMeeting {
                title: request.title,
                primary_language: request.primary_language,
            },
            selected.source_type,
            TranscriptImportInput {
                display_name: selected.display_name,
                original_filename: Some(selected.original_filename),
                content: selected.content,
                language: request.language,
                base_started_at: request.base_started_at,
            },
        )
        .map_err(TranscriptCommandError::from)?;
    complete_import_result(outcome)
}

fn select_and_add_with_picker(
    service: &MeetingService,
    request: SelectAndAddTranscriptToMeetingRequest,
    picker: &dyn TranscriptPicker,
) -> CommandResult<TranscriptImportResult> {
    let Some(path) = picker.pick_file()? else {
        return Ok(TranscriptImportResult::cancelled());
    };
    let selected = read_selected_transcript(&path, service.transcripts().policy().max_file_bytes)?;
    let outcome = service
        .transcript_imports()
        .add_transcript_to_meeting(
            &request.meeting_id,
            selected.source_type,
            TranscriptImportInput {
                display_name: selected.display_name,
                original_filename: Some(selected.original_filename),
                content: selected.content,
                language: request.language,
                base_started_at: request.base_started_at,
            },
        )
        .map_err(TranscriptCommandError::from)?;
    complete_import_result(outcome)
}

fn complete_import_result(
    outcome: TranscriptImportWorkflowOutcome,
) -> CommandResult<TranscriptImportResult> {
    let meeting = MeetingResponse::from(outcome.meeting);
    let segment_count = outcome.source.segment_count;
    Ok(TranscriptImportResult {
        cancelled: false,
        duplicate: outcome.duplicate,
        meeting: Some(meeting),
        source: Some(TranscriptSourceResponse::from_parts(
            outcome.source.source,
            segment_count,
        )),
        segment_count,
    })
}

fn list_sources_with_service(
    service: &MeetingService,
    request: MeetingTranscriptRequest,
) -> CommandResult<ListTranscriptSourcesResponse> {
    service
        .transcripts()
        .list_source_overviews(&request.meeting_id)
        .map(|overviews| ListTranscriptSourcesResponse {
            sources: overviews
                .into_iter()
                .map(|overview| {
                    TranscriptSourceResponse::from_parts(overview.source, overview.segment_count)
                })
                .collect(),
        })
        .map_err(TranscriptCommandError::from)
}

fn get_source_with_service(
    service: &MeetingService,
    request: TranscriptSourceRequest,
) -> CommandResult<TranscriptSourceResponse> {
    service
        .transcripts()
        .get_source_overview(&request.meeting_id, &request.source_id)
        .map(|overview| {
            TranscriptSourceResponse::from_parts(overview.source, overview.segment_count)
        })
        .map_err(TranscriptCommandError::from)
}

fn list_segments_with_service(
    service: &MeetingService,
    request: TranscriptSourceRequest,
) -> CommandResult<ListTranscriptSegmentsResponse> {
    service
        .transcripts()
        .list_segments(&request.meeting_id, &request.source_id)
        .map(|segments| ListTranscriptSegmentsResponse {
            segments: segments
                .into_iter()
                .map(TranscriptSegmentResponse::from)
                .collect(),
        })
        .map_err(TranscriptCommandError::from)
}

fn update_segment_with_service(
    service: &MeetingService,
    request: UpdateTranscriptSegmentRequest,
) -> CommandResult<TranscriptSegmentResponse> {
    service
        .transcripts()
        .update_segment(
            &request.meeting_id,
            &request.segment_id,
            UpdateTranscriptSegment {
                speaker_name: request.speaker_name,
                original_text: request.original_text,
                expected_revision: request.expected_revision,
            },
        )
        .map(TranscriptSegmentResponse::from)
        .map_err(TranscriptCommandError::from)
}

fn delete_source_with_service(
    service: &MeetingService,
    request: TranscriptSourceRequest,
) -> CommandResult<DeleteTranscriptSourceResponse> {
    service
        .transcripts()
        .delete_source(&request.meeting_id, &request.source_id)
        .map(|()| DeleteTranscriptSourceResponse {
            source_id: request.source_id,
            deleted: true,
        })
        .map_err(TranscriptCommandError::from)
}

fn read_selected_transcript(path: &Path, max_bytes: usize) -> CommandResult<SelectedTranscript> {
    let source_type = source_type_from_extension(path.extension())?;
    let original_filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| TranscriptCommandError::from(TranscriptError::InvalidInput))?
        .to_string();
    let path_metadata = std::fs::symlink_metadata(path)
        .map_err(|_| TranscriptCommandError::picker_unavailable())?;
    if is_symlink_or_reparse_point(&path_metadata) || !path_metadata.file_type().is_file() {
        return Err(TranscriptError::InvalidFormat.into());
    }
    let file = open_selected_transcript(path)?;
    let metadata = file
        .metadata()
        .map_err(|_| TranscriptCommandError::picker_unavailable())?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.file_type().is_file() {
        return Err(TranscriptError::InvalidFormat.into());
    }
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| TranscriptCommandError::from(TranscriptError::Internal))?;
    if metadata.len() > max_bytes_u64 {
        return Err(TranscriptError::TooLarge.into());
    }

    let mut content = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(max_bytes)
            .min(max_bytes),
    );
    file.take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(|_| TranscriptCommandError::picker_unavailable())?;
    if content.len() > max_bytes {
        return Err(TranscriptError::TooLarge.into());
    }
    std::str::from_utf8(&content).map_err(|_| TranscriptError::InvalidEncoding)?;

    Ok(SelectedTranscript {
        source_type,
        display_name: original_filename.clone(),
        original_filename,
        content,
    })
}

fn open_selected_transcript(path: &Path) -> CommandResult<std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        // Open the directory entry itself so a path changed to a reparse point between
        // validation and open cannot redirect the read to another file.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
        .open(path)
        .map_err(|_| TranscriptCommandError::picker_unavailable())
}

fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        has_windows_reparse_point_attribute(metadata.file_attributes())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
const fn has_windows_reparse_point_attribute(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn source_type_from_extension(extension: Option<&OsStr>) -> CommandResult<TranscriptSourceType> {
    let extension = extension
        .and_then(OsStr::to_str)
        .ok_or_else(|| TranscriptCommandError::from(TranscriptError::InvalidFormat))?;
    if extension.eq_ignore_ascii_case("txt") {
        Ok(TranscriptSourceType::ImportedTxt)
    } else if extension.eq_ignore_ascii_case("srt") {
        Ok(TranscriptSourceType::ImportedSrt)
    } else if extension.eq_ignore_ascii_case("vtt") {
        Ok(TranscriptSourceType::ImportedVtt)
    } else {
        Err(TranscriptError::InvalidFormat.into())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    struct TestWorkspace {
        root: PathBuf,
        app_data: PathBuf,
        files: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("meetinglens-transcript-command-{}", Uuid::new_v4()));
            let app_data = root.join("app-data");
            let files = root.join("files");
            std::fs::create_dir_all(&files).expect("test file directory should create");
            Self {
                root,
                app_data,
                files,
            }
        }

        fn file(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.files.join(name);
            std::fs::write(&path, content).expect("test transcript should write");
            path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    struct FakePicker(CommandResult<Option<PathBuf>>);

    impl TranscriptPicker for FakePicker {
        fn pick_file(&self) -> CommandResult<Option<PathBuf>> {
            self.0.clone()
        }
    }

    fn create_meeting(service: &MeetingService) -> crate::data::Meeting {
        service
            .create(CreateMeeting {
                title: "Synthetic transcript command meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create")
    }

    fn selected_error(result: CommandResult<SelectedTranscript>) -> TranscriptCommandError {
        match result {
            Ok(_) => panic!("selected transcript should fail"),
            Err(error) => error,
        }
    }

    #[test]
    fn picker_cancellation_is_a_no_op_before_validation_or_mutation() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let result = select_and_create_with_picker(
            &service,
            SelectTranscriptAndCreateMeetingRequest {
                title: String::new(),
                primary_language: String::new(),
                language: None,
                base_started_at: None,
            },
            &FakePicker(Ok(None)),
        )
        .expect("cancel should succeed");

        assert_eq!(result, TranscriptImportResult::cancelled());
        assert!(service.list().expect("meeting list should work").is_empty());
    }

    #[test]
    fn native_file_boundary_imports_txt_srt_and_vtt_and_returns_safe_dtos() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting = create_meeting(&service);
        let fixtures = [
            (
                "synthetic.txt",
                b"Speaker One: Synthetic TXT point".as_slice(),
                TranscriptSourceType::ImportedTxt,
            ),
            (
                "synthetic.srt",
                b"1\n00:00:01,000 --> 00:00:02,000\nSpeaker Two: Synthetic SRT point\n"
                    .as_slice(),
                TranscriptSourceType::ImportedSrt,
            ),
            (
                "synthetic.vtt",
                b"WEBVTT\n\n00:00:03.000 --> 00:00:04.000\n<v Speaker Three>Synthetic VTT point</v>\n"
                    .as_slice(),
                TranscriptSourceType::ImportedVtt,
            ),
        ];

        for (name, content, expected_type) in fixtures {
            let result = select_and_add_with_picker(
                &service,
                SelectAndAddTranscriptToMeetingRequest {
                    meeting_id: meeting.id.clone(),
                    language: Some("en-AU".to_string()),
                    base_started_at: None,
                },
                &FakePicker(Ok(Some(workspace.file(name, content)))),
            )
            .expect("selected transcript should import");
            assert!(!result.cancelled);
            assert!(!result.duplicate);
            assert_eq!(
                result.source.as_ref().map(|source| source.source_type),
                Some(expected_type)
            );
            assert!(result.segment_count > 0);

            let serialized = serde_json::to_string(&result).expect("result should serialize");
            for forbidden in [
                "contentSha256",
                "sha256",
                "parserName",
                "parserVersion",
                "storagePath",
                &workspace.root.to_string_lossy(),
            ] {
                assert!(!serialized.contains(forbidden));
            }
        }
    }

    #[test]
    fn duplicate_import_returns_the_existing_source_and_segment_count() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting = create_meeting(&service);
        let path = workspace.file("duplicate.txt", b"Synthetic duplicate transcript");
        let request = SelectAndAddTranscriptToMeetingRequest {
            meeting_id: meeting.id,
            language: None,
            base_started_at: None,
        };
        let first = select_and_add_with_picker(
            &service,
            request.clone(),
            &FakePicker(Ok(Some(path.clone()))),
        )
        .expect("first import should succeed");
        let duplicate = select_and_add_with_picker(&service, request, &FakePicker(Ok(Some(path))))
            .expect("duplicate should be stable");

        assert!(duplicate.duplicate);
        assert_eq!(
            duplicate.source.as_ref().map(|source| &source.id),
            first.source.as_ref().map(|source| &source.id)
        );
        assert_eq!(duplicate.segment_count, first.segment_count);
    }

    #[test]
    fn picker_boundary_rejects_unsupported_large_non_utf8_and_non_files_safely() {
        let workspace = TestWorkspace::new();
        let unsupported = workspace.file("synthetic.exe", b"Synthetic");
        assert_eq!(
            selected_error(read_selected_transcript(&unsupported, 64)).code,
            "transcript_invalid_format"
        );

        let too_large = workspace.file("large.txt", b"12345");
        assert_eq!(
            selected_error(read_selected_transcript(&too_large, 4)).code,
            "transcript_too_large"
        );

        let non_utf8 = workspace.file("encoding.txt", &[0xff, 0xfe, 0x00]);
        assert_eq!(
            selected_error(read_selected_transcript(&non_utf8, 64)).code,
            "transcript_invalid_encoding"
        );

        let directory = workspace.files.join("directory.txt");
        std::fs::create_dir(&directory).expect("directory should create");
        let error = selected_error(read_selected_transcript(&directory, 64));
        assert_eq!(error.code, "transcript_invalid_format");
        let serialized = serde_json::to_string(&error).expect("error should serialize");
        assert!(!serialized.contains(&workspace.root.to_string_lossy().to_string()));
    }

    #[test]
    fn picker_boundary_rejects_symbolic_links_before_opening_the_target() {
        let workspace = TestWorkspace::new();
        let target = workspace.file("target.txt", b"Synthetic target");
        let link = workspace.files.join("linked.txt");
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_file(&target, &link);
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&target, &link);

        if let Err(error) = link_result {
            #[cfg(windows)]
            if error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314)
            {
                return;
            }
            panic!("test symlink should create: {error}");
        }

        assert_eq!(
            selected_error(read_selected_transcript(&link, 64)).code,
            "transcript_invalid_format"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_reparse_attribute_is_rejected_without_opening_the_target() {
        assert!(has_windows_reparse_point_attribute(0x400));
        assert!(has_windows_reparse_point_attribute(0x420));
        assert!(!has_windows_reparse_point_attribute(0x20));
    }

    #[test]
    fn commands_list_update_and_delete_through_the_transcript_service() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let imported = create_meeting_from_paste_with_service(
            &service,
            CreateMeetingFromPasteRequest {
                title: "Synthetic paste workflow".to_string(),
                primary_language: "en-AU".to_string(),
                pasted_text: "Speaker One: Synthetic point".to_string(),
                display_name: "Synthetic paste".to_string(),
                language: Some("en-AU".to_string()),
                base_started_at: None,
            },
        )
        .expect("paste should import");
        let meeting_id = imported.meeting.as_ref().unwrap().id.clone();
        let source_id = imported.source.as_ref().unwrap().id.clone();

        let sources = list_sources_with_service(
            &service,
            MeetingTranscriptRequest {
                meeting_id: meeting_id.clone(),
            },
        )
        .expect("sources should list");
        assert_eq!(sources.sources.len(), 1);
        assert_eq!(
            get_source_with_service(
                &service,
                TranscriptSourceRequest {
                    meeting_id: meeting_id.clone(),
                    source_id: source_id.clone(),
                },
            )
            .expect("source should get"),
            sources.sources[0]
        );

        let segments = list_segments_with_service(
            &service,
            TranscriptSourceRequest {
                meeting_id: meeting_id.clone(),
                source_id: source_id.clone(),
            },
        )
        .expect("segments should list");
        let segment = &segments.segments[0];
        let updated = update_segment_with_service(
            &service,
            UpdateTranscriptSegmentRequest {
                meeting_id: meeting_id.clone(),
                segment_id: segment.id.clone(),
                speaker_name: Some(Some("Synthetic corrected speaker".to_string())),
                original_text: Some("Synthetic corrected point".to_string()),
                expected_revision: segment.revision,
            },
        )
        .expect("segment should update");
        assert_eq!(updated.revision, segment.revision + 1);

        assert!(
            delete_source_with_service(
                &service,
                TranscriptSourceRequest {
                    meeting_id: meeting_id.clone(),
                    source_id: source_id.clone(),
                },
            )
            .expect("source should delete")
            .deleted
        );
        assert_eq!(
            get_source_with_service(
                &service,
                TranscriptSourceRequest {
                    meeting_id,
                    source_id,
                },
            )
            .expect_err("deleted source should not get")
            .code,
            "transcript_source_not_found"
        );
    }

    #[test]
    fn request_dtos_reject_paths_content_and_other_unknown_fields() {
        let meeting_id = Uuid::new_v4().to_string();
        for forbidden in [
            json!({ "meetingId": meeting_id, "sourcePath": "private" }),
            json!({ "meetingId": meeting_id, "content": "private" }),
            json!({ "meetingId": meeting_id, "filename": "private.txt" }),
            json!({ "meetingId": meeting_id, "sourceType": "imported_txt" }),
        ] {
            assert!(
                serde_json::from_value::<SelectAndAddTranscriptToMeetingRequest>(forbidden)
                    .is_err()
            );
        }

        let clear: UpdateTranscriptSegmentRequest = serde_json::from_value(json!({
            "meetingId": Uuid::new_v4().to_string(),
            "segmentId": Uuid::new_v4().to_string(),
            "speakerName": null,
            "expectedRevision": 1
        }))
        .expect("explicit null should be accepted");
        assert_eq!(clear.speaker_name, Some(None));
    }

    #[test]
    fn policy_error_registration_and_capability_are_safe_and_stable() {
        let policy = TranscriptImportPolicyResponse::from(TranscriptPolicy::default());
        assert_eq!(policy.allowed_extensions, ["txt", "srt", "vtt"]);
        assert_eq!(policy.max_file_bytes, 10 * 1024 * 1024);

        let picker_error = TranscriptCommandError::picker_unavailable();
        assert_eq!(picker_error.code, "transcript_picker_unavailable");
        let serialized = serde_json::to_string(&picker_error).expect("error should serialize");
        for forbidden in ["c:\\users\\", "sqlite", "rusqlite", "select ", "sourcepath"] {
            assert!(!serialized.to_ascii_lowercase().contains(forbidden));
        }

        let source = include_str!("transcript_commands.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source should exist")
            .to_ascii_lowercase();
        assert!(!production.contains("fakepicker"));
        #[cfg(windows)]
        assert!(production.contains("file_flag_open_reparse_point"));
        for forbidden in ["rusqlite", "insert ", "delete from", "source_path:"] {
            assert!(!production.contains(forbidden));
        }

        let app_builder = include_str!("lib.rs");
        let permissions = include_str!("../permissions/ui-commands.toml");
        for command in TRANSCRIPT_COMMAND_NAMES {
            assert!(app_builder.contains(command));
            assert!(permissions.contains(&format!("allow-{}", command.replace('_', "-"))));
        }

        let capability = include_str!("../capabilities/default.json");
        assert!(capability.contains("transcript-management-commands"));
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

    #[test]
    fn workflow_errors_map_only_to_the_frozen_transcript_protocol() {
        let cases = [
            (DataError::InvalidId, "transcript_invalid_input"),
            (DataError::InvalidInput, "transcript_invalid_input"),
            (DataError::Database, "transcript_persistence_failed"),
            (
                DataError::MigrationConflict,
                "transcript_persistence_failed",
            ),
            (
                DataError::StorageUnavailable,
                "transcript_persistence_failed",
            ),
            (DataError::InvalidState, "meeting_invalid_state"),
            (DataError::MeetingDeleteFailed, "meeting_invalid_state"),
            (DataError::MeetingDeletePending, "meeting_invalid_state"),
            (DataError::NotFound, "meeting_not_found"),
            (DataError::UnsafePath, "internal_error"),
        ];
        for (data_error, expected_code) in cases {
            let error = TranscriptCommandError::from(TranscriptWorkflowError::Meeting(data_error));
            assert_eq!(error.code, expected_code);
        }
    }

    #[test]
    fn debug_output_redacts_transcript_bodies_and_speaker_edits() {
        let paste = CreateMeetingFromPasteRequest {
            title: "Private synthetic title".to_string(),
            primary_language: "en-AU".to_string(),
            pasted_text: "Private synthetic transcript".to_string(),
            display_name: "Private synthetic display".to_string(),
            language: None,
            base_started_at: None,
        };
        let update = UpdateTranscriptSegmentRequest {
            meeting_id: Uuid::new_v4().to_string(),
            segment_id: Uuid::new_v4().to_string(),
            speaker_name: Some(Some("Private synthetic speaker".to_string())),
            original_text: Some("Private synthetic corrected transcript".to_string()),
            expected_revision: 1,
        };
        let segment = TranscriptSegmentResponse {
            id: Uuid::new_v4().to_string(),
            source_id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            ordinal: 0,
            speaker_label: Some("Private synthetic label".to_string()),
            speaker_name: Some("Private synthetic response speaker".to_string()),
            start_offset_ms: None,
            end_offset_ms: None,
            original_text: "Private synthetic response transcript".to_string(),
            translated_text: Some("Private synthetic translation".to_string()),
            language: None,
            confidence: None,
            is_final: true,
            revision: 1,
            corrected_at: None,
        };
        let source = TranscriptSourceResponse {
            id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            source_type: TranscriptSourceType::ImportedTxt,
            display_name: "Private synthetic source display".to_string(),
            original_filename: Some("private-synthetic-file.txt".to_string()),
            language: None,
            base_started_at: None,
            segment_count: 1,
            imported_at: "2026-08-06T00:00:00.000Z".to_string(),
            created_at: "2026-08-06T00:00:00.000Z".to_string(),
            updated_at: "2026-08-06T00:00:00.000Z".to_string(),
        };
        let source_list = ListTranscriptSourcesResponse {
            sources: vec![source.clone()],
        };
        let import = TranscriptImportResult {
            cancelled: false,
            duplicate: false,
            meeting: Some(MeetingResponse {
                id: source.meeting_id.clone(),
                title: "Private synthetic import title".to_string(),
                status: crate::data::MeetingStatus::Draft,
                primary_language: "en-AU".to_string(),
                selected_window_json: None,
                started_at: None,
                ended_at: None,
                created_at: "2026-08-06T00:00:00.000Z".to_string(),
                updated_at: "2026-08-06T00:00:00.000Z".to_string(),
                deletion_error_code: None,
            }),
            source: Some(source),
            segment_count: 1,
        };
        let debug = format!("{paste:?} {update:?} {segment:?} {source_list:?} {import:?}");
        for private in [
            "Private synthetic title",
            "Private synthetic transcript",
            "Private synthetic display",
            "Private synthetic speaker",
            "Private synthetic corrected transcript",
            "Private synthetic label",
            "Private synthetic response speaker",
            "Private synthetic response transcript",
            "Private synthetic translation",
            "Private synthetic source display",
            "private-synthetic-file.txt",
            "Private synthetic import title",
        ] {
            assert!(!debug.contains(private));
        }
        assert!(debug.contains("[REDACTED]"));
    }
}
