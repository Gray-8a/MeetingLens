use serde::{Deserialize, Serialize};

use super::document_error::{DocumentError, DocumentResult};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentImportPolicy {
    pub max_file_bytes: u64,
    pub max_batch_files: usize,
    pub max_meeting_files: u64,
    pub max_meeting_bytes: u64,
    pub max_filename_bytes: usize,
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
