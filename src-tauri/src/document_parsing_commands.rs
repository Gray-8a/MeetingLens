use serde::{Deserialize, Serialize};
use tauri::{State, ipc::Channel};

use crate::document_commands::{DocumentCommandError, DocumentResponse};
use crate::document_parsing_runtime::{
    DocumentParseSnapshot, DocumentParserPolicy, DocumentParsingRuntime,
};

const DOCLING_VERSION: &str = "2.117.0";
const DOCUMENT_PARSER_MAX_INPUT_BYTES: u64 = 100 * 1024 * 1024;
const DOCUMENT_PARSER_MAX_PAGES: u32 = 300;
const DOCUMENT_PARSER_TIMEOUT_MS: u64 = 150_000;
const DOCUMENT_PARSER_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "xlsx", "txt", "md", "csv", "png", "jpg", "jpeg",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentParseRequest {
    pub document_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentParseJobRequest {
    pub job_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentParserFormatResponse {
    pub extension: &'static str,
    pub available: bool,
    pub requires_models: bool,
    pub ocr_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentParserPolicyResponse {
    pub worker_available: bool,
    pub models_available: bool,
    pub busy: bool,
    pub parser_version: Option<&'static str>,
    pub device: &'static str,
    pub max_input_bytes: u64,
    pub max_pages: u32,
    pub timeout_ms: u64,
    pub formats: Vec<DocumentParserFormatResponse>,
}

impl From<DocumentParserPolicy> for DocumentParserPolicyResponse {
    fn from(policy: DocumentParserPolicy) -> Self {
        let formats = DOCUMENT_PARSER_EXTENSIONS
            .iter()
            .copied()
            .map(|extension| {
                let requires_models = matches!(extension, "pdf" | "png" | "jpg" | "jpeg");
                let ocr_format = matches!(extension, "pdf" | "png" | "jpg" | "jpeg");
                DocumentParserFormatResponse {
                    extension,
                    available: policy.worker_available
                        && (!requires_models || policy.models_available),
                    requires_models,
                    ocr_available: ocr_format && policy.worker_available && policy.models_available,
                }
            })
            .collect();
        Self {
            worker_available: policy.worker_available,
            models_available: policy.models_available,
            busy: policy.busy,
            parser_version: policy.worker_available.then_some(DOCLING_VERSION),
            device: "cpu",
            max_input_bytes: DOCUMENT_PARSER_MAX_INPUT_BYTES,
            max_pages: DOCUMENT_PARSER_MAX_PAGES,
            timeout_ms: DOCUMENT_PARSER_TIMEOUT_MS,
            formats,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentParseStatusResponse {
    pub job_id: String,
    pub document_id: String,
    pub phase: crate::document_parsing_runtime::DocumentParsePhase,
    pub status: crate::document_parsing_runtime::DocumentParseStatus,
    pub attempt: u8,
    pub error_code: Option<&'static str>,
}

impl From<DocumentParseSnapshot> for DocumentParseStatusResponse {
    fn from(snapshot: DocumentParseSnapshot) -> Self {
        Self {
            job_id: snapshot.job_id,
            document_id: snapshot.document_id,
            phase: snapshot.phase,
            status: snapshot.status,
            attempt: snapshot.attempt,
            error_code: snapshot.error_code,
        }
    }
}

type CommandResult<T> = Result<T, DocumentCommandError>;

#[tauri::command]
pub fn get_document_parser_policy(
    runtime: State<'_, DocumentParsingRuntime>,
) -> DocumentParserPolicyResponse {
    runtime.policy().into()
}

#[tauri::command]
pub fn get_document_parse_status(
    request: DocumentParseJobRequest,
    runtime: State<'_, DocumentParsingRuntime>,
) -> CommandResult<DocumentParseStatusResponse> {
    runtime
        .status(&request.job_id)
        .map(DocumentParseStatusResponse::from)
        .map_err(DocumentCommandError::from)
}

#[tauri::command]
pub fn cancel_document_parse(
    request: DocumentParseJobRequest,
    runtime: State<'_, DocumentParsingRuntime>,
) -> CommandResult<DocumentParseStatusResponse> {
    runtime
        .cancel(&request.job_id)
        .map(DocumentParseStatusResponse::from)
        .map_err(DocumentCommandError::from)
}

#[tauri::command]
pub async fn parse_document(
    request: DocumentParseRequest,
    on_progress: Channel<DocumentParseStatusResponse>,
    runtime: State<'_, DocumentParsingRuntime>,
) -> CommandResult<DocumentResponse> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        runtime
            .parse(&request.document_id, |snapshot| {
                let _ = on_progress.send(DocumentParseStatusResponse::from(snapshot));
            })
            .map(DocumentResponse::from)
            .map_err(DocumentCommandError::from)
    })
    .await
    .map_err(|_| DocumentCommandError::from(crate::data::DocumentError::Internal))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT_PARSER_COMMAND_NAMES: &[&str] = &[
        "get_document_parser_policy",
        "parse_document",
        "get_document_parse_status",
        "cancel_document_parse",
    ];

    #[test]
    fn policy_is_explicit_about_model_and_ocr_requirements() {
        let response = DocumentParserPolicyResponse::from(DocumentParserPolicy {
            worker_available: true,
            models_available: false,
            busy: false,
        });
        let text = response
            .formats
            .iter()
            .find(|format| format.extension == "txt")
            .expect("text format");
        let pdf = response
            .formats
            .iter()
            .find(|format| format.extension == "pdf")
            .expect("PDF format");
        for extension in ["docx", "pptx", "xlsx"] {
            let office = response
                .formats
                .iter()
                .find(|format| format.extension == extension)
                .expect("Office format");
            assert!(office.available);
            assert!(!office.requires_models);
            assert!(!office.ocr_available);
        }
        assert!(text.available);
        assert!(!text.requires_models);
        assert!(!text.ocr_available);
        assert!(!pdf.available);
        assert!(pdf.requires_models);
        assert!(!pdf.ocr_available);
        assert_eq!(response.parser_version, Some(DOCLING_VERSION));
    }

    #[test]
    fn command_registration_exposes_no_path_or_worker_configuration_surface() {
        let app_builder = include_str!("lib.rs");
        let build_manifest = include_str!("../build.rs");
        let permissions = include_str!("../permissions/ui-commands.toml");
        for command in DOCUMENT_PARSER_COMMAND_NAMES {
            assert!(app_builder.contains(command));
            assert!(build_manifest.contains(command));
            assert!(permissions.contains(&format!("allow-{}", command.replace('_', "-"))));
        }
        let production = include_str!("document_parsing_commands.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production command source");
        for forbidden in [
            "source_path",
            "output_path",
            "models_path",
            "worker_path",
            "python_path",
            "command_line",
        ] {
            assert!(!production.contains(forbidden));
        }
    }
}
