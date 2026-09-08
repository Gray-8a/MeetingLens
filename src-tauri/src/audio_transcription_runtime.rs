use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::audio_transcription_error::{
    AudioTranscriptionError, AudioTranscriptionErrorCode, AudioTranscriptionResult,
};
use crate::audio_transcription_worker::{
    AUDIO_MAX_FILE_BYTES, AudioCancellationToken, AudioWorkerClient, AudioWorkerConfig,
    AudioWorkerProgress, AudioWorkerRequest, AudioWorkerStage, ValidatedAudioTranscript,
};
use crate::data::{
    CreateMeeting, MeetingService, StructuredAudioSegment, StructuredAudioTranscript,
    TranscriptImportWorkflowOutcome,
};

const AUDIO_STAGING_DIRECTORY: &str = "audio-transcription-staging";
const AUDIO_MODEL_DIRECTORY: &str =
    "MeetingLens/transcription/models/small-536b0662742c02347bc0e980a01041f333bce120";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const COPY_PROGRESS_INTERVAL_BYTES: u64 = 4 * 1024 * 1024;
const SAFE_FILENAME_MAX_BYTES: usize = 255;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(30);

pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "mp4"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTranscriptionPhase {
    Selected,
    ValidatingInput,
    Copying,
    Probing,
    ModelLoading,
    Transcribing,
    ValidatingOutput,
    Committing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTranscriptionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTranscriptionSnapshot {
    pub job_id: String,
    pub phase: AudioTranscriptionPhase,
    pub status: AudioTranscriptionStatus,
    pub bytes_processed: u64,
    pub total_bytes: Option<u64>,
    pub processed_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<AudioTranscriptionErrorCode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTranscriptionPolicy {
    pub worker_available: bool,
    pub model_available: bool,
    pub busy: bool,
}

#[derive(Clone, PartialEq)]
pub struct AudioTranscriptionRunOutcome {
    pub job_id: String,
    pub outcome: TranscriptImportWorkflowOutcome,
}

impl std::fmt::Debug for AudioTranscriptionRunOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioTranscriptionRunOutcome")
            .field("job_id", &self.job_id)
            .field("meeting_id", &self.outcome.meeting.id)
            .field("source_id", &self.outcome.source.source.id)
            .finish()
    }
}

#[derive(Clone)]
pub struct AudioTranscriptionRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    meetings: MeetingService,
    staging_root: PathBuf,
    registry: Mutex<JobRegistry>,
    changed: Condvar,
}

struct JobRegistry {
    active: Option<ActiveJob>,
    last: Option<AudioTranscriptionSnapshot>,
}

struct ActiveJob {
    job_id: String,
    meeting_id: Option<String>,
    cancellation: AudioCancellationToken,
    commit_started: bool,
    snapshot: AudioTranscriptionSnapshot,
}

enum JobTarget {
    Create(CreateMeeting),
    Existing(String),
}

struct SelectedAudio {
    original_filename: String,
    safe_extension: String,
    detected_media_type: String,
    metadata: std::fs::Metadata,
    identity: SourceFileIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceFileIdentity {
    volume_or_device: u64,
    file_index: u64,
}

struct StagedAudio {
    input_root: PathBuf,
    input_path: PathBuf,
    output_parent: PathBuf,
    output_root: PathBuf,
    working_directory: PathBuf,
    size_bytes: u64,
    sha256: String,
}

impl AudioTranscriptionRuntime {
    pub fn initialize(meetings: MeetingService) -> Self {
        let staging_root = meetings
            .storage_paths()
            .root()
            .join(AUDIO_STAGING_DIRECTORY);
        let runtime = Self {
            inner: Arc::new(RuntimeInner {
                meetings,
                staging_root,
                registry: Mutex::new(JobRegistry {
                    active: None,
                    last: None,
                }),
                changed: Condvar::new(),
            }),
        };
        runtime.reconcile_staging();
        runtime
    }

    pub fn policy(&self) -> AudioTranscriptionPolicy {
        let worker_available = resolve_python_executable().is_some();
        let model_available = resolve_models_root().is_some_and(|root| model_files_exist(&root));
        let busy = self
            .inner
            .registry
            .lock()
            .map(|registry| registry.active.is_some())
            .unwrap_or(true);
        AudioTranscriptionPolicy {
            worker_available,
            model_available,
            busy,
        }
    }

