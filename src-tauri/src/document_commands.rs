use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::data::{
    ALLOWED_DOCUMENT_EXTENSIONS, Document, DocumentError, DocumentImportOutcome,
    DocumentImportPhase, DocumentImportPolicy, DocumentImportProgress, MeetingService,
};
use crate::document_parsing_runtime::DocumentParsingRuntime;

#[cfg(test)]
const DOCUMENT_COMMAND_NAMES: &[&str] = &[
    "select_and_import_documents",
    "list_documents",
    "get_document",
    "request_delete_document",
    "retry_delete_document",
    "get_document_import_policy",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingDocumentRequest {
    pub meeting_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentIdRequest {
    pub document_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub id: String,
    pub meeting_id: String,
    pub original_filename: String,
    pub safe_extension: String,
    pub detected_media_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub status: String,
    pub imported_at: Option<String>,
    pub updated_at: String,
    pub failure_code: Option<String>,
    pub parser_version: Option<String>,
    pub page_count: Option<u64>,
}

impl From<Document> for DocumentResponse {
    fn from(document: Document) -> Self {
        Self {
            id: document.id,
            meeting_id: document.meeting_id,
            original_filename: document.original_filename,
            safe_extension: document.safe_extension,
            detected_media_type: document.detected_media_type,
            size_bytes: document.size_bytes,
            status: document.status.as_str().to_string(),
            imported_at: document.imported_at,
            updated_at: document.updated_at,
            failure_code: document.failure_code,
            parser_version: document.parser_version_optional,
            page_count: document.page_count_optional,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDocumentsResponse {
    pub documents: Vec<DocumentResponse>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImportPolicyResponse {
    pub allowed_extensions: Vec<String>,
    pub max_file_size_bytes: u64,
    pub max_batch_count: usize,
    pub max_meeting_file_count: u64,
    pub max_meeting_total_bytes: u64,
}

impl From<DocumentImportPolicy> for DocumentImportPolicyResponse {
    fn from(policy: DocumentImportPolicy) -> Self {
        Self {
            allowed_extensions: ALLOWED_DOCUMENT_EXTENSIONS
                .iter()
                .map(|extension| (*extension).to_string())
                .collect(),
            max_file_size_bytes: policy.max_file_bytes,
            max_batch_count: policy.max_batch_files,
            max_meeting_file_count: policy.max_meeting_files,
            max_meeting_total_bytes: policy.max_meeting_bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImportItemResponse {
    pub document_id: String,
    pub original_filename: String,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImportBatchResult {
    pub batch_id: String,
    pub cancelled: bool,
    pub imported: Vec<DocumentImportItemResponse>,
    pub duplicates: Vec<DocumentImportItemResponse>,
    pub failed: Vec<DocumentImportItemResponse>,
}

impl DocumentImportBatchResult {
    fn cancelled(batch_id: String) -> Self {
        Self {
            batch_id,
            cancelled: true,
            imported: Vec::new(),
            duplicates: Vec::new(),
            failed: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImportProgressResponse {
    pub batch_id: String,
    pub file_index: usize,
    pub total_files: usize,
    pub document_id: Option<String>,
    pub original_filename: String,
    pub phase: String,
    pub bytes_processed: u64,
    pub total_bytes: Option<u64>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDeleteResponse {
    pub document_id: String,
    pub status: String,
    pub deleted: bool,
    pub pending: bool,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<DocumentError> for DocumentCommandError {
    fn from(error: DocumentError) -> Self {
        Self {
            code: error.code(),
            message: error.safe_message(),
            retryable: error.retryable(),
        }
    }
}

type CommandResult<T> = Result<T, DocumentCommandError>;

trait DocumentPicker {
    fn pick_files(&self) -> Result<Option<Vec<PathBuf>>, DocumentError>;
}

struct NativeDocumentPicker<'a> {
    app: &'a AppHandle,
}

impl DocumentPicker for NativeDocumentPicker<'_> {
    fn pick_files(&self) -> Result<Option<Vec<PathBuf>>, DocumentError> {
        self.app
            .dialog()
            .file()
            .set_title("Add meeting files")
            .add_filter("Meeting documents", ALLOWED_DOCUMENT_EXTENSIONS)
            .blocking_pick_files()
            .map(|files| {
                files
                    .into_iter()
                    .map(|file| {
                        file.into_path()
                            .map_err(|_| DocumentError::StorageUnavailable)
                    })
                    .collect()
            })
            .transpose()
    }
}

#[tauri::command]
pub async fn select_and_import_documents(
    request: MeetingDocumentRequest,
    on_progress: Channel<DocumentImportProgressResponse>,
    app: AppHandle,
    service: State<'_, MeetingService>,
) -> CommandResult<DocumentImportBatchResult> {
    let picker = NativeDocumentPicker { app: &app };
    select_and_import_with_picker(&service, request, &picker, |progress| {
        deliver_progress(&on_progress, progress);
    })
}

fn deliver_progress(
    channel: &Channel<DocumentImportProgressResponse>,
    progress: DocumentImportProgressResponse,
) {
    let _ = channel.send(progress);
}

#[tauri::command]
pub fn list_documents(
    request: MeetingDocumentRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<ListDocumentsResponse> {
    list_documents_with_service(&service, request)
}

#[tauri::command]
pub fn get_document(
    request: DocumentIdRequest,
    service: State<'_, MeetingService>,
) -> CommandResult<DocumentResponse> {
    get_document_with_service(&service, request)
}

#[tauri::command]
pub fn request_delete_document(
    request: DocumentIdRequest,
    service: State<'_, MeetingService>,
    parser: State<'_, DocumentParsingRuntime>,
) -> CommandResult<DocumentDeleteResponse> {
    parser
        .cancel_for_document_and_wait(&request.document_id)
        .map_err(DocumentCommandError::from)?;
    delete_document_with_service(&service, request, false)
}

#[tauri::command]
pub fn retry_delete_document(
    request: DocumentIdRequest,
    service: State<'_, MeetingService>,
    parser: State<'_, DocumentParsingRuntime>,
) -> CommandResult<DocumentDeleteResponse> {
    parser
        .cancel_for_document_and_wait(&request.document_id)
        .map_err(DocumentCommandError::from)?;
    delete_document_with_service(&service, request, true)
}

#[tauri::command]
pub fn get_document_import_policy(
    service: State<'_, MeetingService>,
) -> DocumentImportPolicyResponse {
    service.documents().policy().into()
}

fn select_and_import_with_picker(
    service: &MeetingService,
    request: MeetingDocumentRequest,
    picker: &impl DocumentPicker,
    mut on_progress: impl FnMut(DocumentImportProgressResponse),
) -> CommandResult<DocumentImportBatchResult> {
    let batch_id = Uuid::new_v4().to_string();
    let Some(source_paths) = picker.pick_files().map_err(DocumentCommandError::from)? else {
        return Ok(DocumentImportBatchResult::cancelled(batch_id));
    };
    let outcomes = service
        .documents()
        .import_batch_with_progress(&request.meeting_id, &source_paths, |progress| {
            on_progress(progress_response(&batch_id, progress));
        })
        .map_err(DocumentCommandError::from)?;

    let mut result = DocumentImportBatchResult {
        batch_id,
        cancelled: false,
        imported: Vec::new(),
        duplicates: Vec::new(),
        failed: Vec::new(),
    };
    for outcome in outcomes {
        match outcome {
            DocumentImportOutcome::Imported(document) => {
                let document = *document;
                result.imported.push(DocumentImportItemResponse {
                    document_id: document.id,
                    original_filename: document.original_filename,
                    status: "imported".to_string(),
                    error_code: None,
                });
            }
            DocumentImportOutcome::Duplicate {
                document_id,
                original_filename,
            } => result.duplicates.push(DocumentImportItemResponse {
                document_id,
                original_filename,
                status: "duplicate".to_string(),
                error_code: Some("document_already_imported".to_string()),
            }),
            DocumentImportOutcome::Failed {
                document_id,
                original_filename,
                error,
            } => result.failed.push(DocumentImportItemResponse {
                document_id,
                original_filename,
                status: "failed".to_string(),
                error_code: Some(error.code().to_string()),
            }),
        }
    }
    Ok(result)
}

fn list_documents_with_service(
    service: &MeetingService,
    request: MeetingDocumentRequest,
) -> CommandResult<ListDocumentsResponse> {
    service
        .documents()
        .list(&request.meeting_id)
        .map(|documents| ListDocumentsResponse {
            documents: documents.into_iter().map(DocumentResponse::from).collect(),
        })
        .map_err(DocumentCommandError::from)
}

fn get_document_with_service(
    service: &MeetingService,
    request: DocumentIdRequest,
) -> CommandResult<DocumentResponse> {
    service
        .documents()
        .get(&request.document_id)
        .map(DocumentResponse::from)
        .map_err(DocumentCommandError::from)
}

fn delete_document_with_service(
    service: &MeetingService,
    request: DocumentIdRequest,
    retry: bool,
) -> CommandResult<DocumentDeleteResponse> {
    let result = if retry {
        service.documents().retry_delete(&request.document_id)
    } else {
        service.documents().delete(&request.document_id)
    };
    match result {
        Ok(()) => Ok(DocumentDeleteResponse {
            document_id: request.document_id,
            status: "deleted".to_string(),
            deleted: true,
            pending: false,
            retryable: false,
        }),
        Err(DocumentError::DeletePending) => Ok(DocumentDeleteResponse {
            document_id: request.document_id,
            status: "deleting".to_string(),
            deleted: false,
            pending: true,
            retryable: true,
        }),
        Err(error) => Err(DocumentCommandError::from(error)),
    }
}

fn progress_response(
    batch_id: &str,
    progress: DocumentImportProgress,
) -> DocumentImportProgressResponse {
    DocumentImportProgressResponse {
        batch_id: batch_id.to_string(),
        file_index: progress.file_index,
        total_files: progress.total_files,
        document_id: progress.document_id,
        original_filename: progress.original_filename,
        phase: progress.phase.as_str().to_string(),
        bytes_processed: progress.bytes_processed,
        total_bytes: progress.total_bytes,
        status: progress_status(progress.phase).to_string(),
        error_code: progress.error_code.map(str::to_string),
    }
}

const fn progress_status(phase: DocumentImportPhase) -> &'static str {
    match phase {
        DocumentImportPhase::Selected
        | DocumentImportPhase::Validating
        | DocumentImportPhase::Copying
        | DocumentImportPhase::Committing => "in_progress",
        DocumentImportPhase::Imported => "imported",
        DocumentImportPhase::Duplicate => "duplicate",
        DocumentImportPhase::Failed => "failed",
        DocumentImportPhase::Complete => "complete",
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use serde_json::json;

    use super::*;
    use crate::data::{CreateMeeting, MeetingStatus};

    struct TestWorkspace {
        root: PathBuf,
        app_data: PathBuf,
        sources: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "meetinglens-document-command-test-{}",
                Uuid::new_v4()
            ));
            let app_data = root.join("app-data");
            let sources = root.join("sources");
            std::fs::create_dir_all(&sources).expect("sources should create");
            Self {
                root,
                app_data,
                sources,
            }
        }

        fn source(&self, name: &str, contents: &[u8]) -> PathBuf {
            let path = self.sources.join(name);
            std::fs::write(&path, contents).expect("source should write");
            path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    struct FakePicker(Result<Option<Vec<PathBuf>>, DocumentError>);

    impl DocumentPicker for FakePicker {
        fn pick_files(&self) -> Result<Option<Vec<PathBuf>>, DocumentError> {
            self.0.clone()
        }
    }

    fn create_meeting(service: &MeetingService) -> String {
        service
            .create(CreateMeeting {
                title: "Synthetic document command meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create")
            .id
    }

    #[test]
    fn picker_cancellation_is_a_safe_no_op() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting_id = create_meeting(&service);
        let mut progress = Vec::new();
        let result = select_and_import_with_picker(
            &service,
            MeetingDocumentRequest { meeting_id },
            &FakePicker(Ok(None)),
            |event| progress.push(event),
        )
        .expect("cancel should succeed");

        assert!(result.cancelled);
        assert!(result.imported.is_empty());
        assert!(progress.is_empty());
        assert_eq!(count_files(service.storage_paths().staging()), 0);
    }

    #[test]
    fn multi_file_import_returns_safe_dtos_progress_and_partial_failures() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting_id = create_meeting(&service);
        let valid = workspace.source("synthetic.txt", b"synthetic text\n");
        let duplicate = workspace.source("duplicate.txt", b"synthetic text\n");
        let invalid = workspace.source("mismatch.pdf", b"not a pdf");
        let mut progress = Vec::new();
        let result = select_and_import_with_picker(
            &service,
            MeetingDocumentRequest {
                meeting_id: meeting_id.clone(),
            },
            &FakePicker(Ok(Some(vec![valid, duplicate, invalid]))),
            |event| progress.push(event),
        )
        .expect("batch should complete");

        assert_eq!(result.imported.len(), 1);
        assert_eq!(result.duplicates.len(), 1);
        assert_eq!(result.failed.len(), 1);
        assert!(
            result
                .imported
                .iter()
                .chain(&result.duplicates)
                .chain(&result.failed)
                .all(|item| Uuid::parse_str(&item.document_id).is_ok())
        );
        assert_eq!(service.documents().list(&meeting_id).unwrap().len(), 2);
        assert!(progress.iter().all(|event| {
            !event
                .original_filename
                .contains(&workspace.root.to_string_lossy().to_string())
                && event
                    .total_bytes
                    .is_none_or(|total| event.bytes_processed <= total)
        }));
        assert!(progress.windows(2).all(|events| {
            events[0].file_index != events[1].file_index
                || events[0].phase != "copying"
                || events[1].phase != "copying"
                || events[0].bytes_processed <= events[1].bytes_processed
        }));
        assert_eq!(
            progress.last().map(|event| event.phase.as_str()),
            Some("complete")
        );
    }

    #[test]
    fn meeting_states_are_enforced_by_the_document_service() {
        for status in [
            MeetingStatus::Draft,
            MeetingStatus::Recording,
            MeetingStatus::Ready,
        ] {
            let workspace = TestWorkspace::new();
            let service =
                MeetingService::initialize(&workspace.app_data).expect("service should open");
            let meeting_id = create_meeting(&service);
            if status == MeetingStatus::Recording {
                service.start(&meeting_id).unwrap();
            } else if status == MeetingStatus::Ready {
                service.start(&meeting_id).unwrap();
                service.end(&meeting_id).unwrap();
            }
            let source = workspace.source("allowed.txt", b"allowed synthetic text");
            assert!(
                select_and_import_with_picker(
                    &service,
                    MeetingDocumentRequest { meeting_id },
                    &FakePicker(Ok(Some(vec![source]))),
                    |_| {},
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn invalid_meeting_state_and_batch_limit_return_stable_errors() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting_id = create_meeting(&service);
        let source = workspace.source("allowed.txt", b"allowed synthetic text");
        service.delete(&meeting_id).expect("meeting should delete");
        assert_eq!(
            select_and_import_with_picker(
                &service,
                MeetingDocumentRequest { meeting_id },
                &FakePicker(Ok(Some(vec![source.clone()]))),
                |_| {},
            )
            .expect_err("deleted meeting should reject")
            .code,
            "document_not_found"
        );

        let meeting_id = create_meeting(&service);
        assert_eq!(
            select_and_import_with_picker(
                &service,
                MeetingDocumentRequest { meeting_id },
                &FakePicker(Ok(Some(vec![source; 11]))),
                |_| {},
            )
            .expect_err("batch should reject")
            .code,
            "document_quota_exceeded"
        );
    }

    #[test]
    fn list_get_delete_and_retry_use_safe_service_results() {
        let workspace = TestWorkspace::new();
        let service = MeetingService::initialize(&workspace.app_data).expect("service should open");
        let meeting_id = create_meeting(&service);
        let source = workspace.source("delete.txt", b"synthetic delete");
        let document = service
            .documents()
            .import_document(&meeting_id, source)
            .expect("document should import");

        let listed = list_documents_with_service(
            &service,
            MeetingDocumentRequest {
                meeting_id: meeting_id.clone(),
            },
        )
        .expect("list should succeed");
        assert_eq!(listed.documents.len(), 1);
        assert_eq!(
            get_document_with_service(
                &service,
                DocumentIdRequest {
                    document_id: document.id.clone(),
                },
            )
            .expect("get should succeed")
            .id,
            document.id
        );
        assert!(
            serde_json::to_string(&listed)
                .expect("response should serialize")
                .find("storageRelativePath")
                .is_none()
        );

        let deleted = delete_document_with_service(
            &service,
            DocumentIdRequest {
                document_id: document.id.clone(),
            },
            false,
        )
        .expect("delete should succeed");
        assert!(deleted.deleted);
        assert_eq!(
            get_document_with_service(
                &service,
                DocumentIdRequest {
                    document_id: document.id,
                },
            )
            .expect_err("deleted document should be gone")
            .code,
            "document_not_found"
        );
        assert_eq!(
            delete_document_with_service(
                &service,
                DocumentIdRequest {
                    document_id: Uuid::new_v4().to_string(),
                },
                true,
            )
            .expect_err("non-failed retry should reject")
            .code,
            "document_not_found"
        );
    }

    #[test]
    fn request_dtos_reject_source_paths_and_responses_omit_private_fields() {
        assert!(
            serde_json::from_value::<MeetingDocumentRequest>(json!({
                "meetingId": Uuid::new_v4().to_string(),
                "sourcePath": "private"
            }))
            .is_err()
        );
        let response = DocumentResponse {
            id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            original_filename: "synthetic.txt".to_string(),
            safe_extension: "txt".to_string(),
            detected_media_type: Some("text/plain".to_string()),
            size_bytes: Some(9),
            status: "staged".to_string(),
            imported_at: Some("2026-07-31T00:00:00.000Z".to_string()),
            updated_at: "2026-07-31T00:00:00.000Z".to_string(),
            failure_code: None,
            parser_version: None,
            page_count: None,
        };
        let serialized = serde_json::to_string(&response).expect("response should serialize");
        for forbidden in [
            "sourcePath",
            "storageRelativePath",
            "sha256",
            "failureMessage",
            "staging",
            "deletionTrash",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn policy_and_error_mapping_are_stable_and_safe() {
        let policy = DocumentImportPolicyResponse::from(DocumentImportPolicy::default());
        assert_eq!(policy.allowed_extensions, ALLOWED_DOCUMENT_EXTENSIONS);
        assert_eq!(policy.max_file_size_bytes, 100 * 1024 * 1024);
        let error = DocumentCommandError::from(DocumentError::StorageUnavailable);
        assert_eq!(error.code, "document_storage_unavailable");
        let serialized = serde_json::to_string(&error).expect("error should serialize");
        for forbidden in ["c:\\users\\", "sqlite", "rusqlite", "select ", "sourcepath"] {
            assert!(!serialized.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn dropped_progress_receiver_does_not_abort_import_delivery() {
        let channel = Channel::new(|_| {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "synthetic").into())
        });
        deliver_progress(
            &channel,
            DocumentImportProgressResponse {
                batch_id: Uuid::new_v4().to_string(),
                file_index: 1,
                total_files: 1,
                document_id: None,
                original_filename: "synthetic.txt".to_string(),
                phase: "selected".to_string(),
                bytes_processed: 0,
                total_bytes: None,
                status: "in_progress".to_string(),
                error_code: None,
            },
        );
    }

    #[test]
    fn command_layer_registers_only_safe_commands_and_contains_no_data_access() {
        let source = include_str!("document_commands.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
            .to_ascii_lowercase();
        for forbidden in [
            "rusqlite",
            "select ",
            "insert ",
            "delete from",
            "std::fs",
            "copy(",
            "remove_file",
            "remove_dir",
            "sourcepath",
            "source_path:",
        ] {
            assert!(!production.contains(forbidden));
        }
        let app_builder = include_str!("lib.rs");
        for command in DOCUMENT_COMMAND_NAMES {
            assert!(app_builder.contains(command));
        }
        for forbidden in [
            "import_document_path",
            "import_paths",
            "reveal_document_path",
            "get_storage_path",
            "open_document",
            "read_document_contents",
        ] {
            assert!(!app_builder.contains(forbidden));
        }

        let capability = include_str!("../capabilities/default.json");
        assert!(capability.contains("document-management-commands"));
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

        let permissions = include_str!("../permissions/ui-commands.toml");
        for command in DOCUMENT_COMMAND_NAMES {
            let permission_name = command.replace('_', "-");
            assert!(permissions.contains(&format!("allow-{permission_name}")));
        }
    }

    fn count_files(root: &std::path::Path) -> usize {
        let mut count = 0;
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).expect("directory should read") {
                let path = entry.expect("entry should read").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    count += 1;
                }
            }
        }
        count
    }

    #[allow(dead_code)]
    fn write_ooxml(path: &std::path::Path) {
        let file = std::fs::File::create(path).expect("OOXML should create");
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file(
                "[Content_Types].xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"synthetic").unwrap();
        archive.finish().unwrap();
    }
}
