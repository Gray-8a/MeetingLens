use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audio_transcription_error::{
    AudioTranscriptionError, AudioTranscriptionErrorCode, AudioTranscriptionResult,
};
use crate::data::AUDIO_TRANSCRIPT_PARSER_VERSION;

pub const AUDIO_PROTOCOL_VERSION: &str = "1";
pub const AUDIO_WORKER_MODULE: &str = "meetinglens_transcription_worker";
pub const AUDIO_MODEL_ID: &str = "Systran/faster-whisper-small";
pub const AUDIO_MODEL_REVISION: &str = "536b0662742c02347bc0e980a01041f333bce120";
pub const AUDIO_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const AUDIO_MAX_DURATION_MS: u64 = 4 * 60 * 60 * 1000;
pub const AUDIO_MAX_SEGMENTS: usize = 20_000;
pub const AUDIO_MAX_SEGMENT_BYTES: usize = 64 * 1024;
pub const AUDIO_MAX_TRANSCRIPT_BYTES: usize = 10 * 1024 * 1024;

const MAX_PROTOCOL_LINE_BYTES: usize = 64 * 1024;
const MAX_PROTOCOL_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const MAX_STDERR_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_ARTIFACT_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_JOB_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_HARD_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
const CHANNEL_CAPACITY: usize = 32;

#[derive(Clone, Default)]
pub struct AudioCancellationToken(Arc<AtomicBool>);

impl AudioCancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone)]
pub struct AudioWorkerConfig {
    python_executable: PathBuf,
    working_directory: PathBuf,
    models_root: PathBuf,
    staging_root: PathBuf,
    hard_timeout: Duration,
    job_memory_limit_bytes: u64,
}