    pub fn status(&self, job_id: &str) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        validate_uuid(job_id)?;
        let registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        if let Some(active) = &registry.active
            && active.job_id == job_id
        {
            return Ok(active.snapshot.clone());
        }
        if let Some(last) = &registry.last
            && last.job_id == job_id
        {
            return Ok(last.clone());
        }
        Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ))
    }

    pub fn cancel(&self, job_id: &str) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        validate_uuid(job_id)?;
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        if let Some(active) = registry.active.as_mut()
            && active.job_id == job_id
        {
            if active.commit_started {
                return Ok(active.snapshot.clone());
            }
            active.cancellation.cancel();
            active.snapshot.phase = AudioTranscriptionPhase::Cancelled;
            active.snapshot.status = AudioTranscriptionStatus::Cancelled;
            active.snapshot.error_code = Some(AudioTranscriptionErrorCode::AudioCancelled);
            return Ok(active.snapshot.clone());
        }
        if let Some(last) = &registry.last
            && last.job_id == job_id
        {
            return Ok(last.clone());
        }
        Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ))
    }

    pub fn transcribe_new_meeting(
        &self,
        meeting: CreateMeeting,
        selected_path: PathBuf,
        on_progress: impl FnMut(AudioTranscriptionSnapshot),
    ) -> AudioTranscriptionResult<AudioTranscriptionRunOutcome> {
        self.run_job(JobTarget::Create(meeting), selected_path, on_progress)
    }

    pub fn transcribe_existing_meeting(
        &self,
        meeting_id: String,
        selected_path: PathBuf,
        on_progress: impl FnMut(AudioTranscriptionSnapshot),
    ) -> AudioTranscriptionResult<AudioTranscriptionRunOutcome> {
        self.run_job(JobTarget::Existing(meeting_id), selected_path, on_progress)
    }

    pub fn cancel_for_meeting_and_wait(&self, meeting_id: &str) -> AudioTranscriptionResult<()> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let should_wait = registry
            .active
            .as_ref()
            .is_some_and(|active| active.meeting_id.as_deref() == Some(meeting_id));
        if !should_wait {
            return Ok(());
        }
        if let Some(active) = registry.active.as_mut() {
            active.cancellation.cancel();
        }
        let (registry, timeout) = self
            .inner
            .changed
            .wait_timeout_while(registry, SHUTDOWN_WAIT, |state| {
                state
                    .active
                    .as_ref()
                    .is_some_and(|active| active.meeting_id.as_deref() == Some(meeting_id))
            })
            .map_err(|_| internal_error())?;
        if timeout.timed_out()
            && registry
                .active
                .as_ref()
                .is_some_and(|active| active.meeting_id.as_deref() == Some(meeting_id))
        {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioPersistenceFailed,
            ));
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        let Ok(mut registry) = self.inner.registry.lock() else {
            return;
        };
        if let Some(active) = registry.active.as_mut() {
            active.cancellation.cancel();
        } else {
            return;
        }
        let _ = self
            .inner
            .changed
            .wait_timeout_while(registry, SHUTDOWN_WAIT, |state| state.active.is_some());
    }

    fn run_job(
        &self,
        target: JobTarget,
        selected_path: PathBuf,
        mut on_progress: impl FnMut(AudioTranscriptionSnapshot),
    ) -> AudioTranscriptionResult<AudioTranscriptionRunOutcome> {
        let meeting_id = match &target {
            JobTarget::Create(_) => None,
            JobTarget::Existing(meeting_id) => Some(meeting_id.clone()),
        };
        let (job_id, cancellation) = self.begin_job(meeting_id)?;
        let selected = self.update_progress(
            &job_id,
            AudioTranscriptionPhase::Selected,
            AudioTranscriptionStatus::Running,
            0,
            None,
            None,
            None,
            None,
        )?;
        on_progress(selected);

        let result = self.execute_job(
            &job_id,
            &cancellation,
            &target,
            &selected_path,
            &mut on_progress,
        );
        let cleanup = self.cleanup_job(&job_id);
        let result = match (result, cleanup) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        };
        match result {
            Ok(outcome) => {
                let completed = self.finish_job(
                    &job_id,
                    AudioTranscriptionPhase::Completed,
                    AudioTranscriptionStatus::Completed,
                    None,
                )?;
                on_progress(completed);
                Ok(AudioTranscriptionRunOutcome { job_id, outcome })
            }
            Err(error) => {
                let cancelled = error.code == AudioTranscriptionErrorCode::AudioCancelled;
                let final_snapshot = self.finish_job(
                    &job_id,
                    if cancelled {
                        AudioTranscriptionPhase::Cancelled
                    } else {
                        AudioTranscriptionPhase::Failed
                    },
                    if cancelled {
                        AudioTranscriptionStatus::Cancelled
                    } else {
                        AudioTranscriptionStatus::Failed
                    },
                    Some(error.code),
                )?;
                on_progress(final_snapshot);
                Err(error)
            }
        }
    }

    fn execute_job(
        &self,
        job_id: &str,
        cancellation: &AudioCancellationToken,
        target: &JobTarget,
        source_path: &Path,
        on_progress: &mut impl FnMut(AudioTranscriptionSnapshot),
    ) -> AudioTranscriptionResult<TranscriptImportWorkflowOutcome> {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        if let JobTarget::Existing(meeting_id) = target {
            self.inner.meetings.get(meeting_id)?;
        }
        let validating = self.update_progress(
            job_id,
            AudioTranscriptionPhase::ValidatingInput,
            AudioTranscriptionStatus::Running,
            0,
            None,
            None,
            None,
            None,
        )?;
        on_progress(validating);
        let selected = validate_selected_audio(source_path)?;
        let total_bytes = selected.metadata.len();
        let staged = self.stage_audio(job_id, source_path, &selected, cancellation, |bytes| {
            if let Ok(snapshot) = self.update_progress(
                job_id,
                AudioTranscriptionPhase::Copying,
                AudioTranscriptionStatus::Running,
                bytes,
                Some(total_bytes),
                None,
                None,
                None,
            ) {
                on_progress(snapshot);
            }
        })?;
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let probing = self.update_progress(
            job_id,
            AudioTranscriptionPhase::Probing,
            AudioTranscriptionStatus::Running,
            staged.size_bytes,
            Some(staged.size_bytes),
            None,
            None,
            None,
        )?;
        on_progress(probing);

        let python = resolve_python_executable().ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerUnavailable)
        })?;
        let models_root = resolve_models_root()
            .filter(|root| model_files_exist(root))
            .ok_or_else(|| {
                AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioModelMissing)
            })?;
        let config = AudioWorkerConfig::new(
            python,
            &staged.working_directory,
            models_root,
            &staged.output_parent,
        )?;
        let request = AudioWorkerRequest::new(
            job_id.to_string(),
            staged.input_path.clone(),
            staged.input_root.clone(),
            staged.output_root.clone(),
            staged.sha256.clone(),
            staged.size_bytes,
            selected.detected_media_type.clone(),
            selected.safe_extension.clone(),
        );
        let client = AudioWorkerClient::new(config);
        let validated = client.transcribe(&request, cancellation, |progress| {
            if let Ok(snapshot) = self.worker_progress(job_id, staged.size_bytes, progress) {
                on_progress(snapshot);
            }
        })?;
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let validating_output = self.update_progress(
            job_id,
            AudioTranscriptionPhase::ValidatingOutput,
            AudioTranscriptionStatus::Running,
            staged.size_bytes,
            Some(staged.size_bytes),
            Some(validated.duration_ms),
            Some(validated.duration_ms),
            None,
        )?;
        on_progress(validating_output);

        let transcript = structured_transcript(selected, validated);
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        self.cleanup_job(job_id)?;
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let committing = self.begin_commit(job_id, staged.size_bytes)?;
        on_progress(committing);
        match target {
            JobTarget::Create(meeting) => self
                .inner
                .meetings
                .transcript_imports()
                .create_meeting_from_audio_transcript(meeting.clone(), transcript)
                .map_err(AudioTranscriptionError::from),
            JobTarget::Existing(meeting_id) => self
                .inner
                .meetings
                .transcript_imports()
                .add_audio_transcript_to_meeting(meeting_id, transcript)
                .map_err(AudioTranscriptionError::from),
        }
    }

    fn stage_audio(
        &self,
        job_id: &str,
        source_path: &Path,
        selected: &SelectedAudio,
        cancellation: &AudioCancellationToken,
        on_progress: impl FnMut(u64),
    ) -> AudioTranscriptionResult<StagedAudio> {
        ensure_private_directory(&self.inner.staging_root)?;
        let inputs_parent = self.inner.staging_root.join("inputs");
        let outputs_parent = self.inner.staging_root.join("outputs");
        let work_parent = self.inner.staging_root.join("work");
        for directory in [&inputs_parent, &outputs_parent, &work_parent] {
            ensure_private_directory(directory)?;
        }
        let input_root = inputs_parent.join(job_id);
        let output_root = outputs_parent.join(job_id);
        let working_directory = work_parent.join(job_id);
        for directory in [&input_root, &output_root, &working_directory] {
            std::fs::create_dir(directory).map_err(|_| persistence_error())?;
        }
        let input_path = input_root.join(format!("input.{}", selected.safe_extension));
        let (size_bytes, sha256) = copy_and_hash(
            source_path,
            &input_path,
            selected.metadata.clone(),
            selected.identity,
            cancellation,
            on_progress,
        )?;
        let staged_selected = validate_selected_audio(&input_path)?;
        if staged_selected.safe_extension != selected.safe_extension
            || staged_selected.detected_media_type != selected.detected_media_type
        {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioIntegrityFailed,
            ));
        }
        Ok(StagedAudio {
            input_root,
            input_path,
            output_parent: outputs_parent,
            output_root,
            working_directory,
            size_bytes,
            sha256,
        })
    }

    fn begin_job(
        &self,
        meeting_id: Option<String>,
    ) -> AudioTranscriptionResult<(String, AudioCancellationToken)> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        if registry.active.is_some() {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioTranscriptionBusy,
            ));
        }
        let job_id = Uuid::new_v4().to_string();
        let cancellation = AudioCancellationToken::default();
        registry.active = Some(ActiveJob {
            job_id: job_id.clone(),
            meeting_id,
            cancellation: cancellation.clone(),
            commit_started: false,
            snapshot: AudioTranscriptionSnapshot {
                job_id: job_id.clone(),
                phase: AudioTranscriptionPhase::Selected,
                status: AudioTranscriptionStatus::Running,
                bytes_processed: 0,
                total_bytes: None,
                processed_ms: None,
                duration_ms: None,
                error_code: None,
            },
        });
        Ok((job_id, cancellation))
    }

    fn begin_commit(
        &self,
        job_id: &str,
        file_bytes: u64,
    ) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let active = registry.active.as_mut().ok_or_else(internal_error)?;
        if active.job_id != job_id {
            return Err(internal_error());
        }
        if active.cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        active.commit_started = true;
        active.snapshot = AudioTranscriptionSnapshot {
            job_id: job_id.to_string(),
            phase: AudioTranscriptionPhase::Committing,
            status: AudioTranscriptionStatus::Running,
            bytes_processed: file_bytes,
            total_bytes: Some(file_bytes),
            processed_ms: None,
            duration_ms: None,
            error_code: None,
        };
        Ok(active.snapshot.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn update_progress(
        &self,
        job_id: &str,
        phase: AudioTranscriptionPhase,
        status: AudioTranscriptionStatus,
        bytes_processed: u64,
        total_bytes: Option<u64>,
        processed_ms: Option<u64>,
        duration_ms: Option<u64>,
        error_code: Option<AudioTranscriptionErrorCode>,
    ) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let active = registry.active.as_mut().ok_or_else(internal_error)?;
        if active.job_id != job_id {
            return Err(internal_error());
        }
        if active.cancellation.is_cancelled() && !active.commit_started {
            return Ok(active.snapshot.clone());
        }
        let previous = &active.snapshot;
        if bytes_processed < previous.bytes_processed
            || matches!((processed_ms, duration_ms), (Some(done), Some(total)) if done > total)
        {
            return Err(internal_error());
        }
        active.snapshot = AudioTranscriptionSnapshot {
            job_id: job_id.to_string(),
            phase,
            status,
            bytes_processed,
            total_bytes,
            processed_ms,
            duration_ms,
            error_code,
        };
        Ok(active.snapshot.clone())
    }

    fn worker_progress(
        &self,
        job_id: &str,
        file_bytes: u64,
        progress: AudioWorkerProgress,
    ) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        let phase = match progress.stage {
            AudioWorkerStage::ValidatingInput => AudioTranscriptionPhase::Probing,
            AudioWorkerStage::VerifyingModel | AudioWorkerStage::LoadingModel => {
                AudioTranscriptionPhase::ModelLoading
            }
            AudioWorkerStage::Transcribing => AudioTranscriptionPhase::Transcribing,
            AudioWorkerStage::WritingArtifacts => AudioTranscriptionPhase::ValidatingOutput,
        };
        self.update_progress(
            job_id,
            phase,
            AudioTranscriptionStatus::Running,
            file_bytes,
            Some(file_bytes),
            progress.processed_ms,
            progress.total_ms,
            None,
        )
    }

    fn finish_job(
        &self,
        job_id: &str,
        phase: AudioTranscriptionPhase,
        status: AudioTranscriptionStatus,
        error_code: Option<AudioTranscriptionErrorCode>,
    ) -> AudioTranscriptionResult<AudioTranscriptionSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let mut active = registry.active.take().ok_or_else(internal_error)?;
        if active.job_id != job_id {
            registry.active = Some(active);
            return Err(internal_error());
        }
        active.snapshot.phase = phase;
        active.snapshot.status = status;
        active.snapshot.error_code = error_code;
        let snapshot = active.snapshot;
        registry.last = Some(snapshot.clone());
        self.inner.changed.notify_all();
        Ok(snapshot)
    }

    fn cleanup_job(&self, job_id: &str) -> AudioTranscriptionResult<()> {
        validate_uuid(job_id)?;
        for parent in ["inputs", "outputs", "work"] {
            cleanup_staging_job(&self.inner.staging_root, parent, job_id)?;
        }
        Ok(())
    }

    fn reconcile_staging(&self) {
        let root = &self.inner.staging_root;
        let Ok(metadata) = std::fs::symlink_metadata(root) else {
            return;
        };
        if is_redirected(&metadata) || !metadata.is_dir() {
            return;
        }
        let mut job_ids = std::collections::BTreeSet::new();
        for parent_name in ["inputs", "outputs", "work"] {
            let parent = root.join(parent_name);
            let Ok(parent_metadata) = std::fs::symlink_metadata(&parent) else {
                continue;
            };
            if !parent_metadata.is_dir() || is_redirected(&parent_metadata) {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&parent) else {
                continue;
            };
            for entry in entries.flatten() {
                if let Some(job_id) = entry.file_name().to_str()
                    && validate_uuid(job_id).is_ok()
                {
                    job_ids.insert(job_id.to_string());
                }
            }
        }
        for job_id in job_ids {
            let _ = self.cleanup_job(&job_id);
        }
    }
}

