use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryError {
    InputInsufficient,
    InputTooLarge,
    Internal,
    InvalidInput,
    InvalidState,
    MeetingInvalidState,
    MeetingNotFound,
    PersistenceFailed,
    ProviderFailed,
    ProviderOutputInvalid,
    RunNotFound,
    SegmentLimitExceeded,
    SnapshotMissing,
    SourceNotFound,
    UnknownEvidence,
}

impl SummaryError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputInsufficient => "summary_input_insufficient",
            Self::InputTooLarge => "summary_input_too_large",
            Self::Internal => "internal_error",
            Self::InvalidInput => "summary_invalid_input",
            Self::InvalidState => "summary_invalid_state",
            Self::MeetingInvalidState => "meeting_invalid_state",
            Self::MeetingNotFound => "meeting_not_found",
            Self::PersistenceFailed => "summary_persistence_failed",
            Self::ProviderFailed => "summary_provider_failed",
            Self::ProviderOutputInvalid => "summary_provider_output_invalid",
            Self::RunNotFound => "summary_run_not_found",
            Self::SegmentLimitExceeded => "summary_segment_limit_exceeded",
            Self::SnapshotMissing => "summary_snapshot_missing",
            Self::SourceNotFound => "summary_source_not_found",
            Self::UnknownEvidence => "summary_unknown_evidence",
        }
    }

    pub const fn safe_message(self) -> &'static str {
        match self {
            Self::InputInsufficient => {
                "The selected sources do not contain usable final transcript segments."
            }
            Self::InputTooLarge => "The selected summary input is too large.",
            Self::Internal => "An internal error occurred.",
            Self::InvalidInput => "The summary request is invalid.",
            Self::InvalidState => "The summary run cannot be changed in its current state.",
            Self::MeetingInvalidState => "The meeting cannot be summarized in its current state.",
            Self::MeetingNotFound => "The meeting was not found.",
            Self::PersistenceFailed => "The summary run could not be saved.",
            Self::ProviderFailed => "The summary provider could not complete the run.",
            Self::ProviderOutputInvalid => "The summary provider returned an invalid result.",
            Self::RunNotFound => "The summary run was not found.",
            Self::SegmentLimitExceeded => "The selected summary input contains too many segments.",
            Self::SnapshotMissing => "The summary input snapshot is unavailable.",
            Self::SourceNotFound => "A selected transcript source was not found.",
            Self::UnknownEvidence => "The summary result referenced unknown evidence.",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::PersistenceFailed | Self::ProviderFailed | Self::SnapshotMissing
        )
    }
}

impl fmt::Display for SummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SummaryError {}

impl From<rusqlite::Error> for SummaryError {
    fn from(_: rusqlite::Error) -> Self {
        Self::PersistenceFailed
    }
}

impl From<serde_json::Error> for SummaryError {
    fn from(_: serde_json::Error) -> Self {
        Self::PersistenceFailed
    }
}

pub type SummaryResult<T> = Result<T, SummaryError>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn summary_errors_have_unique_safe_protocol_codes() {
        let errors = [
            SummaryError::InputInsufficient,
            SummaryError::InputTooLarge,
            SummaryError::Internal,
            SummaryError::InvalidInput,
            SummaryError::InvalidState,
            SummaryError::MeetingInvalidState,
            SummaryError::MeetingNotFound,
            SummaryError::PersistenceFailed,
            SummaryError::ProviderFailed,
            SummaryError::ProviderOutputInvalid,
            SummaryError::RunNotFound,
            SummaryError::SegmentLimitExceeded,
            SummaryError::SnapshotMissing,
            SummaryError::SourceNotFound,
            SummaryError::UnknownEvidence,
        ];
        let mut codes = HashSet::new();

        for error in errors {
            assert!(codes.insert(error.code()));
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