impl AudioWorkerConfig {
    pub fn new(
        python_executable: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
        models_root: impl Into<PathBuf>,
        staging_root: impl Into<PathBuf>,
    ) -> AudioTranscriptionResult<Self> {
        let config = Self {
            python_executable: python_executable.into(),
            working_directory: working_directory.into(),
            models_root: models_root.into(),
            staging_root: staging_root.into(),
            hard_timeout: MAX_HARD_TIMEOUT,
            job_memory_limit_bytes: MAX_JOB_MEMORY_BYTES,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> AudioTranscriptionResult<()> {
        for path in [
            &self.python_executable,
            &self.working_directory,
            &self.models_root,
            &self.staging_root,
        ] {
            if !path.is_absolute() {
                return Err(AudioTranscriptionError::new(
                    AudioTranscriptionErrorCode::AudioWorkerUnavailable,
                ));
            }
        }
        if !self.python_executable.is_file()
            || !self.working_directory.is_dir()
            || !self.models_root.is_dir()
            || !self.staging_root.is_dir()
            || self.hard_timeout.is_zero()
            || self.hard_timeout > MAX_HARD_TIMEOUT
            || self.job_memory_limit_bytes == 0
            || self.job_memory_limit_bytes > MAX_JOB_MEMORY_BYTES
        {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioWorkerUnavailable,
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for AudioWorkerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AudioWorkerConfig")
            .field("python_executable", &"[PRIVATE_PATH]")
            .field("working_directory", &"[PRIVATE_PATH]")
            .field("models_root", &"[PRIVATE_PATH]")
            .field("staging_root", &"[PRIVATE_PATH]")
            .field("hard_timeout", &self.hard_timeout)
            .field("job_memory_limit_bytes", &self.job_memory_limit_bytes)
            .finish()
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioWorkerRequest {
    protocol_version: &'static str,
    pub request_id: String,
    operation: &'static str,
    pub input_path: PathBuf,
    pub allowed_input_root: PathBuf,
    pub output_root: PathBuf,
    pub expected_sha256: String,
    pub expected_size_bytes: u64,
    pub detected_media_type: String,
    pub safe_extension: String,
}

impl AudioWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: String,
        input_path: PathBuf,
        allowed_input_root: PathBuf,
        output_root: PathBuf,
        expected_sha256: String,
        expected_size_bytes: u64,
        detected_media_type: String,
        safe_extension: String,
    ) -> Self {
        Self {
            protocol_version: AUDIO_PROTOCOL_VERSION,
            request_id,
            operation: "transcribe",
            input_path,
            allowed_input_root,
            output_root,
            expected_sha256,
            expected_size_bytes,
            detected_media_type,
            safe_extension,
        }
    }
}

impl fmt::Debug for AudioWorkerRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AudioWorkerRequest")
            .field("protocol_version", &self.protocol_version)
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .field("input_path", &"[PRIVATE_PATH]")
            .field("allowed_input_root", &"[PRIVATE_PATH]")
            .field("output_root", &"[PRIVATE_PATH]")
            .field("expected_sha256", &"[SHA256]")
            .field("expected_size_bytes", &self.expected_size_bytes)
            .field("detected_media_type", &self.detected_media_type)
            .field("safe_extension", &self.safe_extension)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioWorkerStage {
    ValidatingInput,
    VerifyingModel,
    LoadingModel,
    Transcribing,
    WritingArtifacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioWorkerProgress {
    pub sequence: u64,
    pub stage: AudioWorkerStage,
    pub processed_ms: Option<u64>,
    pub total_ms: Option<u64>,
    pub progress_permille: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioArtifact {
    kind: String,
    relative_path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioWorkerTerminalResult {
    parser_version: String,
    language: Option<String>,
    duration_ms: u64,
    segment_count: usize,
    segments_artifact: AudioArtifact,
    manifest_artifact: AudioArtifact,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoteAudioError {
    code: AudioTranscriptionErrorCode,
    message: String,
    retryable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkerEvent {
    protocol_version: String,
    request_id: String,
    #[serde(rename = "type")]
    event_type: String,
    sequence: Option<u64>,
    stage: Option<AudioWorkerStage>,
    processed_ms: Option<u64>,
    total_ms: Option<u64>,
    progress_permille: Option<u16>,
    ok: Option<bool>,
    result: Option<AudioWorkerTerminalResult>,
    error: Option<RemoteAudioError>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioManifest {
    schema_version: u32,
    protocol_version: String,
    request_id: String,
    status: String,
    parser_name: String,
    parser_version: String,
    input_sha256: String,
    model: String,
    model_revision: String,
    faster_whisper_version: String,
    device: String,
    compute_type: String,
    language: Option<String>,
    duration_ms: u64,
    segment_count: usize,
    beam_size: u32,
    task: String,
    segments_artifact: AudioArtifact,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArtifactSegment {
    start_offset_ms: u64,
    end_offset_ms: u64,
    text: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ValidatedAudioSegment {
    pub start_offset_ms: u64,
    pub end_offset_ms: u64,
    pub text: String,
}

impl fmt::Debug for ValidatedAudioSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedAudioSegment")
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("text", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ValidatedAudioTranscript {
    pub parser_version: String,
    pub language: Option<String>,
    pub duration_ms: u64,
    pub segments: Vec<ValidatedAudioSegment>,
}

impl fmt::Debug for ValidatedAudioTranscript {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedAudioTranscript")
            .field("parser_version", &self.parser_version)
            .field("language", &self.language)
            .field("duration_ms", &self.duration_ms)
            .field("segment_count", &self.segments.len())
            .finish()
    }
}

enum ProtocolOutcome {
    Progress(AudioWorkerProgress),
    Success(AudioWorkerTerminalResult),
    RemoteError(AudioTranscriptionError),
}

#[derive(Debug)]
enum ReaderMessage {
    Line(Vec<u8>),
    End,
    Error,
}

trait AudioWorkerProcessRunner: Send + Sync {
    fn run(
        &self,
        config: &AudioWorkerConfig,
        request: &[u8],
        cancellation: &AudioCancellationToken,
        on_line: &mut dyn FnMut(&[u8]) -> AudioTranscriptionResult<()>,
    ) -> AudioTranscriptionResult<()>;
}

#[derive(Default)]
struct SystemAudioWorkerProcessRunner;

pub struct AudioWorkerClient {
    config: AudioWorkerConfig,
    runner: Arc<dyn AudioWorkerProcessRunner>,
}

impl AudioWorkerClient {
    pub fn new(config: AudioWorkerConfig) -> Self {
        Self {
            config,
            runner: Arc::new(SystemAudioWorkerProcessRunner),
        }
    }

    #[cfg(test)]
    fn with_runner(config: AudioWorkerConfig, runner: Arc<dyn AudioWorkerProcessRunner>) -> Self {
        Self { config, runner }
    }

    pub fn transcribe(
        &self,
        request: &AudioWorkerRequest,
        cancellation: &AudioCancellationToken,
        mut on_progress: impl FnMut(AudioWorkerProgress),
    ) -> AudioTranscriptionResult<ValidatedAudioTranscript> {
        validate_client_request(request, &self.config)?;
        if cancellation.is_cancelled() {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioCancelled,
            ));
        }
        let mut serialized = serde_json::to_vec(request).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::InternalError)
        })?;
        if serialized.len() + 1 > MAX_PROTOCOL_LINE_BYTES {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioProtocolError,
            ));
        }
        serialized.push(b'\n');

        let mut expected_sequence = 1_u64;
        let mut terminal: Option<Result<AudioWorkerTerminalResult, AudioTranscriptionError>> = None;
        self.runner
            .run(&self.config, &serialized, cancellation, &mut |line| {
                if terminal.is_some() {
                    return Err(AudioTranscriptionError::new(
                        AudioTranscriptionErrorCode::AudioProtocolError,
                    ));
                }
                match decode_worker_event(&request.request_id, line, expected_sequence)? {
                    ProtocolOutcome::Progress(progress) => {
                        expected_sequence = expected_sequence.checked_add(1).ok_or_else(|| {
                            AudioTranscriptionError::new(
                                AudioTranscriptionErrorCode::AudioProtocolError,
                            )
                        })?;
                        on_progress(progress);
                    }
                    ProtocolOutcome::Success(result) => terminal = Some(Ok(result)),
                    ProtocolOutcome::RemoteError(error) => terminal = Some(Err(error)),
                }
                Ok(())
            })?;
        let terminal = terminal.ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioProtocolError)
        })??;
        validate_worker_result(request, &terminal)
    }
}

fn decode_worker_event(
    request_id: &str,
    line: &[u8],
    expected_sequence: u64,
) -> AudioTranscriptionResult<ProtocolOutcome> {
    if line.is_empty() || line.len() > MAX_PROTOCOL_LINE_BYTES || line.contains(&b'\n') {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioProtocolError,
        ));
    }
    let event: WorkerEvent = serde_json::from_slice(line).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioProtocolError)
    })?;
    if event.protocol_version != AUDIO_PROTOCOL_VERSION || event.request_id != request_id {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioProtocolError,
        ));
    }
    match event.event_type.as_str() {
        "progress" => {
            let sequence = event.sequence.ok_or_else(protocol_error)?;
            let stage = event.stage.ok_or_else(protocol_error)?;
            if sequence != expected_sequence
                || event.ok.is_some()
                || event.result.is_some()
                || event.error.is_some()
                || event.progress_permille.is_some_and(|value| value > 1000)
                || matches!((event.processed_ms, event.total_ms), (Some(done), Some(total)) if done > total)
            {
                return Err(protocol_error());
            }
            Ok(ProtocolOutcome::Progress(AudioWorkerProgress {
                sequence,
                stage,
                processed_ms: event.processed_ms,
                total_ms: event.total_ms,
                progress_permille: event.progress_permille,
            }))
        }
        "result" => {
            if event.sequence.is_some()
                || event.stage.is_some()
                || event.processed_ms.is_some()
                || event.total_ms.is_some()
                || event.progress_permille.is_some()
            {
                return Err(protocol_error());
            }
            match (event.ok, event.result, event.error) {
                (Some(true), Some(result), None) => Ok(ProtocolOutcome::Success(result)),
                (Some(false), None, Some(error)) => {
                    let expected_message = error.code.safe_message();
                    if error.message != expected_message
                        || error.retryable != error.code.retryable()
                    {
                        return Err(protocol_error());
                    }
                    Ok(ProtocolOutcome::RemoteError(AudioTranscriptionError::new(
                        error.code,
                    )))
                }
                _ => Err(protocol_error()),
            }
        }
        _ => Err(protocol_error()),
    }
}