fn structured_transcript(
    selected: SelectedAudio,
    validated: ValidatedAudioTranscript,
) -> StructuredAudioTranscript {
    StructuredAudioTranscript {
        display_name: selected.original_filename.clone(),
        original_filename: selected.original_filename,
        language: validated.language,
        parser_version: validated.parser_version,
        segments: validated
            .segments
            .into_iter()
            .map(|segment| StructuredAudioSegment {
                start_offset_ms: segment.start_offset_ms,
                end_offset_ms: segment.end_offset_ms,
                original_text: segment.text,
            })
            .collect(),
    }
}

fn validate_selected_audio(path: &Path) -> AudioTranscriptionResult<SelectedAudio> {
    if !is_local_absolute_path(path) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    let original_filename = safe_filename(path)?;
    let safe_extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .filter(|value| SUPPORTED_AUDIO_EXTENSIONS.contains(&value.as_str()))
        .ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioUnsupportedType)
        })?;
    if has_dangerous_double_extension(path) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioTypeMismatch,
        ));
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| picker_error())?;
    if !metadata.file_type().is_file() || is_redirected(&metadata) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    if metadata.len() == 0 {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioEmpty,
        ));
    }
    if metadata.len() > AUDIO_MAX_FILE_BYTES {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioTooLarge,
        ));
    }
    let mut file = open_source_file(path)?;
    let opened = file.metadata().map_err(|_| picker_error())?;
    let identity = source_file_identity(&file)?;
    if !same_source_snapshot(&metadata, &opened) || is_redirected(&opened) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let mut header = [0_u8; 64];
    let read = file.read(&mut header).map_err(|_| picker_error())?;
    let detected_media_type = detect_media_type(&safe_extension, &header[..read])?;
    Ok(SelectedAudio {
        original_filename,
        safe_extension,
        detected_media_type,
        metadata,
        identity,
    })
}

