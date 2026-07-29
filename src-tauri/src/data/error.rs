use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataError {
    Database,
    InvalidInput,
    MigrationConflict,
    NotFound,
    StorageUnavailable,
    UnsafePath,
}

impl DataError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Database => "meeting_database_error",
            Self::InvalidInput => "meeting_invalid_input",
            Self::MigrationConflict => "meeting_migration_conflict",
            Self::NotFound => "meeting_not_found",
            Self::StorageUnavailable => "meeting_storage_unavailable",
            Self::UnsafePath => "meeting_unsafe_path",
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for DataError {}

impl From<rusqlite::Error> for DataError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

pub type DataResult<T> = Result<T, DataError>;