fn protocol_error() -> AudioTranscriptionError {
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioProtocolError)
}

impl AudioWorkerProcessRunner for SystemAudioWorkerProcessRunner {
    fn run(
        &self,
        config: &AudioWorkerConfig,
        request: &[u8],
        cancellation: &AudioCancellationToken,
        on_line: &mut dyn FnMut(&[u8]) -> AudioTranscriptionResult<()>,
    ) -> AudioTranscriptionResult<()> {
        config.validate()?;
        let mut child = build_worker_command(config).spawn().map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let job = WindowsJob::new(config.job_memory_limit_bytes).inspect_err(|_| {
            terminate_unassigned_child(&mut child);
        })?;
        job.assign(&child).inspect_err(|_| {
            terminate_unassigned_child(&mut child);
        })?;
        let mut guard = AudioProcessGuard::new(child, job);

        let mut stdin = guard.child_mut().stdin.take().ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let stdout = guard.child_mut().stdout.take().ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let stderr = guard.child_mut().stderr.take().ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let request = request.to_vec();
        let writer = thread::spawn(move || -> std::io::Result<()> {
            stdin.write_all(&request)?;
            stdin.flush()
        });
        let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
        let stdout_reader = thread::spawn(move || read_json_lines(stdout, sender));
        let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_STDERR_CAPTURE_BYTES));

        let started = Instant::now();
        let mut stdout_done = false;
        let mut status: Option<ExitStatus> = None;
        while !stdout_done || status.is_none() {
            if cancellation.is_cancelled() {
                guard.terminate();
                finish_threads(writer, stdout_reader, stderr_reader);
                return Err(AudioTranscriptionError::new(
                    AudioTranscriptionErrorCode::AudioCancelled,
                ));
            }
            if started.elapsed() >= config.hard_timeout {
                guard.terminate();
                finish_threads(writer, stdout_reader, stderr_reader);
                return Err(AudioTranscriptionError::new(
                    AudioTranscriptionErrorCode::AudioTimeout,
                ));
            }
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(ReaderMessage::Line(line)) => {
                    if let Err(error) = on_line(&line) {
                        guard.terminate();
                        finish_threads(writer, stdout_reader, stderr_reader);
                        return Err(error);
                    }
                }
                Ok(ReaderMessage::End) => stdout_done = true,
                Ok(ReaderMessage::Error) => {
                    guard.terminate();
                    finish_threads(writer, stdout_reader, stderr_reader);
                    return Err(protocol_error());
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) if stdout_done => {}
                Err(RecvTimeoutError::Disconnected) => {
                    guard.terminate();
                    finish_threads(writer, stdout_reader, stderr_reader);
                    return Err(protocol_error());
                }
            }
            if status.is_none() {
                status = match guard.child_mut().try_wait() {
                    Ok(value) => value,
                    Err(_) => {
                        guard.terminate();
                        finish_threads(writer, stdout_reader, stderr_reader);
                        return Err(AudioTranscriptionError::new(
                            AudioTranscriptionErrorCode::AudioWorkerCrashed,
                        ));
                    }
                };
            }
        }

        join_writer(writer)?;
        stdout_reader.join().map_err(|_| protocol_error())?;
        let stderr = stderr_reader.join().map_err(|_| protocol_error())??;
        guard.disarm();
        let status = status.ok_or_else(protocol_error)?;
        if stderr.overflowed || !status.success() {
            return Err(map_exit_status(status));
        }
        Ok(())
    }
}

