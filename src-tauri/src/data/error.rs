use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataError {
    Database,
    InvalidId,
    InvalidInput,
    InvalidState,
    MeetingDeleteFailed,
    MeetingDeletePending,
    MigrationConflict,
    NotFound,
    StorageUnavailable,
    UnsafePath,
}

impl DataError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Database | Self::MigrationConflict => "persistence_error",
            Self::InvalidId => "invalid_meeting_id",
            Self::InvalidInput => "validation_error",
            Self::InvalidState => "meeting_invalid_state",
            Self::MeetingDeleteFailed => "meeting_delete_failed",
            Self::MeetingDeletePending => "meeting_delete_pending",
            Self::NotFound => "meeting_not_found",
            Self::StorageUnavailable => "database_unavailable",
            Self::UnsafePath => "internal_error",
        }
    }

    pub const fn safe_message(self) -> &'static str {
        match self {
            Self::Database => "Meeting data could not be saved.",
            Self::InvalidId => "The meeting ID is invalid.",
            Self::InvalidInput => "The meeting details are invalid.",
            Self::InvalidState => "The meeting cannot be changed in its current state.",
            Self::MeetingDeleteFailed => "The meeting could not be deleted safely.",
            Self::MeetingDeletePending => "The meeting deletion is already in progress.",
            Self::MigrationConflict => "Meeting data could not be opened safely.",
            Self::NotFound => "The meeting was not found.",
            Self::StorageUnavailable => "Meeting storage is unavailable.",
            Self::UnsafePath => "An internal error occurred.",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Database
                | Self::MeetingDeleteFailed
                | Self::MeetingDeletePending
                | Self::StorageUnavailable
        )
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
