use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_dialog::DialogExt;

use crate::audio_transcription_error::{AudioTranscriptionError, AudioTranscriptionErrorCode};
use crate::audio_transcription_runtime::{
    AudioTranscriptionPhase, AudioTranscriptionRuntime, AudioTranscriptionSnapshot,
    AudioTranscriptionStatus, SUPPORTED_AUDIO_EXTENSIONS,
};
use crate::audio_transcription_worker::{
    AUDIO_MAX_DURATION_MS, AUDIO_MAX_FILE_BYTES, AUDIO_MAX_SEGMENTS, AUDIO_MODEL_ID,
    AUDIO_MODEL_REVISION,
};
use crate::data::CreateMeeting;
use crate::meeting_commands::MeetingResponse;
use crate::transcript_commands::TranscriptSourceResponse;

#[cfg(test)]
const AUDIO_TRANSCRIPTION_COMMAND_NAMES: &[&str] = &[
    "get_audio_transcription_policy",
    "get_audio_transcription_status",
    "select_audio_and_create_meeting",
    "select_and_add_audio_to_meeting",
    "cancel_audio_transcription",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectAudioAndCreateMeetingRequest {
    pub title: String,
    pub primary_language: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectAndAddAudioToMeetingRequest {
    pub meeting_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioTranscriptionJobRequest {
    pub job_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTranscriptionPolicyResponse {
    pub supported_extensions: Vec<&'static str>,
    pub max_file_bytes: u64,
    pub max_duration_ms: u64,
    pub max_segments: usize,
    pub device: &'static str,
    pub compute_type: &'static str,
    pub model: &'static str,
    pub model_revision: &'static str,
    pub worker_available: bool,
    pub model_available: bool,
    pub busy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTranscriptionStatusResponse {
    pub job_id: String,
    pub phase: AudioTranscriptionPhase,
    pub status: AudioTranscriptionStatus,
    pub bytes_processed: u64,
    pub total_bytes: Option<u64>,
    pub processed_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<&'static str>,
}

impl From<AudioTranscriptionSnapshot> for AudioTranscriptionStatusResponse {
    fn from(snapshot: AudioTranscriptionSnapshot) -> Self {
        Self {
            job_id: snapshot.job_id,
            phase: snapshot.phase,
            status: snapshot.status,
            bytes_processed: snapshot.bytes_processed,
            total_bytes: snapshot.total_bytes,
            processed_ms: snapshot.processed_ms,
            duration_ms: snapshot.duration_ms,
            error_code: snapshot.error_code.map(AudioTranscriptionErrorCode::as_str),
        }
    }
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTranscriptionResultResponse {
    pub cancelled: bool,
    pub job_id: Option<String>,
    pub meeting: Option<MeetingResponse>,
    pub source: Option<TranscriptSourceResponse>,
    pub segment_count: u64,
}

impl std::fmt::Debug for AudioTranscriptionResultResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioTranscriptionResultResponse")
            .field("cancelled", &self.cancelled)
            .field("job_id", &self.job_id)
            .field(
                "meeting_id",
                &self.meeting.as_ref().map(|meeting| &meeting.id),
            )
            .field("source_id", &self.source.as_ref().map(|source| &source.id))
            .field("segment_count", &self.segment_count)
            .finish()
    }
}

impl AudioTranscriptionResultResponse {
    fn picker_cancelled() -> Self {
        Self {
            cancelled: true,
            job_id: None,
            meeting: None,
            source: None,
            segment_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTranscriptionCommandError {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl From<AudioTranscriptionError> for AudioTranscriptionCommandError {
    fn from(error: AudioTranscriptionError) -> Self {
        Self {
            code: error.code.as_str(),
            message: error.code.safe_message(),
            retryable: error.code.retryable(),
        }
    }
}

type CommandResult<T> = Result<T, AudioTranscriptionCommandError>;

trait AudioPicker {
    fn pick_file(&self) -> CommandResult<Option<PathBuf>>;
}

struct NativeAudioPicker<'a> {
    app: &'a AppHandle,
}

impl AudioPicker for NativeAudioPicker<'_> {
    fn pick_file(&self) -> CommandResult<Option<PathBuf>> {
        self.app
            .dialog()
            .file()
            .set_title("Select a meeting recording")
            .add_filter("Meeting recordings", SUPPORTED_AUDIO_EXTENSIONS)
            .blocking_pick_file()
            .map(|file| {
                file.into_path().map_err(|_| {
                    AudioTranscriptionCommandError::from(AudioTranscriptionError::new(
                        AudioTranscriptionErrorCode::AudioPickerUnavailable,
                    ))
                })
            })
            .transpose()
    }
}

#[tauri::command]
pub fn get_audio_transcription_policy(
    runtime: State<'_, AudioTranscriptionRuntime>,
) -> AudioTranscriptionPolicyResponse {
    let policy = runtime.policy();
    AudioTranscriptionPolicyResponse {
        supported_extensions: SUPPORTED_AUDIO_EXTENSIONS.to_vec(),
        max_file_bytes: AUDIO_MAX_FILE_BYTES,
        max_duration_ms: AUDIO_MAX_DURATION_MS,
        max_segments: AUDIO_MAX_SEGMENTS,
        device: "cpu",
        compute_type: "int8",
        model: AUDIO_MODEL_ID,
        model_revision: AUDIO_MODEL_REVISION,
        worker_available: policy.worker_available,
        model_available: policy.model_available,
        busy: policy.busy,
    }
}

#[tauri::command]
pub fn get_audio_transcription_status(
    request: AudioTranscriptionJobRequest,
    runtime: State<'_, AudioTranscriptionRuntime>,
) -> CommandResult<AudioTranscriptionStatusResponse> {
    runtime
        .status(&request.job_id)
        .map(AudioTranscriptionStatusResponse::from)
        .map_err(AudioTranscriptionCommandError::from)
}

#[tauri::command]
pub fn cancel_audio_transcription(
    request: AudioTranscriptionJobRequest,
    runtime: State<'_, AudioTranscriptionRuntime>,
) -> CommandResult<AudioTranscriptionStatusResponse> {
    runtime
        .cancel(&request.job_id)
        .map(AudioTranscriptionStatusResponse::from)
        .map_err(AudioTranscriptionCommandError::from)
}

#[tauri::command]
pub async fn select_audio_and_create_meeting(
    request: SelectAudioAndCreateMeetingRequest,
    on_progress: Channel<AudioTranscriptionStatusResponse>,
    app: AppHandle,
    runtime: State<'_, AudioTranscriptionRuntime>,
) -> CommandResult<AudioTranscriptionResultResponse> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picker = NativeAudioPicker { app: &app };
        select_and_create_with_picker(&runtime, request, &picker, |progress| {
            let _ = on_progress.send(AudioTranscriptionStatusResponse::from(progress));
        })
    })
    .await
    .map_err(|_| {
        AudioTranscriptionCommandError::from(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::InternalError,
        ))
    })?
}

#[tauri::command]
pub async fn select_and_add_audio_to_meeting(
    request: SelectAndAddAudioToMeetingRequest,
    on_progress: Channel<AudioTranscriptionStatusResponse>,
    app: AppHandle,
    runtime: State<'_, AudioTranscriptionRuntime>,
) -> CommandResult<AudioTranscriptionResultResponse> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picker = NativeAudioPicker { app: &app };
        select_and_add_with_picker(&runtime, request, &picker, |progress| {
            let _ = on_progress.send(AudioTranscriptionStatusResponse::from(progress));
        })
    })
    .await
    .map_err(|_| {
        AudioTranscriptionCommandError::from(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::InternalError,
        ))
    })?
}