fn is_local_absolute_path(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
    }
    #[cfg(not(windows))]
    true
}

fn detect_media_type(extension: &str, header: &[u8]) -> AudioTranscriptionResult<String> {
    let detected = if header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WAVE" {
        Some(("wav", "audio/wav"))
    } else if header.starts_with(b"ID3")
        || (header.len() >= 2 && header[0] == 0xff && header[1] & 0xe0 == 0xe0)
    {
        Some(("mp3", "audio/mpeg"))
    } else if header.len() >= 12 && &header[4..8] == b"ftyp" {
        Some(("mp4", "audio/mp4"))
    } else {
        None
    };
    match detected {
        Some((detected_extension, media_type))
            if detected_extension == extension
                || (matches!(extension, "m4a" | "mp4")
                    && matches!(detected_extension, "m4a" | "mp4")) =>
        {
            Ok(media_type.to_string())
        }
        Some(_) => Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioTypeMismatch,
        )),
        None => Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioTypeMismatch,
        )),
    }
}

fn copy_and_hash(
    source: &Path,
    destination: &Path,
    before: std::fs::Metadata,
    expected_identity: SourceFileIdentity,
    cancellation: &AudioCancellationToken,
    mut on_progress: impl FnMut(u64),
) -> AudioTranscriptionResult<(u64, String)> {
    let source_file = open_source_file(source)?;
    let opened = source_file.metadata().map_err(|_| picker_error())?;
    if !same_source_snapshot(&before, &opened)
        || source_file_identity(&source_file)? != expected_identity
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let destination_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| persistence_error())?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, source_file);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, destination_file);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut last_reported = 0_u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let read = reader.read(&mut buffer).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
        })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > AUDIO_MAX_FILE_BYTES {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioTooLarge,
            ));
        }
        digest.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|_| persistence_error())?;
        if total.saturating_sub(last_reported) >= COPY_PROGRESS_INTERVAL_BYTES
            || total == before.len()
        {
            on_progress(total.min(before.len()));
            last_reported = total;
        }
    }
    writer.flush().map_err(|_| persistence_error())?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| persistence_error())?;
    let handle_after = reader.get_ref().metadata().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    let after = std::fs::symlink_metadata(source).map_err(|_| picker_error())?;
    if !same_source_snapshot(&before, &handle_after)
        || !same_source_snapshot(&handle_after, &after)
        || total != after.len()
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok((total, encode_hex(&digest.finalize())))
}