fn build_worker_command(config: &AudioWorkerConfig) -> Command {
    let mut command = Command::new(&config.python_executable);
    command.args(["-I", "-m", AUDIO_WORKER_MODULE]);

    command
        .env_clear()
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONNOUSERSITE", "1")
        .env("OMP_NUM_THREADS", "4")
        .env("MKL_NUM_THREADS", "4")
        .env("OPENBLAS_NUM_THREADS", "4")
        .env("NUMEXPR_NUM_THREADS", "4")
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_DATASETS_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .env("HOME", &config.working_directory)
        .env("USERPROFILE", &config.working_directory)
        .env("LOCALAPPDATA", &config.working_directory)
        .env("TEMP", &config.working_directory)
        .env("TMP", &config.working_directory)
        .env("XDG_CACHE_HOME", &config.working_directory)
        .env("MEETINGLENS_TRANSCRIPTION_MODELS_ROOT", &config.models_root)
        .env(
            "MEETINGLENS_TRANSCRIPTION_STAGING_ROOT",
            &config.staging_root,
        )
        .current_dir(&config.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for variable in ["SystemRoot", "WINDIR", "ComSpec"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn read_json_lines(reader: impl Read, sender: SyncSender<ReaderMessage>) {
    let mut reader = BufReader::new(reader);
    let mut total = 0_usize;
    loop {
        let mut line = Vec::new();
        let read = match reader.read_until(b'\n', &mut line) {
            Ok(read) => read,
            Err(_) => {
                let _ = sender.try_send(ReaderMessage::Error);
                return;
            }
        };
        if read == 0 {
            let _ = sender.try_send(ReaderMessage::End);
            return;
        }
        total = total.saturating_add(read);
        if total > MAX_PROTOCOL_TOTAL_BYTES || line.len() > MAX_PROTOCOL_LINE_BYTES {
            let _ = sender.try_send(ReaderMessage::Error);
            return;
        }
        if line.last() != Some(&b'\n') {
            let _ = sender.try_send(ReaderMessage::Error);
            return;
        }
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.is_empty() {
            let _ = sender.try_send(ReaderMessage::Error);
            return;
        }
        if matches!(
            sender.try_send(ReaderMessage::Line(line)),
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_))
        ) {
            return;
        }
    }
}

#[derive(Debug, Default)]
struct BoundedCapture {
    bytes: Vec<u8>,
    overflowed: bool,
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> AudioTranscriptionResult<BoundedCapture> {
    let mut capture = BoundedCapture::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer).map_err(|_| protocol_error())?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(capture.bytes.len());
        let keep = read.min(remaining);
        capture.bytes.extend_from_slice(&buffer[..keep]);
        if keep < read {
            capture.overflowed = true;
        }
    }
    Ok(capture)
}

fn finish_threads(
    writer: thread::JoinHandle<std::io::Result<()>>,
    stdout_reader: thread::JoinHandle<()>,
    stderr_reader: thread::JoinHandle<AudioTranscriptionResult<BoundedCapture>>,
) {
    let _ = writer.join();
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
}

fn join_writer(writer: thread::JoinHandle<std::io::Result<()>>) -> AudioTranscriptionResult<()> {
    match writer.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        _ => Err(protocol_error()),
    }
}

fn map_exit_status(status: ExitStatus) -> AudioTranscriptionError {
    #[cfg(windows)]
    if matches!(status.code(), Some(-1_073_741_801) | Some(-1_073_741_523)) {
        return AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerCrashed);
    }
    AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerCrashed)
}

