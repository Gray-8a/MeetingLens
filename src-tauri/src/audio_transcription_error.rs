use std::fmt;

use serde::{Deserialize, Serialize};

use crate::data::{DataError, TranscriptError, TranscriptWorkflowError};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTranscriptionErrorCode {
    AudioPickerUnavailable,
    AudioInvalidInput,
    AudioUnsupportedType,
    AudioTypeMismatch,
    AudioEmpty,
    AudioTooLarge,
    AudioTooLong,
    AudioStreamMissing,
    AudioModelMissing,
    AudioModelIncompatible,
    AudioWorkerUnavailable,
    AudioWorkerStartFailed,
    AudioWorkerCrashed,
    AudioProtocolError,
    AudioOutputTooLarge,
    AudioIntegrityFailed,
    AudioTimeout,
    AudioCancelled,
    AudioTranscriptionBusy,
    AudioPersistenceFailed,
    MeetingNotFound,
    MeetingInvalidState,
    InternalError,
}

impl AudioTranscriptionErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AudioPickerUnavailable => "audio_picker_unavailable",
            Self::AudioInvalidInput => "audio_invalid_input",
            Self::AudioUnsupportedType => "audio_unsupported_type",
            Self::AudioTypeMismatch => "audio_type_mismatch",
            Self::AudioEmpty => "audio_empty",
            Self::AudioTooLarge => "audio_too_large",
            Self::AudioTooLong => "audio_too_long",
            Self::AudioStreamMissing => "audio_stream_missing",
            Self::AudioModelMissing => "audio_model_missing",
            Self::AudioModelIncompatible => "audio_model_incompatible",
            Self::AudioWorkerUnavailable => "audio_worker_unavailable",
            Self::AudioWorkerStartFailed => "audio_worker_start_failed",
            Self::AudioWorkerCrashed => "audio_worker_crashed",
            Self::AudioProtocolError => "audio_protocol_error",
            Self::AudioOutputTooLarge => "audio_output_too_large",
            Self::AudioIntegrityFailed => "audio_integrity_failed",
            Self::AudioTimeout => "audio_timeout",
            Self::AudioCancelled => "audio_cancelled",
            Self::AudioTranscriptionBusy => "audio_transcription_busy",
            Self::AudioPersistenceFailed => "audio_persistence_failed",
            Self::MeetingNotFound => "meeting_not_found",
            Self::MeetingInvalidState => "meeting_invalid_state",
            Self::InternalError => "internal_error",
        }
    }

    pub const fn safe_message(self) -> &'static str {
        match self {
            Self::AudioPickerUnavailable => "The audio file picker is unavailable.",
            Self::AudioInvalidInput => "The selected media file is invalid.",
            Self::AudioUnsupportedType => "This media type is not supported.",
            Self::AudioTypeMismatch => "The selected file does not match its reported media type.",
            Self::AudioEmpty => "The selected media file is empty.",
            Self::AudioTooLarge => "The selected media file exceeds the size limit.",
            Self::AudioTooLong => "The selected recording exceeds the duration limit.",
            Self::AudioStreamMissing => {
                "The selected media does not contain a supported audio stream."
            }
            Self::AudioModelMissing => "The required local transcription model is missing.",
            Self::AudioModelIncompatible => "The local transcription model is incompatible.",
            Self::AudioWorkerUnavailable => "The local transcription worker is unavailable.",
            Self::AudioWorkerStartFailed => "The local transcription worker could not start.",
            Self::AudioWorkerCrashed => "The local transcription worker stopped unexpectedly.",
            Self::AudioProtocolError => {
                "The local transcription worker returned an invalid response."
            }
            Self::AudioOutputTooLarge => "The transcription output exceeds the safety limit.",
            Self::AudioIntegrityFailed => "A transcription integrity check failed.",
            Self::AudioTimeout => "Local transcription timed out.",
            Self::AudioCancelled => "Local transcription was cancelled.",
            Self::AudioTranscriptionBusy => "Another local transcription is already running.",
            Self::AudioPersistenceFailed => "The transcription could not be saved.",
            Self::MeetingNotFound => "The meeting was not found.",
            Self::MeetingInvalidState => "The meeting cannot be changed in its current state.",
            Self::InternalError => "An internal error occurred.",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::AudioPickerUnavailable
                | Self::AudioWorkerUnavailable
                | Self::AudioWorkerStartFailed
                | Self::AudioWorkerCrashed
                | Self::AudioTimeout
                | Self::AudioTranscriptionBusy
                | Self::AudioPersistenceFailed
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTranscriptionError {
    pub code: AudioTranscriptionErrorCode,
}

impl AudioTranscriptionError {
    pub const fn new(code: AudioTranscriptionErrorCode) -> Self {
        Self { code }
    }
}

impl fmt::Display for AudioTranscriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.as_str())
    }
}