fn open_source_file(path: &Path) -> AudioTranscriptionResult<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path).map_err(|_| picker_error())
}

#[cfg(windows)]
fn source_file_identity(file: &File) -> AudioTranscriptionResult<SourceFileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut information) }.map_err(
        |_| AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed),
    )?;
    Ok(SourceFileIdentity {
        volume_or_device: u64::from(information.dwVolumeSerialNumber),
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
    })
}

#[cfg(unix)]
fn source_file_identity(file: &File) -> AudioTranscriptionResult<SourceFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    Ok(SourceFileIdentity {
        volume_or_device: metadata.dev(),
        file_index: metadata.ino(),
    })
}

#[cfg(not(any(windows, unix)))]
fn source_file_identity(file: &File) -> AudioTranscriptionResult<SourceFileIdentity> {
    let metadata = file.metadata().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    Ok(SourceFileIdentity {
        volume_or_device: metadata.len(),
        file_index: metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_nanos() as u64)
            .unwrap_or_default(),
    })
}

fn safe_filename(path: &Path) -> AudioTranscriptionResult<String> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= SAFE_FILENAME_MAX_BYTES
                && !value.contains(':')
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
        })?;
    Ok(name.to_string())
}

fn has_dangerous_double_extension(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(OsStr::to_str) else {
        return true;
    };
    let nested = Path::new(stem)
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    nested.is_some_and(|value| {
        matches!(
            value.as_str(),
            "exe" | "com" | "bat" | "cmd" | "ps1" | "js" | "vbs" | "scr" | "dll" | "lnk"
        )
    })
}

fn same_source_snapshot(first: &std::fs::Metadata, second: &std::fs::Metadata) -> bool {
    first.len() == second.len()
        && first.modified().ok() == second.modified().ok()
        && first.created().ok() == second.created().ok()
}

fn model_files_exist(root: &Path) -> bool {
    root.join("manifest.json").is_file()
        && root.join("model/config.json").is_file()
        && root.join("model/model.bin").is_file()
        && root.join("model/tokenizer.json").is_file()
        && root.join("model/vocabulary.txt").is_file()
}

fn resolve_models_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join(AUDIO_MODEL_DIRECTORY))
}

fn resolve_python_executable() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let worker = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("workers/transcription");
        let worker_metadata = std::fs::symlink_metadata(&worker).ok()?;
        if !worker_metadata.is_dir() || is_redirected(&worker_metadata) {
            return None;
        }
        let worker = worker.canonicalize().ok()?;
        #[cfg(windows)]
        let python = worker.join(".venv/Scripts/python.exe");
        #[cfg(not(windows))]
        let python = worker.join(".venv/bin/python");
        let python_metadata = std::fs::symlink_metadata(&python).ok()?;
        if !python_metadata.is_file() || is_redirected(&python_metadata) {
            return None;
        }
        let python = python.canonicalize().ok()?;
        if worker.join("pyproject.toml").is_file()
            && python.is_file()
            && python.starts_with(&worker)
        {
            return Some(python);
        }
    }
    None
}