fn select_and_create_with_picker(
    runtime: &AudioTranscriptionRuntime,
    request: SelectAudioAndCreateMeetingRequest,
    picker: &dyn AudioPicker,
    on_progress: impl FnMut(AudioTranscriptionSnapshot),
) -> CommandResult<AudioTranscriptionResultResponse> {
    let Some(path) = picker.pick_file()? else {
        return Ok(AudioTranscriptionResultResponse::picker_cancelled());
    };
    runtime
        .transcribe_new_meeting(
            CreateMeeting {
                title: request.title,
                primary_language: request.primary_language,
            },
            path,
            on_progress,
        )
        .map(complete_result)
        .map_err(AudioTranscriptionCommandError::from)
}

fn select_and_add_with_picker(
    runtime: &AudioTranscriptionRuntime,
    request: SelectAndAddAudioToMeetingRequest,
    picker: &dyn AudioPicker,
    on_progress: impl FnMut(AudioTranscriptionSnapshot),
) -> CommandResult<AudioTranscriptionResultResponse> {
    let Some(path) = picker.pick_file()? else {
        return Ok(AudioTranscriptionResultResponse::picker_cancelled());
    };
    runtime
        .transcribe_existing_meeting(request.meeting_id, path, on_progress)
        .map(complete_result)
        .map_err(AudioTranscriptionCommandError::from)
}