impl std::error::Error for AudioTranscriptionError {}

impl From<TranscriptError> for AudioTranscriptionError {
    fn from(error: TranscriptError) -> Self {
        let code = match error {
            TranscriptError::MeetingNotFound => AudioTranscriptionErrorCode::MeetingNotFound,
            TranscriptError::MeetingInvalidState | TranscriptError::InvalidState => {
                AudioTranscriptionErrorCode::MeetingInvalidState
            }
            TranscriptError::Empty | TranscriptError::InputInsufficient => {
                AudioTranscriptionErrorCode::AudioInvalidInput
            }
            TranscriptError::SegmentLimitExceeded
            | TranscriptError::SegmentTooLarge
            | TranscriptError::TooLarge => AudioTranscriptionErrorCode::AudioOutputTooLarge,
            TranscriptError::PersistenceFailed | TranscriptError::AlreadyImported { .. } => {
                AudioTranscriptionErrorCode::AudioPersistenceFailed
            }
            TranscriptError::InvalidEncoding
            | TranscriptError::InvalidFormat
            | TranscriptError::InvalidInput
            | TranscriptError::InvalidTimestamp
            | TranscriptError::RevisionConflict
            | TranscriptError::SegmentNotFound
            | TranscriptError::SourceInUse
            | TranscriptError::SourceNotFound
            | TranscriptError::Internal => AudioTranscriptionErrorCode::InternalError,
        };
        Self::new(code)
    }
}

impl From<TranscriptWorkflowError> for AudioTranscriptionError {
    fn from(error: TranscriptWorkflowError) -> Self {
        match error {
            TranscriptWorkflowError::Transcript(error) => error.into(),
            TranscriptWorkflowError::Meeting(error) => Self::from(error),
        }
    }
}

impl From<DataError> for AudioTranscriptionError {
    fn from(error: DataError) -> Self {
        let code = match error {
            DataError::NotFound | DataError::InvalidId => {
                AudioTranscriptionErrorCode::MeetingNotFound
            }
            DataError::InvalidState
            | DataError::MeetingDeleteFailed
            | DataError::MeetingDeletePending => AudioTranscriptionErrorCode::MeetingInvalidState,
            DataError::Database | DataError::MigrationConflict | DataError::StorageUnavailable => {
                AudioTranscriptionErrorCode::AudioPersistenceFailed
            }
            DataError::InvalidInput | DataError::UnsafePath => {
                AudioTranscriptionErrorCode::InternalError
            }
        };
        Self::new(code)
    }
}

pub type AudioTranscriptionResult<T> = Result<T, AudioTranscriptionError>;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn stable_errors_are_unique_and_safe() {
        let codes = [
            AudioTranscriptionErrorCode::AudioPickerUnavailable,
            AudioTranscriptionErrorCode::AudioInvalidInput,
            AudioTranscriptionErrorCode::AudioUnsupportedType,
            AudioTranscriptionErrorCode::AudioTypeMismatch,
            AudioTranscriptionErrorCode::AudioEmpty,
            AudioTranscriptionErrorCode::AudioTooLarge,
            AudioTranscriptionErrorCode::AudioTooLong,
            AudioTranscriptionErrorCode::AudioStreamMissing,
            AudioTranscriptionErrorCode::AudioModelMissing,
            AudioTranscriptionErrorCode::AudioModelIncompatible,
            AudioTranscriptionErrorCode::AudioWorkerUnavailable,
            AudioTranscriptionErrorCode::AudioWorkerStartFailed,
            AudioTranscriptionErrorCode::AudioWorkerCrashed,
            AudioTranscriptionErrorCode::AudioProtocolError,
            AudioTranscriptionErrorCode::AudioOutputTooLarge,
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
            AudioTranscriptionErrorCode::AudioTimeout,
            AudioTranscriptionErrorCode::AudioCancelled,
            AudioTranscriptionErrorCode::AudioTranscriptionBusy,
            AudioTranscriptionErrorCode::AudioPersistenceFailed,
            AudioTranscriptionErrorCode::MeetingNotFound,
            AudioTranscriptionErrorCode::MeetingInvalidState,
            AudioTranscriptionErrorCode::InternalError,
        ];
        let mut unique = HashSet::new();
        for code in codes {
            assert!(unique.insert(code.as_str()));
            let safe = format!("{} {}", code.as_str(), code.safe_message()).to_ascii_lowercase();
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
