use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    AlreadyImported { document_id: String },
    DeleteFailed,
    DeletePending,
    Empty,
    FilenameTooLong,
    ImportFailed,
    IntegrityFailed,
    Internal,
    InvalidState,
    InvalidType,
    NotFound,
    QuotaExceeded,
    StorageUnavailable,
    TooLarge,
    TypeMismatch,
}

impl DocumentError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AlreadyImported { .. } => "document_already_imported",
            Self::DeleteFailed => "document_delete_failed",
            Self::DeletePending => "document_delete_pending",
            Self::Empty => "document_empty",
            Self::FilenameTooLong => "document_filename_too_long",
            Self::ImportFailed => "document_import_failed",
            Self::IntegrityFailed => "document_integrity_failed",
            Self::Internal => "internal_error",
            Self::InvalidState => "document_invalid_state",
            Self::InvalidType => "document_invalid_type",
            Self::NotFound => "document_not_found",
            Self::QuotaExceeded => "document_quota_exceeded",
            Self::StorageUnavailable => "document_storage_unavailable",
            Self::TooLarge => "document_too_large",
            Self::TypeMismatch => "document_type_mismatch",
        }
    }

    pub const fn safe_message(&self) -> &'static str {
        match self {
            Self::AlreadyImported { .. } => "This document is already attached to the meeting.",
            Self::DeleteFailed => "The document could not be deleted safely.",
            Self::DeletePending => "The document deletion is already in progress.",
            Self::Empty => "The document is empty.",
            Self::FilenameTooLong => "The document filename is too long.",
            Self::ImportFailed => "The document could not be imported.",
            Self::IntegrityFailed => "The document could not be verified safely.",
            Self::Internal => "An internal error occurred.",
            Self::InvalidState => "The document cannot be changed in its current state.",
            Self::InvalidType => "The document type is not supported.",
            Self::NotFound => "The document was not found.",
            Self::QuotaExceeded => "The meeting document limit has been reached.",
            Self::StorageUnavailable => "Document storage is unavailable.",
            Self::TooLarge => "The document is too large.",
            Self::TypeMismatch => "The document type does not match its filename.",
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::DeleteFailed
                | Self::DeletePending
                | Self::ImportFailed
                | Self::IntegrityFailed
                | Self::StorageUnavailable
        )
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DocumentError {}

pub type DocumentResult<T> = Result<T, DocumentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_uses_a_stable_safe_protocol_without_path_details() {
        let errors = [
            DocumentError::AlreadyImported {
                document_id: "00000000-0000-4000-8000-000000000001".to_string(),
            },
            DocumentError::DeleteFailed,
            DocumentError::DeletePending,
            DocumentError::Empty,
            DocumentError::FilenameTooLong,
            DocumentError::ImportFailed,
            DocumentError::IntegrityFailed,
            DocumentError::Internal,
            DocumentError::InvalidState,
            DocumentError::InvalidType,
            DocumentError::NotFound,
            DocumentError::QuotaExceeded,
            DocumentError::StorageUnavailable,
            DocumentError::TooLarge,
            DocumentError::TypeMismatch,
        ];

        for error in errors {
            assert!(!error.code().is_empty());
            assert!(!error.safe_message().is_empty());
            let safe = format!("{} {}", error.code(), error.safe_message()).to_ascii_lowercase();
            for forbidden in [
                "c:\\users\\",
                "select ",
                "sqlite",
                "rusqlite",
                "backtrace",
                "authorization",
                "bearer ",
            ] {
                assert!(!safe.contains(forbidden));
            }
        }
    }
}
