use std::fmt;

use serde::{Deserialize, Serialize};

use super::document_error::{DocumentError, DocumentResult};

pub const ALLOWED_DOCUMENT_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "xlsx", "txt", "md", "csv", "png", "jpg", "jpeg",
];
pub const DOCUMENT_CHUNK_MAX_BYTES: usize = 65_536;
pub const DOCUMENT_CHUNK_MAX_COUNT: usize = 20_000;
pub const DOCUMENT_CHUNKS_MAX_TOTAL_BYTES: usize = 10 * 1024 * 1024;
pub const DOCUMENT_CHUNK_SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    DeleteFailed,
    Deleting,
    ImportFailed,
    Importing,
    ParseFailed,
    Parsed,
    Parsing,
    Staged,
}

impl DocumentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeleteFailed => "delete_failed",
            Self::Deleting => "deleting",
            Self::ImportFailed => "import_failed",
            Self::Importing => "importing",
            Self::ParseFailed => "parse_failed",
            Self::Parsed => "parsed",
            Self::Parsing => "parsing",
            Self::Staged => "staged",
        }
    }

    pub(crate) fn from_database(value: &str) -> DocumentResult<Self> {
        match value {
            "delete_failed" => Ok(Self::DeleteFailed),
            "deleting" => Ok(Self::Deleting),
            "import_failed" => Ok(Self::ImportFailed),
            "importing" => Ok(Self::Importing),
            "parse_failed" => Ok(Self::ParseFailed),
            "parsed" => Ok(Self::Parsed),
            "parsing" => Ok(Self::Parsing),
            "staged" => Ok(Self::Staged),
            _ => Err(DocumentError::Internal),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Document {
    pub id: String,
    pub meeting_id: String,
    pub original_filename: String,
    pub safe_extension: String,
    pub detected_media_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
    pub status: DocumentStatus,
    pub storage_relative_path: String,
    pub imported_at: Option<String>,
    pub updated_at: String,
    pub failure_code: Option<String>,
    pub failure_message_safe: Option<String>,
    pub parser_version_optional: Option<String>,
    pub page_count_optional: Option<u64>,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentChunkKind {
    Heading,
    ListItem,
    Paragraph,
    Table,
}

impl DocumentChunkKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::ListItem => "list_item",
            Self::Paragraph => "paragraph",
            Self::Table => "table",
        }
    }

    pub(crate) fn from_database(value: &str) -> DocumentResult<Self> {
        match value {
            "heading" => Ok(Self::Heading),
            "list_item" => Ok(Self::ListItem),
            "paragraph" => Ok(Self::Paragraph),
            "table" => Ok(Self::Table),
            _ => Err(DocumentError::Internal),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DocumentChunkInput {
    pub kind: DocumentChunkKind,
    pub text: String,
    pub content_sha256: String,
    pub page_start: Option<u64>,
    pub page_end: Option<u64>,
    pub section_path_json: String,
    pub table_ordinal: Option<u64>,
    pub chunk_schema_version: u64,
}

impl fmt::Debug for DocumentChunkInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DocumentChunkInput")
            .field("kind", &self.kind)
            .field("text", &"[REDACTED]")
            .field("content_sha256", &"[REDACTED]")
            .field("page_start", &self.page_start)
            .field("page_end", &self.page_end)
            .field("section_path_json", &"[REDACTED]")
            .field("table_ordinal", &self.table_ordinal)
            .field("chunk_schema_version", &self.chunk_schema_version)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub meeting_id: String,
    pub ordinal: u64,
    pub kind: DocumentChunkKind,
    pub text: String,
    pub content_sha256: String,
    pub page_start: Option<u64>,
    pub page_end: Option<u64>,
    pub section_path_json: String,
    pub table_ordinal: Option<u64>,
    pub parser_version: String,
    pub chunk_schema_version: u64,
    pub created_at: String,
}

impl fmt::Debug for DocumentChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DocumentChunk")
            .field("id", &self.id)
            .field("document_id", &self.document_id)
            .field("meeting_id", &self.meeting_id)
            .field("ordinal", &self.ordinal)
            .field("kind", &self.kind)
            .field("text", &"[REDACTED]")
            .field("content_sha256", &"[REDACTED]")
            .field("page_start", &self.page_start)
            .field("page_end", &self.page_end)
            .field("section_path_json", &"[REDACTED]")
            .field("table_ordinal", &self.table_ordinal)
            .field("parser_version", &self.parser_version)
            .field("chunk_schema_version", &self.chunk_schema_version)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentImportPolicy {
    pub max_file_bytes: u64,
    pub max_batch_files: usize,
    pub max_meeting_files: u64,
    pub max_meeting_bytes: u64,
    pub max_filename_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentImportPhase {
    Selected,
    Validating,
    Copying,
    Committing,
    Imported,
    Duplicate,
    Failed,
    Complete,
}

impl DocumentImportPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Validating => "validating",
            Self::Copying => "copying",
            Self::Committing => "committing",
            Self::Imported => "imported",
            Self::Duplicate => "duplicate",
            Self::Failed => "failed",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentImportProgress {
    pub file_index: usize,
    pub total_files: usize,
    pub original_filename: String,
    pub phase: DocumentImportPhase,
    pub bytes_processed: u64,
    pub total_bytes: Option<u64>,
    pub document_id: Option<String>,
    pub error_code: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentImportOutcome {
    Imported(Box<Document>),
    Duplicate {
        document_id: String,
        original_filename: String,
    },
    Failed {
        document_id: String,
        original_filename: String,
        error: DocumentError,
    },
}

impl Default for DocumentImportPolicy {
    fn default() -> Self {
        Self {
            max_file_bytes: 100 * 1024 * 1024,
            max_batch_files: 10,
            max_meeting_files: 20,
            max_meeting_bytes: 500 * 1024 * 1024,
            max_filename_bytes: 255,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub imports_failed_closed: u64,
    pub imports_completed: u64,
    pub missing_committed_files: u64,
    pub quarantined_files: u64,
    pub deletions_completed: u64,
    pub deletions_failed: u64,
    pub parses_failed_closed: u64,
    pub parsed_integrity_failures: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeletionJob {
    pub id: String,
    pub entity_kind: String,
    pub meeting_id: String,
    pub document_id: Option<String>,
    pub status: String,
    pub trash_relative_path: Option<String>,
    pub attempt_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_import_limits_are_fixed_to_the_approved_policy() {
        assert_eq!(
            DocumentImportPolicy::default(),
            DocumentImportPolicy {
                max_file_bytes: 100 * 1024 * 1024,
                max_batch_files: 10,
                max_meeting_files: 20,
                max_meeting_bytes: 500 * 1024 * 1024,
                max_filename_bytes: 255,
            }
        );
    }
}