fn terminate_unassigned_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct AudioProcessGuard {
    child: Child,
    job: Option<WindowsJob>,
}

impl AudioProcessGuard {
    fn new(child: Child, job: WindowsJob) -> Self {
        Self {
            child,
            job: Some(job),
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn terminate(&mut self) {
        if let Some(job) = &self.job {
            job.terminate();
        }
        let _ = self.child.wait();
    }

    fn disarm(&mut self) {
        self.job.take();
    }
}

impl Drop for AudioProcessGuard {
    fn drop(&mut self) {
        if self.job.is_some() {
            self.terminate();
        }
    }
}

#[cfg(windows)]
struct WindowsJob {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl WindowsJob {
    fn new(memory_limit_bytes: u64) -> AudioTranscriptionResult<Self> {
        use std::mem::size_of;
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_JOB_MEMORY;
        information.JobMemoryLimit = usize::try_from(memory_limit_bytes).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })?;
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &information as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured.is_err() {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioWorkerStartFailed,
            ));
        }
        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> AudioTranscriptionResult<()> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;

        let process = HANDLE(child.as_raw_handle());
        unsafe { AssignProcessToJobObject(self.handle, process) }.map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioWorkerStartFailed)
        })
    }

    fn terminate(&self) {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            let _ = TerminateJobObject(self.handle, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
struct WindowsJob;

#[cfg(not(windows))]
impl WindowsJob {
    fn new(_memory_limit_bytes: u64) -> AudioTranscriptionResult<Self> {
        Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioWorkerUnavailable,
        ))
    }

    fn assign(&self, _child: &Child) -> AudioTranscriptionResult<()> {
        Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioWorkerUnavailable,
        ))
    }

    fn terminate(&self) {}
}

fn validate_client_request(
    request: &AudioWorkerRequest,
    config: &AudioWorkerConfig,
) -> AudioTranscriptionResult<()> {
    if uuid::Uuid::parse_str(&request.request_id)
        .map(|value| value.to_string() != request.request_id)
        .unwrap_or(true)
        || request.expected_size_bytes == 0
        || request.expected_size_bytes > AUDIO_MAX_FILE_BYTES
        || !valid_sha256(&request.expected_sha256)
        || media_type_for_extension(&request.safe_extension)
            != Some(request.detected_media_type.as_str())
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }

    let input = canonical_regular_file(&request.input_path)?;
    let input_root = canonical_directory(&request.allowed_input_root)?;
    let output = canonical_directory(&request.output_root)?;
    let staging = canonical_directory(&config.staging_root)?;
    let output_name = output.file_name().and_then(|value| value.to_str());
    if !input.starts_with(&input_root)
        || input.metadata().map(|value| value.len()).ok() != Some(request.expected_size_bytes)
        || !output.starts_with(&staging)
        || output == staging
        || output_name != Some(request.request_id.as_str())
        || std::fs::read_dir(&output)
            .map_err(|_| {
                AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
            })?
            .next()
            .is_some()
        || input.extension().and_then(|value| value.to_str()) != Some(&request.safe_extension)
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    if hash_file(&input, AUDIO_MAX_FILE_BYTES)? != request.expected_sha256 {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok(())
}

fn validate_worker_result(
    request: &AudioWorkerRequest,
    result: &AudioWorkerTerminalResult,
) -> AudioTranscriptionResult<ValidatedAudioTranscript> {
    if result.parser_version != AUDIO_TRANSCRIPT_PARSER_VERSION
        || result.duration_ms == 0
        || result.duration_ms > AUDIO_MAX_DURATION_MS
        || result.segment_count == 0
        || result.segment_count > AUDIO_MAX_SEGMENTS
        || result.segments_artifact.kind != "segments"
        || result.segments_artifact.relative_path != "segments.jsonl"
        || result.manifest_artifact.kind != "manifest"
        || result.manifest_artifact.relative_path != "manifest.json"
    {
        return Err(AudioTranscriptionError::new(
            if result.duration_ms > AUDIO_MAX_DURATION_MS {
                AudioTranscriptionErrorCode::AudioTooLong
            } else {
                AudioTranscriptionErrorCode::AudioIntegrityFailed
            },
        ));
    }
    validate_language(result.language.as_deref())?;
    let artifacts = [&result.segments_artifact, &result.manifest_artifact];
    let total = artifacts.iter().try_fold(0_u64, |total, artifact| {
        total.checked_add(artifact.size_bytes).ok_or_else(|| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioOutputTooLarge)
        })
    })?;
    if total > MAX_ARTIFACT_TOTAL_BYTES {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioOutputTooLarge,
        ));
    }

    let segments_bytes = read_validated_artifact(
        &request.output_root,
        &result.segments_artifact,
        MAX_ARTIFACT_TOTAL_BYTES,
    )?;
    let manifest_bytes =
        read_validated_artifact(&request.output_root, &result.manifest_artifact, 1024 * 1024)?;
    let manifest: AudioManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| protocol_error())?;
    if manifest.schema_version != 1
        || manifest.protocol_version != AUDIO_PROTOCOL_VERSION
        || manifest.request_id != request.request_id
        || manifest.status != "success"
        || manifest.parser_name != "meetinglens-transcription-worker"
        || manifest.parser_version != result.parser_version
        || manifest.input_sha256 != request.expected_sha256
        || manifest.model != AUDIO_MODEL_ID
        || manifest.model_revision != AUDIO_MODEL_REVISION
        || manifest.faster_whisper_version != "1.2.1"
        || manifest.device != "cpu"
        || manifest.compute_type != "int8"
        || manifest.language != result.language
        || manifest.duration_ms != result.duration_ms
        || manifest.segment_count != result.segment_count
        || manifest.beam_size != 5
        || manifest.task != "transcribe"
        || manifest.segments_artifact != result.segments_artifact
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }

    let segments =
        read_and_validate_segments(&segments_bytes, result.segment_count, result.duration_ms)?;
    Ok(ValidatedAudioTranscript {
        parser_version: result.parser_version.clone(),
        language: result.language.clone(),
        duration_ms: result.duration_ms,
        segments,
    })
}