fn ensure_private_directory(path: &Path) -> AudioTranscriptionResult<()> {
    std::fs::create_dir_all(path).map_err(|_| persistence_error())?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| persistence_error())?;
    if !metadata.is_dir() || is_redirected(&metadata) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok(())
}

fn cleanup_staging_job(
    root: &Path,
    parent_name: &str,
    job_id: &str,
) -> AudioTranscriptionResult<()> {
    if !matches!(parent_name, "inputs" | "outputs" | "work") {
        return Err(persistence_error());
    }
    let root_metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(persistence_error()),
    };
    if !root_metadata.is_dir() || is_redirected(&root_metadata) {
        return Err(persistence_error());
    }
    let parent = root.join(parent_name);
    let parent_metadata = match std::fs::symlink_metadata(&parent) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(persistence_error()),
    };
    if !parent_metadata.is_dir() || is_redirected(&parent_metadata) {
        return Err(persistence_error());
    }
    let job = parent.join(job_id);
    let job_metadata = match std::fs::symlink_metadata(&job) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(persistence_error()),
    };
    if is_redirected(&job_metadata) {
        return remove_redirected_leaf(&job, &job_metadata);
    }
    if !job_metadata.is_dir() {
        return Err(persistence_error());
    }

    match parent_name {
        "inputs" => {
            for extension in SUPPORTED_AUDIO_EXTENSIONS {
                remove_known_staging_file(&job, &format!("input.{extension}"))?;
            }
        }
        "outputs" => {
            remove_known_staging_file(&job, "segments.jsonl")?;
            remove_known_staging_file(&job, "manifest.json")?;
            remove_known_staging_file(&job, ".segments.jsonl.tmp")?;
            remove_known_staging_file(&job, ".manifest.json.tmp")?;
        }
        "work" => {}
        _ => unreachable!(),
    }
    std::fs::remove_dir(&job).map_err(|_| persistence_error())
}

fn remove_known_staging_file(directory: &Path, name: &str) -> AudioTranscriptionResult<()> {
    let directory_metadata =
        std::fs::symlink_metadata(directory).map_err(|_| persistence_error())?;
    if !directory_metadata.is_dir() || is_redirected(&directory_metadata) {
        return Err(persistence_error());
    }
    let path = directory.join(name);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(persistence_error()),
    };
    if is_redirected(&metadata) {
        return remove_redirected_leaf(&path, &metadata);
    }
    if !metadata.is_file() {
        return Err(persistence_error());
    }
    std::fs::remove_file(path).map_err(|_| persistence_error())
}

fn remove_redirected_leaf(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> AudioTranscriptionResult<()> {
    if metadata.is_dir() {
        std::fs::remove_dir(path).map_err(|_| persistence_error())
    } else {
        std::fs::remove_file(path).map_err(|_| persistence_error())
    }
}

fn is_redirected(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn validate_uuid(value: &str) -> AudioTranscriptionResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
    })?;
    if parsed.to_string() != value {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    Ok(())
}

fn picker_error() -> AudioTranscriptionError {
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioPickerUnavailable)
}

fn persistence_error() -> AudioTranscriptionError {
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioPersistenceFailed)
}

fn internal_error() -> AudioTranscriptionError {
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::InternalError)
}

fn cancelled_error() -> AudioTranscriptionError {
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioCancelled)
}