fn complete_result(
    result: crate::audio_transcription_runtime::AudioTranscriptionRunOutcome,
) -> AudioTranscriptionResultResponse {
    let segment_count = result.outcome.source.segment_count;
    AudioTranscriptionResultResponse {
        cancelled: false,
        job_id: Some(result.job_id),
        meeting: Some(MeetingResponse::from(result.outcome.meeting)),
        source: Some(TranscriptSourceResponse::from_overview(
            result.outcome.source,
        )),
        segment_count,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::data::MeetingService;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("meetinglens-audio-command-{}", Uuid::new_v4())))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct FakePicker(CommandResult<Option<PathBuf>>);

    impl AudioPicker for FakePicker {
        fn pick_file(&self) -> CommandResult<Option<PathBuf>> {
            self.0.clone()
        }
    }

    #[test]
    fn picker_cancellation_has_no_database_or_job_side_effect() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should open");
        let runtime = AudioTranscriptionRuntime::initialize(meetings.clone());
        let result = select_and_create_with_picker(
            &runtime,
            SelectAudioAndCreateMeetingRequest {
                title: String::new(),
                primary_language: String::new(),
            },
            &FakePicker(Ok(None)),
            |_| {},
        )
        .expect("picker cancel should succeed");
        assert!(result.cancelled);
        assert!(result.job_id.is_none());
        assert!(meetings.list().expect("meetings should list").is_empty());
        assert!(!runtime.policy().busy);
    }

    #[test]
    fn policy_and_status_dtos_use_the_strict_public_shape() {
        let value = serde_json::to_value(AudioTranscriptionPolicyResponse {
            supported_extensions: SUPPORTED_AUDIO_EXTENSIONS.to_vec(),
            max_file_bytes: AUDIO_MAX_FILE_BYTES,
            max_duration_ms: AUDIO_MAX_DURATION_MS,
            max_segments: AUDIO_MAX_SEGMENTS,
            device: "cpu",
            compute_type: "int8",
            model: AUDIO_MODEL_ID,
            model_revision: AUDIO_MODEL_REVISION,
            worker_available: false,
            model_available: false,
            busy: false,
        })
        .expect("policy should serialize");
        assert_eq!(
            value["supportedExtensions"],
            json!(["wav", "mp3", "m4a", "mp4"])
        );
        assert_eq!(value["device"], "cpu");
        assert!(value.get("workerPath").is_none());
        assert!(value.get("modelPath").is_none());

        let status = serde_json::to_value(AudioTranscriptionStatusResponse::from(
            AudioTranscriptionSnapshot {
                job_id: Uuid::new_v4().to_string(),
                phase: AudioTranscriptionPhase::Transcribing,
                status: AudioTranscriptionStatus::Running,
                bytes_processed: 12,
                total_bytes: Some(24),
                processed_ms: Some(1000),
                duration_ms: Some(2000),
                error_code: None,
            },
        ))
        .expect("status should serialize");
        assert_eq!(status["phase"], "transcribing");
        assert!(status.get("path").is_none());
        assert!(status.get("text").is_none());
    }

    #[test]
    fn errors_are_stable_and_redacted() {
        let error = AudioTranscriptionCommandError::from(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioWorkerCrashed,
        ));
        let serialized = serde_json::to_string(&error).expect("error should serialize");
        assert_eq!(error.code, "audio_worker_crashed");
        for forbidden in ["c:\\\\users\\\\", "sqlite", "traceback", "authorization"] {
            assert!(!serialized.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn command_layer_is_registered_and_contains_no_arbitrary_file_api() {
        let app_builder = include_str!("lib.rs");
        for command in AUDIO_TRANSCRIPTION_COMMAND_NAMES {
            assert!(app_builder.contains(&format!("audio_transcription_commands::{command}")));
        }
        let source = include_str!("audio_transcription_commands.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
            .to_ascii_lowercase();
        assert!(!production.contains("read_to_end"));
        assert!(!production.contains("std::fs"));
        assert!(!production.contains("remove_file"));
        assert!(!production.contains("remove_dir"));
    }
}