fn read_and_validate_segments(
    bytes: &[u8],
    expected_count: usize,
    duration_ms: u64,
) -> AudioTranscriptionResult<Vec<ValidatedAudioSegment>> {
    let mut reader = BufReader::new(bytes);
    let mut segments = Vec::with_capacity(expected_count);
    let mut total_text_bytes = 0_usize;
    let mut previous_end = 0_u64;
    loop {
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
        })?;
        if read == 0 {
            break;
        }
        if line.len() > AUDIO_MAX_SEGMENT_BYTES.saturating_add(4096) || line.last() != Some(&b'\n')
        {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioOutputTooLarge,
            ));
        }
        line.pop();
        if line.last() == Some(&b'\r') {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioIntegrityFailed,
            ));
        }
        let parsed: ArtifactSegment = serde_json::from_slice(&line).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioProtocolError)
        })?;
        if parsed.text.trim().is_empty()
            || parsed.text.len() > AUDIO_MAX_SEGMENT_BYTES
            || parsed.text.chars().any(forbidden_text_control)
            || parsed.start_offset_ms < previous_end
            || parsed.end_offset_ms <= parsed.start_offset_ms
            || parsed.end_offset_ms > duration_ms.saturating_add(1000)
        {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioIntegrityFailed,
            ));
        }
        total_text_bytes = total_text_bytes
            .checked_add(parsed.text.len())
            .ok_or_else(|| {
                AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioOutputTooLarge)
            })?;
        if total_text_bytes > AUDIO_MAX_TRANSCRIPT_BYTES || segments.len() >= AUDIO_MAX_SEGMENTS {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioOutputTooLarge,
            ));
        }
        previous_end = parsed.end_offset_ms;
        segments.push(ValidatedAudioSegment {
            start_offset_ms: parsed.start_offset_ms,
            end_offset_ms: parsed.end_offset_ms,
            text: parsed.text,
        });
    }
    if segments.len() != expected_count || segments.is_empty() {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok(segments)
}

fn forbidden_text_control(character: char) -> bool {
    (character < ' ' && !matches!(character, '\t' | '\n')) || character == '\u{7f}'
}

fn read_validated_artifact(
    root: &Path,
    artifact: &AudioArtifact,
    max_bytes: u64,
) -> AudioTranscriptionResult<Vec<u8>> {
    let relative = Path::new(&artifact.relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)
            ) || matches!(component, Component::Normal(value) if value.to_string_lossy().contains(':'))
        })
        || artifact.size_bytes == 0
        || artifact.size_bytes > max_bytes
        || !valid_sha256(&artifact.sha256)
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let canonical_root = canonical_directory(root)?;
    let lexical_path = root.join(relative);
    let lexical_metadata = std::fs::symlink_metadata(&lexical_path).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    if !lexical_metadata.is_file() || is_redirected(&lexical_metadata) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let canonical_path = lexical_path.canonicalize().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let mut options = std::fs::OpenOptions::new();
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
    let mut file = options.open(&lexical_path).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    let before = file.metadata().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    if !before.is_file()
        || is_redirected(&before)
        || before.len() != artifact.size_bytes
        || before.len() > max_bytes
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
        })?;
    let after = file.metadata().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    if bytes.len() as u64 != artifact.size_bytes
        || bytes.len() as u64 > max_bytes
        || before.len() != after.len()
        || hash_bytes(&bytes) != artifact.sha256
    {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok(bytes)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn media_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "wav" => Some("audio/wav"),
        "mp3" => Some("audio/mpeg"),
        "m4a" | "mp4" => Some("audio/mp4"),
        _ => None,
    }
}