#[cfg(debug_assertions)]
pub(crate) fn run_local_audio_smoke(
    smoke_root: &Path,
    input_path: &Path,
) -> Result<(), &'static str> {
    const PREFIX: &str = "meetinglens-audio-live-smoke-";
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| "temp_root_unavailable")?;
    let root = smoke_root
        .canonicalize()
        .map_err(|_| "smoke_root_unavailable")?;
    let smoke_id = root
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_prefix(PREFIX))
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| {
            root.file_name()
                .and_then(OsStr::to_str)
                .and_then(|name| name.strip_prefix(PREFIX))
                .is_some_and(|candidate| value.to_string() == candidate)
        });
    if root.parent() != Some(temp.as_path()) || smoke_id.is_none() {
        return Err("smoke_root_not_isolated");
    }
    let canonical_input = input_path
        .canonicalize()
        .map_err(|_| "smoke_input_unavailable")?;
    if canonical_input.parent() != Some(root.as_path()) {
        return Err("smoke_input_not_isolated");
    }
    let app_data = root.join("app-data");
    if app_data.exists() {
        return Err("smoke_app_data_already_exists");
    }

    let meetings =
        MeetingService::initialize(&app_data).map_err(|_| "service_initialize_failed")?;
    let runtime = AudioTranscriptionRuntime::initialize(meetings.clone());
    let policy = runtime.policy();
    if !policy.worker_available || !policy.model_available || policy.busy {
        return Err("worker_or_model_unavailable");
    }
    let mut phases = Vec::new();
    let result = runtime
        .transcribe_new_meeting(
            CreateMeeting {
                title: "Synthetic local audio smoke".to_string(),
                primary_language: "en-AU".to_string(),
            },
            input_path.to_path_buf(),
            |snapshot| phases.push(snapshot.phase),
        )
        .map_err(|error| error.code.as_str())?;
    let meeting_id = result.outcome.meeting.id.clone();
    let source_id = result.outcome.source.source.id.clone();
    if result.outcome.duplicate
        || result.outcome.source.source.source_type
            != crate::data::TranscriptSourceType::ImportedAudio
        || result.outcome.source.segment_count == 0
        || !phases.contains(&AudioTranscriptionPhase::Transcribing)
        || phases.last() != Some(&AudioTranscriptionPhase::Completed)
    {
        return Err("smoke_result_invalid");
    }
    for parent in ["inputs", "outputs", "work"] {
        let directory = runtime.inner.staging_root.join(parent);
        if directory.exists()
            && std::fs::read_dir(directory)
                .map_err(|_| "staging_inspection_failed")?
                .next()
                .is_some()
        {
            return Err("staging_not_clean");
        }
    }
    drop(runtime);
    drop(meetings);

    let reopened = MeetingService::initialize(&app_data).map_err(|_| "restart_failed")?;
    let source = reopened
        .transcripts()
        .get_source(&meeting_id, &source_id)
        .map_err(|_| "source_recovery_failed")?;
    let segments = reopened
        .transcripts()
        .list_segments(&meeting_id, &source_id)
        .map_err(|_| "segment_recovery_failed")?;
    if source.source_type != crate::data::TranscriptSourceType::ImportedAudio
        || source.parser_version != crate::data::AUDIO_TRANSCRIPT_PARSER_VERSION
        || segments.is_empty()
        || segments.iter().any(|segment| {
            segment.original_text.trim().is_empty()
                || segment.speaker_label.is_some()
                || segment.speaker_name.is_some()
                || segment.confidence.is_some()
                || segment.translated_text.is_some()
                || !segment.is_final
        })
    {
        return Err("recovered_transcript_invalid");
    }
    let connection = rusqlite::Connection::open(reopened.storage_paths().database())
        .map_err(|_| "database_open_failed")?;
    let schema_version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|_| "schema_check_failed")?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| "integrity_check_failed")?;
    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(|_| "foreign_key_check_failed")?;
    drop(connection);
    if schema_version != 7 || integrity != "ok" || foreign_key_violations != 0 {
        return Err("database_validation_failed");
    }
    reopened
        .delete(&meeting_id)
        .map_err(|_| "meeting_cleanup_failed")?;
    if !reopened
        .list()
        .map_err(|_| "meeting_list_failed")?
        .is_empty()
    {
        return Err("meeting_cleanup_incomplete");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("meetinglens-audio-runtime-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("test root should create");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn detects_only_matching_supported_container_signatures() {
        assert_eq!(
            detect_media_type("wav", b"RIFF\x00\x00\x00\x00WAVE"),
            Ok("audio/wav".to_string())
        );
        assert_eq!(
            detect_media_type("mp3", b"ID3\x04\x00\x00"),
            Ok("audio/mpeg".to_string())
        );
        assert_eq!(
            detect_media_type("m4a", b"\x00\x00\x00\x18ftypM4A "),
            Ok("audio/mp4".to_string())
        );
        assert_eq!(
            detect_media_type("mp4", b"\x00\x00\x00\x18ftypisom"),
            Ok("audio/mp4".to_string())
        );
        assert_eq!(
            detect_media_type("wav", b"ID3\x04\x00\x00")
                .expect_err("mismatch should fail")
                .code,
            AudioTranscriptionErrorCode::AudioTypeMismatch
        );
    }

    #[test]
    fn local_path_guard_accepts_drive_paths_but_rejects_unc_and_relative_paths() {
        #[cfg(windows)]
        {
            assert!(is_local_absolute_path(Path::new(
                "C:\\synthetic\\audio.wav"
            )));
            assert!(is_local_absolute_path(Path::new(
                "\\\\?\\C:\\synthetic\\audio.wav"
            )));
            assert!(!is_local_absolute_path(Path::new(
                "\\\\server\\share\\audio.wav"
            )));
            assert!(!is_local_absolute_path(Path::new(
                "\\\\?\\UNC\\server\\share\\audio.wav"
            )));
        }
        assert!(!is_local_absolute_path(Path::new("relative/audio.wav")));
    }

    #[test]
    fn streaming_copy_hashes_and_honours_pre_cancel() {
        let directory = TestDirectory::new();
        let source = directory.0.join("source.wav");
        let destination = directory.0.join("destination.wav");
        std::fs::write(&source, b"RIFF\x00\x00\x00\x00WAVEsynthetic").expect("source should write");
        let metadata = std::fs::metadata(&source).expect("metadata should read");
        let source_file = open_source_file(&source).expect("source should open");
        let identity = source_file_identity(&source_file).expect("identity should read");
        drop(source_file);
        let token = AudioCancellationToken::default();
        let (size, hash) = copy_and_hash(
            &source,
            &destination,
            metadata.clone(),
            identity,
            &token,
            |_| {},
        )
        .expect("copy should succeed");
        assert_eq!(size, metadata.len());
        assert_eq!(hash.len(), 64);

        let replacement = directory.0.join("replacement.wav");
        std::fs::write(&replacement, b"RIFF\x00\x00\x00\x00WAVEsynthetic")
            .expect("replacement should write");
        let replacement_file = open_source_file(&replacement).expect("replacement should open");
        let replacement_identity =
            source_file_identity(&replacement_file).expect("replacement identity should read");
        drop(replacement_file);
        assert_ne!(identity, replacement_identity);
        assert_eq!(
            copy_and_hash(
                &source,
                &directory.0.join("wrong-identity.wav"),
                metadata.clone(),
                replacement_identity,
                &token,
                |_| {},
            )
            .expect_err("a different file identity must fail closed")
            .code,
            AudioTranscriptionErrorCode::AudioIntegrityFailed
        );

        let cancelled_destination = directory.0.join("cancelled.wav");
        token.cancel();
        assert_eq!(
            copy_and_hash(
                &source,
                &cancelled_destination,
                metadata,
                identity,
                &token,
                |_| {},
            )
            .expect_err("cancelled copy should fail")
            .code,
            AudioTranscriptionErrorCode::AudioCancelled
        );
    }

    #[test]
    fn staging_cleanup_removes_only_the_known_non_recursive_layout() {
        let directory = TestDirectory::new();
        let root = directory.0.join("staging");
        let job_id = Uuid::new_v4().to_string();
        for parent in ["inputs", "outputs", "work"] {
            std::fs::create_dir_all(root.join(parent).join(&job_id))
                .expect("known directory should create");
        }
        std::fs::write(
            root.join("inputs").join(&job_id).join("input.wav"),
            b"synthetic",
        )
        .expect("input should write");
        std::fs::write(
            root.join("outputs").join(&job_id).join("segments.jsonl"),
            b"synthetic",
        )
        .expect("segments should write");
        std::fs::write(
            root.join("outputs").join(&job_id).join("manifest.json"),
            b"synthetic",
        )
        .expect("manifest should write");
        std::fs::write(
            root.join("outputs")
                .join(&job_id)
                .join(".segments.jsonl.tmp"),
            b"synthetic",
        )
        .expect("segments temp should write");
        std::fs::write(
            root.join("outputs")
                .join(&job_id)
                .join(".manifest.json.tmp"),
            b"synthetic",
        )
        .expect("manifest temp should write");

        for parent in ["inputs", "outputs", "work"] {
            cleanup_staging_job(&root, parent, &job_id).expect("known job should clean");
            assert!(!root.join(parent).join(&job_id).exists());
            assert!(root.join(parent).exists());
        }

        let unexpected_id = Uuid::new_v4().to_string();
        let unexpected = root.join("work").join(&unexpected_id);
        std::fs::create_dir(&unexpected).expect("unexpected job should create");
        std::fs::write(unexpected.join("unexpected.tmp"), b"synthetic")
            .expect("unexpected leaf should write");
        assert_eq!(
            cleanup_staging_job(&root, "work", &unexpected_id)
                .expect_err("unknown leaves must fail closed")
                .code,
            AudioTranscriptionErrorCode::AudioPersistenceFailed
        );
        assert!(unexpected.join("unexpected.tmp").exists());
    }

    #[test]
    fn cancellation_and_atomic_commit_have_a_single_winner() {
        let cancelled_directory = TestDirectory::new();
        let cancelled_runtime = AudioTranscriptionRuntime::initialize(
            MeetingService::initialize(&cancelled_directory.0).expect("service should initialize"),
        );
        let (cancelled_id, cancelled_token) =
            cancelled_runtime.begin_job(None).expect("job should start");
        cancelled_token.cancel();
        assert_eq!(
            cancelled_runtime
                .begin_commit(&cancelled_id, 12)
                .expect_err("cancel should win before commit")
                .code,
            AudioTranscriptionErrorCode::AudioCancelled
        );
        cancelled_runtime
            .finish_job(
                &cancelled_id,
                AudioTranscriptionPhase::Cancelled,
                AudioTranscriptionStatus::Cancelled,
                Some(AudioTranscriptionErrorCode::AudioCancelled),
            )
            .expect("cancelled job should finish");

        let committing_directory = TestDirectory::new();
        let committing_runtime = AudioTranscriptionRuntime::initialize(
            MeetingService::initialize(&committing_directory.0).expect("service should initialize"),
        );
        let (committing_id, committing_token) = committing_runtime
            .begin_job(None)
            .expect("job should start");
        let committing = committing_runtime
            .begin_commit(&committing_id, 12)
            .expect("commit should start");
        assert_eq!(committing.phase, AudioTranscriptionPhase::Committing);
        assert_eq!(
            committing_runtime
                .cancel(&committing_id)
                .expect("commit winner should remain visible")
                .phase,
            AudioTranscriptionPhase::Committing
        );
        assert!(!committing_token.is_cancelled());
        committing_runtime
            .finish_job(
                &committing_id,
                AudioTranscriptionPhase::Completed,
                AudioTranscriptionStatus::Completed,
                None,
            )
            .expect("committing job should finish");
    }

    #[test]
    fn late_progress_cannot_revert_a_cancelled_job_to_running() {
        let directory = TestDirectory::new();
        let runtime = AudioTranscriptionRuntime::initialize(
            MeetingService::initialize(&directory.0).expect("service should initialize"),
        );
        let (job_id, _token) = runtime.begin_job(None).expect("job should start");

        let cancelled = runtime.cancel(&job_id).expect("job should cancel");
        assert_eq!(cancelled.phase, AudioTranscriptionPhase::Cancelled);
        assert_eq!(cancelled.status, AudioTranscriptionStatus::Cancelled);

        let after_late_progress = runtime
            .update_progress(
                &job_id,
                AudioTranscriptionPhase::Transcribing,
                AudioTranscriptionStatus::Running,
                12,
                Some(12),
                Some(1),
                Some(2),
                None,
            )
            .expect("late progress should return the terminal snapshot");
        assert_eq!(after_late_progress, cancelled);
        assert_eq!(
            runtime
                .status(&job_id)
                .expect("job status should remain available"),
            cancelled
        );
    }
}
