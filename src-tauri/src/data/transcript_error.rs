use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptError {
    AlreadyImported { source_id: String },
    Empty,
    InputInsufficient,
    Internal,
    InvalidEncoding,
    InvalidFormat,
    InvalidInput,
    InvalidState,
    InvalidTimestamp,
    PersistenceFailed,
    RevisionConflict,
    SegmentLimitExceeded,
    SegmentNotFound,
    SegmentTooLarge,
    SourceInUse,
    SourceNotFound,
    TooLarge,
}

impl TranscriptError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AlreadyImported { .. } => "transcript_already_imported",
            Self::Empty => "transcript_empty",
            Self::InputInsufficient => "transcript_input_insufficient",
            Self::Internal => "internal_error",
            Self::InvalidEncoding => "transcript_invalid_encoding",
            Self::InvalidFormat => "transcript_invalid_format",
            Self::InvalidInput => "transcript_invalid_input",
            Self::InvalidState => "transcript_invalid_state",
            Self::InvalidTimestamp => "transcript_invalid_timestamp",
            Self::PersistenceFailed => "transcript_persistence_failed",
            Self::RevisionConflict => "transcript_revision_conflict",
            Self::SegmentLimitExceeded => "transcript_segment_limit_exceeded",
            Self::SegmentNotFound => "transcript_segment_not_found",
            Self::SegmentTooLarge => "transcript_segment_too_large",
            Self::SourceInUse => "transcript_source_in_use",
            Self::SourceNotFound => "transcript_source_not_found",
            Self::TooLarge => "transcript_too_large",
        }
    }

    pub const fn safe_message(&self) -> &'static str {
        match self {
            Self::AlreadyImported { .. } => "This transcript is already attached to the meeting.",
            Self::Empty => "The transcript is empty.",
            Self::InputInsufficient => "The transcript does not contain usable final segments.",
            Self::Internal => "An internal error occurred.",
            Self::InvalidEncoding => "The transcript must use supported UTF-8 text encoding.",
            Self::InvalidFormat => "The transcript format is invalid.",
            Self::InvalidInput => "The transcript details are invalid.",
            Self::InvalidState => "The transcript cannot be changed in its current state.",
            Self::InvalidTimestamp => "The transcript contains an invalid timestamp.",
            Self::PersistenceFailed => "The transcript could not be saved.",
            Self::RevisionConflict => "The transcript segment changed before this update.",
            Self::SegmentLimitExceeded => "The transcript contains too many segments.",
            Self::SegmentNotFound => "The transcript segment was not found.",
            Self::SegmentTooLarge => "A transcript segment is too large.",
            Self::SourceInUse => "The transcript source is currently in use.",
            Self::SourceNotFound => "The transcript source was not found.",
            Self::TooLarge => "The transcript is too large.",
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(self, Self::PersistenceFailed | Self::SourceInUse)
    }
}

impl fmt::Display for TranscriptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for TranscriptError {}

impl From<rusqlite::Error> for TranscriptError {
    fn from(_: rusqlite::Error) -> Self {
        Self::PersistenceFailed
    }
}

pub type TranscriptResult<T> = Result<T, TranscriptError>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn transcript_errors_have_unique_safe_protocol_codes() {
        let errors = [
            TranscriptError::AlreadyImported {
                source_id: "00000000-0000-4000-8000-000000000001".to_string(),
            },
            TranscriptError::Empty,
            TranscriptError::InputInsufficient,
            TranscriptError::Internal,
            TranscriptError::InvalidEncoding,
            TranscriptError::InvalidFormat,
            TranscriptError::InvalidInput,
            TranscriptError::InvalidState,
            TranscriptError::InvalidTimestamp,
            TranscriptError::PersistenceFailed,
            TranscriptError::RevisionConflict,
            TranscriptError::SegmentLimitExceeded,
            TranscriptError::SegmentNotFound,
            TranscriptError::SegmentTooLarge,
            TranscriptError::SourceInUse,
            TranscriptError::SourceNotFound,
            TranscriptError::TooLarge,
        ];
        let mut codes = HashSet::new();

        for error in errors {
            assert!(codes.insert(error.code()));
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