fn canonical_directory(path: &Path) -> AudioTranscriptionResult<PathBuf> {
    let canonical = path.canonicalize().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
    })?;
    let metadata = std::fs::symlink_metadata(&canonical).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
    })?;
    if !metadata.is_dir() || is_redirected(&metadata) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    Ok(canonical)
}

fn canonical_regular_file(path: &Path) -> AudioTranscriptionResult<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
    })?;
    if !metadata.file_type().is_file() || is_redirected(&metadata) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    let canonical = path.canonicalize().map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioInvalidInput)
    })?;
    if !canonical.is_file() {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioInvalidInput,
        ));
    }
    Ok(canonical)
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

fn hash_file(path: &Path, max_bytes: u64) -> AudioTranscriptionResult<String> {
    let mut file = File::open(path).map_err(|_| {
        AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|_| {
            AudioTranscriptionError::new(AudioTranscriptionErrorCode::AudioIntegrityFailed)
        })?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > max_bytes {
            return Err(AudioTranscriptionError::new(
                AudioTranscriptionErrorCode::AudioOutputTooLarge,
            ));
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn validate_language(language: Option<&str>) -> AudioTranscriptionResult<()> {
    if language.is_some_and(|value| {
        value.is_empty()
            || value.len() > 35
            || value.chars().any(|character| {
                !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            })
    }) {
        return Err(AudioTranscriptionError::new(
            AudioTranscriptionErrorCode::AudioIntegrityFailed,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("meetinglens-audio-worker-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&root).expect("create temp root");
            Self(root)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct SuccessfulFakeRunner;

    impl AudioWorkerProcessRunner for SuccessfulFakeRunner {
        fn run(
            &self,
            _config: &AudioWorkerConfig,
            request: &[u8],
            _cancellation: &AudioCancellationToken,
            on_line: &mut dyn FnMut(&[u8]) -> AudioTranscriptionResult<()>,
        ) -> AudioTranscriptionResult<()> {
            let request: serde_json::Value =
                serde_json::from_slice(request).expect("parse request");
            let request_id = request["requestId"].as_str().expect("request id");
            let input_sha256 = request["expectedSha256"].as_str().expect("input hash");
            let output_root = PathBuf::from(request["outputRoot"].as_str().expect("output root"));

            let progress = serde_json::json!({
                "protocolVersion": AUDIO_PROTOCOL_VERSION,
                "requestId": request_id,
                "type": "progress",
                "sequence": 1,
                "stage": "transcribing",
                "processedMs": 500,
                "totalMs": 1000,
                "progressPermille": 500
            });
            on_line(
                serde_json::to_string(&progress)
                    .expect("progress json")
                    .as_bytes(),
            )?;

            let segments_bytes =
                b"{\"startOffsetMs\":0,\"endOffsetMs\":1000,\"text\":\"Synthetic words only\"}\n";
            let segments_path = output_root.join("segments.jsonl");
            std::fs::write(&segments_path, segments_bytes).expect("write segments");
            let segments_artifact = serde_json::json!({
                "kind": "segments",
                "relativePath": "segments.jsonl",
                "sizeBytes": segments_bytes.len(),
                "sha256": sha256_bytes(segments_bytes)
            });
            let manifest = serde_json::json!({
                "schemaVersion": 1,
                "protocolVersion": AUDIO_PROTOCOL_VERSION,
                "requestId": request_id,
                "status": "success",
                "parserName": "meetinglens-transcription-worker",
                "parserVersion": AUDIO_TRANSCRIPT_PARSER_VERSION,
                "inputSha256": input_sha256,
                "model": AUDIO_MODEL_ID,
                "modelRevision": AUDIO_MODEL_REVISION,
                "fasterWhisperVersion": "1.2.1",
                "device": "cpu",
                "computeType": "int8",
                "language": "en",
                "durationMs": 1000,
                "segmentCount": 1,
                "beamSize": 5,
                "task": "transcribe",
                "segmentsArtifact": segments_artifact
            });
            let manifest_bytes = serde_json::to_vec(&manifest).expect("manifest json");
            std::fs::write(output_root.join("manifest.json"), &manifest_bytes)
                .expect("write manifest");
            let terminal = serde_json::json!({
                "protocolVersion": AUDIO_PROTOCOL_VERSION,
                "requestId": request_id,
                "type": "result",
                "ok": true,
                "result": {
                    "parserVersion": AUDIO_TRANSCRIPT_PARSER_VERSION,
                    "language": "en",
                    "durationMs": 1000,
                    "segmentCount": 1,
                    "segmentsArtifact": segments_artifact,
                    "manifestArtifact": {
                        "kind": "manifest",
                        "relativePath": "manifest.json",
                        "sizeBytes": manifest_bytes.len(),
                        "sha256": sha256_bytes(&manifest_bytes)
                    }
                }
            });
            on_line(
                serde_json::to_string(&terminal)
                    .expect("terminal json")
                    .as_bytes(),
            )
        }
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let mut digest = Sha256::new();
        digest.update(bytes);
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[test]
    fn fake_runner_exercises_the_strict_success_contract() {
        let temp = TempRoot::new();
        let input_root = temp.0.join("input");
        let output_staging = temp.0.join("output");
        let working = temp.0.join("work");
        let models = temp.0.join("models");
        for directory in [&input_root, &output_staging, &working, &models] {
            std::fs::create_dir_all(directory).expect("create directory");
        }
        let python = temp.0.join("python.exe");
        std::fs::write(&python, b"synthetic executable placeholder").expect("write python");
        let input = input_root.join("sample.wav");
        std::fs::write(&input, b"synthetic audio bytes").expect("write input");
        let request_id = Uuid::new_v4().to_string();
        let output = output_staging.join(&request_id);
        std::fs::create_dir(&output).expect("create output");
        let input_hash = hash_file(&input, AUDIO_MAX_FILE_BYTES).expect("hash input");
        let request = AudioWorkerRequest::new(
            request_id,
            input,
            input_root,
            output,
            input_hash,
            b"synthetic audio bytes".len() as u64,
            "audio/wav".to_string(),
            "wav".to_string(),
        );
        let config =
            AudioWorkerConfig::new(python, working, models, output_staging).expect("worker config");
        let client = AudioWorkerClient::with_runner(config, Arc::new(SuccessfulFakeRunner));
        let mut progress = Vec::new();
        let result = client
            .transcribe(&request, &AudioCancellationToken::default(), |event| {
                progress.push(event)
            })
            .expect("validated transcript");

        assert_eq!(progress.len(), 1);
        assert_eq!(result.parser_version, AUDIO_TRANSCRIPT_PARSER_VERSION);
        assert_eq!(result.duration_ms, 1000);
        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].text, "Synthetic words only");
    }

    #[test]
    fn segment_artifact_rejects_zero_duration_segments() {
        let bytes =
            b"{\"startOffsetMs\":500,\"endOffsetMs\":500,\"text\":\"Synthetic words only\"}\n";
        assert_eq!(
            read_and_validate_segments(bytes, 1, 1_000)
                .expect_err("zero-duration segments must fail closed")
                .code,
            AudioTranscriptionErrorCode::AudioIntegrityFailed
        );
    }

    #[test]
    fn protocol_rejects_non_monotonic_or_mismatched_events() {
        let request_id = Uuid::new_v4().to_string();
        let valid = serde_json::json!({
            "protocolVersion": AUDIO_PROTOCOL_VERSION,
            "requestId": request_id,
            "type": "progress",
            "sequence": 1,
            "stage": "transcribing",
            "processedMs": 500,
            "totalMs": 1000,
            "progressPermille": 500
        });
        assert!(matches!(
            decode_worker_event(
                valid["requestId"].as_str().expect("request id"),
                serde_json::to_string(&valid).expect("serialize").as_bytes(),
                1,
            ),
            Ok(ProtocolOutcome::Progress(_))
        ));
        assert!(
            decode_worker_event(
                valid["requestId"].as_str().expect("request id"),
                serde_json::to_string(&valid).expect("serialize").as_bytes(),
                2,
            )
            .is_err()
        );
    }

    #[test]
    fn debug_values_redact_private_paths_hashes_and_text() {
        let request = AudioWorkerRequest::new(
            Uuid::new_v4().to_string(),
            PathBuf::from("C:/synthetic/private/input.wav"),
            PathBuf::from("C:/synthetic/private"),
            PathBuf::from("C:/synthetic/private/output"),
            "a".repeat(64),
            32,
            "audio/wav".to_string(),
            "wav".to_string(),
        );
        let debug = format!("{request:?}");
        assert!(!debug.contains("synthetic/private"));
        assert!(!debug.contains(&"a".repeat(64)));
        let segment = ValidatedAudioSegment {
            start_offset_ms: 0,
            end_offset_ms: 1000,
            text: "Synthetic private transcript".to_string(),
        };
        assert!(!format!("{segment:?}").contains("Synthetic private transcript"));
    }

    #[test]
    fn media_types_are_fixed_to_the_four_supported_extensions() {
        assert_eq!(media_type_for_extension("wav"), Some("audio/wav"));
        assert_eq!(media_type_for_extension("mp3"), Some("audio/mpeg"));
        assert_eq!(media_type_for_extension("m4a"), Some("audio/mp4"));
        assert_eq!(media_type_for_extension("mp4"), Some("audio/mp4"));
        assert!(media_type_for_extension("mov").is_none());
    }
}
