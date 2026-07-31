use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const PROTOCOL_VERSION: &str = "1";
const WORKER_MODULE: &str = "meetinglens_docling_worker";
const MAX_PROTOCOL_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_STDERR_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_JOB_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const HARD_TIMEOUT: Duration = Duration::from_secs(150);
const MAX_ARTIFACT_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone)]
pub struct DoclingWorkerConfig {
    python_executable: PathBuf,
    working_directory: PathBuf,
    models_root: PathBuf,
    parser_staging_root: PathBuf,
    hard_timeout: Duration,
    job_memory_limit_bytes: u64,
    #[cfg(test)]
    test_arguments: Option<Vec<OsString>>,
}

impl DoclingWorkerConfig {
    pub fn new(
        python_executable: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
        models_root: impl Into<PathBuf>,
        parser_staging_root: impl Into<PathBuf>,
    ) -> Result<Self, DoclingWorkerError> {
        let config = Self {
            python_executable: python_executable.into(),
            working_directory: working_directory.into(),
            models_root: models_root.into(),
            parser_staging_root: parser_staging_root.into(),
            hard_timeout: HARD_TIMEOUT,
            job_memory_limit_bytes: MAX_JOB_MEMORY_BYTES,
            #[cfg(test)]
            test_arguments: None,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), DoclingWorkerError> {
        for path in [
            &self.python_executable,
            &self.working_directory,
            &self.models_root,
            &self.parser_staging_root,
        ] {
            if !path.is_absolute() {
                return Err(DoclingWorkerError::new(
                    DoclingWorkerErrorCode::ParserEnvironmentMissing,
                ));
            }
        }
        if !self.python_executable.is_file()
            || !self.working_directory.is_dir()
            || !self.models_root.is_dir()
            || !self.parser_staging_root.is_dir()
            || self.hard_timeout > HARD_TIMEOUT
            || self.hard_timeout.is_zero()
            || self.job_memory_limit_bytes > MAX_JOB_MEMORY_BYTES
            || self.job_memory_limit_bytes == 0
        {
            return Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserEnvironmentMissing,
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn with_test_limits(mut self, timeout: Duration, memory_bytes: u64) -> Self {
        self.hard_timeout = timeout;
        self.job_memory_limit_bytes = memory_bytes;
        self
    }

    #[cfg(test)]
    fn with_test_arguments(mut self, arguments: Vec<OsString>) -> Self {
        self.test_arguments = Some(arguments);
        self
    }
}

impl fmt::Debug for DoclingWorkerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DoclingWorkerConfig")
            .field("python_executable", &"[PRIVATE_PATH]")
            .field("working_directory", &"[PRIVATE_PATH]")
            .field("models_root", &"[PRIVATE_PATH]")
            .field("parser_staging_root", &"[PRIVATE_PATH]")
            .field("hard_timeout", &self.hard_timeout)
            .field("job_memory_limit_bytes", &self.job_memory_limit_bytes)
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoclingWorkerErrorCode {
    ParserUnavailable,
    ParserEnvironmentMissing,
    ParserDependencyIncompatible,
    ParserModelsMissing,
    ParserModelsIncompatible,
    ParserInvalidInput,
    ParserUnsupportedType,
    ParserOcrUnavailable,
    ParserProtocolError,
    ParserWorkerStartFailed,
    ParserWorkerCrashed,
    ParserTimeout,
    ParserCancelled,
    ParserResourceLimit,
    ParserPageLimitExceeded,
    ParserOutputTooLarge,
    ParserPartialResult,
    ParserIntegrityFailed,
    ParserFailed,
    InternalError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DoclingWorkerError {
    pub code: DoclingWorkerErrorCode,
    pub message: &'static str,
    pub retryable: bool,
}

impl DoclingWorkerError {
    fn new(code: DoclingWorkerErrorCode) -> Self {
        let (message, retryable) = match code {
            DoclingWorkerErrorCode::ParserUnavailable => {
                ("The local document parser is unavailable.", true)
            }
            DoclingWorkerErrorCode::ParserEnvironmentMissing => {
                ("The document parser environment is missing.", false)
            }
            DoclingWorkerErrorCode::ParserDependencyIncompatible => {
                ("The document parser dependencies are incompatible.", false)
            }
            DoclingWorkerErrorCode::ParserModelsMissing => {
                ("Required local parser models are missing.", false)
            }
            DoclingWorkerErrorCode::ParserModelsIncompatible => {
                ("The local parser models are incompatible.", false)
            }
            DoclingWorkerErrorCode::ParserInvalidInput => {
                ("The document parser input is invalid.", false)
            }
            DoclingWorkerErrorCode::ParserUnsupportedType => {
                ("The document type is not supported.", false)
            }
            DoclingWorkerErrorCode::ParserOcrUnavailable => ("Local OCR is unavailable.", false),
            DoclingWorkerErrorCode::ParserProtocolError => {
                ("The document parser protocol is invalid.", false)
            }
            DoclingWorkerErrorCode::ParserWorkerStartFailed => {
                ("The document parser could not start.", true)
            }
            DoclingWorkerErrorCode::ParserWorkerCrashed => {
                ("The document parser stopped unexpectedly.", true)
            }
            DoclingWorkerErrorCode::ParserTimeout => ("The document parser timed out.", true),
            DoclingWorkerErrorCode::ParserCancelled => ("Document parsing was cancelled.", false),
            DoclingWorkerErrorCode::ParserResourceLimit => {
                ("The document exceeded a parser resource limit.", false)
            }
            DoclingWorkerErrorCode::ParserPageLimitExceeded => {
                ("The document exceeds the page limit.", false)
            }
            DoclingWorkerErrorCode::ParserOutputTooLarge => {
                ("The parser output exceeds the size limit.", false)
            }
            DoclingWorkerErrorCode::ParserPartialResult => {
                ("The parser produced only a partial result.", true)
            }
            DoclingWorkerErrorCode::ParserIntegrityFailed => {
                ("A parser integrity check failed.", false)
            }
            DoclingWorkerErrorCode::ParserFailed => ("The document could not be parsed.", true),
            DoclingWorkerErrorCode::InternalError => ("An internal parser error occurred.", false),
        };
        Self {
            code,
            message,
            retryable,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BaseRequest<'a> {
    protocol_version: &'static str,
    request_id: &'a str,
    operation: &'a str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserOptions {
    pub ocr: bool,
    pub max_pages: u32,
    pub document_timeout_seconds: f64,
}

impl Default for ParserOptions {
    fn default() -> Self {
        Self {
            ocr: false,
            max_pages: 300,
            document_timeout_seconds: 120.0,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoclingWorkerRequest {
    protocol_version: &'static str,
    pub request_id: String,
    operation: &'static str,
    pub document_id: String,
    pub meeting_id: String,
    pub input_path: PathBuf,
    pub allowed_input_root: PathBuf,
    pub expected_sha256: String,
    pub detected_media_type: String,
    pub safe_extension: String,
    pub output_root: PathBuf,
    pub parser_options: ParserOptions,
}

impl DoclingWorkerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request_id: String,
        document_id: String,
        meeting_id: String,
        input_path: PathBuf,
        allowed_input_root: PathBuf,
        expected_sha256: String,
        detected_media_type: String,
        safe_extension: String,
        output_root: PathBuf,
        parser_options: ParserOptions,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            operation: "parse",
            document_id,
            meeting_id,
            input_path,
            allowed_input_root,
            expected_sha256,
            detected_media_type,
            safe_extension,
            output_root,
            parser_options,
        }
    }
}

impl fmt::Debug for DoclingWorkerRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DoclingWorkerRequest")
            .field("protocol_version", &self.protocol_version)
            .field("request_id", &self.request_id)
            .field("operation", &self.operation)
            .field("document_id", &self.document_id)
            .field("meeting_id", &self.meeting_id)
            .field("input_path", &"[PRIVATE_PATH]")
            .field("allowed_input_root", &"[PRIVATE_PATH]")
            .field("expected_sha256", &"[SHA256]")
            .field("detected_media_type", &self.detected_media_type)
            .field("safe_extension", &self.safe_extension)
            .field("output_root", &"[PRIVATE_PATH]")
            .field("parser_options", &self.parser_options)
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingWorkerHealth {
    pub status: String,
    pub service: String,
    pub worker_version: String,
    pub parser_version: String,
    pub protocol_version: String,
    pub python_version: String,
    pub transport: String,
    pub network_listener: bool,
    pub remote_services: bool,
    pub external_plugins: bool,
    pub device: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingFormatCapability {
    pub format: String,
    pub supported: bool,
    pub available: bool,
    pub requires_models: bool,
    pub ocr_available: bool,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingWorkerLimits {
    pub max_input_bytes: u64,
    pub max_pages: u32,
    pub max_cpu_threads: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingWorkerCapabilities {
    pub formats: Vec<DoclingFormatCapability>,
    pub models: String,
    pub ocr_backend: Option<String>,
    pub limits: DoclingWorkerLimits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingArtifact {
    pub kind: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingManifest {
    pub schema_version: u32,
    pub protocol_version: String,
    pub request_id: String,
    pub document_id: String,
    pub parser_name: String,
    pub parser_version: String,
    pub input_sha256: String,
    pub detected_media_type: String,
    pub status: String,
    pub page_count: Option<u32>,
    pub has_ocr: bool,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
    pub artifacts: Vec<DoclingArtifact>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DoclingWorkerResult {
    pub manifest: DoclingManifest,
    pub manifest_artifact: DoclingArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RemoteError {
    code: DoclingWorkerErrorCode,
    message: String,
    retryable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkerEnvelope<T> {
    protocol_version: String,
    request_id: String,
    ok: bool,
    result: Option<T>,
    error: Option<RemoteError>,
}

#[derive(Debug)]
struct ProcessOutput {
    stdout: Vec<u8>,
    #[allow(dead_code)]
    stderr: BoundedCapture,
}

#[derive(Debug, Default)]
struct BoundedCapture {
    bytes: Vec<u8>,
    overflowed: bool,
}

trait WorkerProcessRunner: Send + Sync {
    fn run(
        &self,
        config: &DoclingWorkerConfig,
        request: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<ProcessOutput, DoclingWorkerError>;
}

#[derive(Default)]
struct SystemWorkerProcessRunner;

pub struct DoclingWorkerClient {
    config: DoclingWorkerConfig,
    runner: Arc<dyn WorkerProcessRunner>,
    single_job: Mutex<()>,
}

impl DoclingWorkerClient {
    pub fn new(config: DoclingWorkerConfig) -> Self {
        Self {
            config,
            runner: Arc::new(SystemWorkerProcessRunner),
            single_job: Mutex::new(()),
        }
    }

    #[cfg(test)]
    fn with_runner(config: DoclingWorkerConfig, runner: Arc<dyn WorkerProcessRunner>) -> Self {
        Self {
            config,
            runner,
            single_job: Mutex::new(()),
        }
    }

    pub fn health(
        &self,
        request_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<DoclingWorkerHealth, DoclingWorkerError> {
        let request = BaseRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            operation: "health",
        };
        self.execute(request_id, &request, cancellation)
    }

    pub fn capabilities(
        &self,
        request_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<DoclingWorkerCapabilities, DoclingWorkerError> {
        let request = BaseRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id,
            operation: "capabilities",
        };
        self.execute(request_id, &request, cancellation)
    }

    pub fn parse(
        &self,
        request: &DoclingWorkerRequest,
        cancellation: &CancellationToken,
    ) -> Result<DoclingWorkerResult, DoclingWorkerError> {
        validate_client_request(request, &self.config)?;
        let result = match self.execute(&request.request_id, request, cancellation) {
            Ok(result) => result,
            Err(error) => {
                cleanup_failed_output(&request.output_root);
                return Err(error);
            }
        };
        if let Err(error) = validate_worker_result(request, &result) {
            cleanup_failed_output(&request.output_root);
            return Err(error);
        }
        Ok(result)
    }

    fn execute<T: for<'de> Deserialize<'de>>(
        &self,
        request_id: &str,
        request: &impl Serialize,
        cancellation: &CancellationToken,
    ) -> Result<T, DoclingWorkerError> {
        let _guard = self
            .single_job
            .lock()
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::InternalError))?;
        if cancellation.is_cancelled() {
            return Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserCancelled,
            ));
        }
        let mut serialized = serde_json::to_vec(request)
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::InternalError))?;
        if serialized.len() + 1 > MAX_PROTOCOL_MESSAGE_BYTES {
            return Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserProtocolError,
            ));
        }
        serialized.push(b'\n');
        let output = self.runner.run(&self.config, &serialized, cancellation)?;
        decode_envelope(request_id, &output.stdout)
    }
}

fn decode_envelope<T: for<'de> Deserialize<'de>>(
    request_id: &str,
    stdout: &[u8],
) -> Result<T, DoclingWorkerError> {
    if stdout.is_empty() || stdout.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserProtocolError,
        ));
    }
    let trimmed = stdout
        .strip_suffix(b"\r\n")
        .or_else(|| stdout.strip_suffix(b"\n"))
        .unwrap_or(stdout);
    if trimmed.contains(&b'\n') || trimmed.contains(&b'\r') {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserProtocolError,
        ));
    }
    let envelope: WorkerEnvelope<T> = serde_json::from_slice(trimmed)
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserProtocolError))?;
    if envelope.protocol_version != PROTOCOL_VERSION || envelope.request_id != request_id {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserProtocolError,
        ));
    }
    match (envelope.ok, envelope.result, envelope.error) {
        (true, Some(result), None) => Ok(result),
        (false, None, Some(error)) => {
            let expected = DoclingWorkerError::new(error.code);
            if error.message != expected.message || error.retryable != expected.retryable {
                Err(DoclingWorkerError::new(
                    DoclingWorkerErrorCode::ParserProtocolError,
                ))
            } else {
                Err(expected)
            }
        }
        _ => Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserProtocolError,
        )),
    }
}

impl WorkerProcessRunner for SystemWorkerProcessRunner {
    fn run(
        &self,
        config: &DoclingWorkerConfig,
        request: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<ProcessOutput, DoclingWorkerError> {
        config.validate()?;
        let mut command = build_worker_command(config);
        let mut child = command.spawn().map_err(|_| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
        })?;

        let job = WindowsJob::new(config.job_memory_limit_bytes).inspect_err(|_| {
            terminate_unassigned_child(&mut child);
        })?;
        job.assign(&child).inspect_err(|_| {
            terminate_unassigned_child(&mut child);
        })?;
        let mut guard = DoclingProcessGuard::new(child, job);

        let mut stdin = guard.child_mut().stdin.take().ok_or_else(|| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
        })?;
        let stdout = guard.child_mut().stdout.take().ok_or_else(|| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
        })?;
        let stderr = guard.child_mut().stderr.take().ok_or_else(|| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
        })?;
        let request = request.to_vec();
        let writer = thread::spawn(move || -> std::io::Result<()> {
            stdin.write_all(&request)?;
            stdin.flush()
        });
        let stdout_reader =
            thread::spawn(move || drain_bounded(stdout, MAX_PROTOCOL_MESSAGE_BYTES));
        let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_STDERR_CAPTURE_BYTES));

        let started = Instant::now();
        let status = loop {
            if cancellation.is_cancelled() {
                guard.terminate();
                join_writer(writer)?;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(DoclingWorkerError::new(
                    DoclingWorkerErrorCode::ParserCancelled,
                ));
            }
            if started.elapsed() >= config.hard_timeout {
                guard.terminate();
                join_writer(writer)?;
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(DoclingWorkerError::new(
                    DoclingWorkerErrorCode::ParserTimeout,
                ));
            }
            match guard.child_mut().try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(_) => {
                    guard.terminate();
                    join_writer(writer)?;
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(DoclingWorkerError::new(
                        DoclingWorkerErrorCode::ParserWorkerCrashed,
                    ));
                }
            }
        };
        join_writer(writer)?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::InternalError))??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::InternalError))??;
        guard.disarm();
        if stdout.overflowed {
            return Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserProtocolError,
            ));
        }
        if !status.success() {
            return Err(map_exit_status(status));
        }
        Ok(ProcessOutput {
            stdout: stdout.bytes,
            stderr,
        })
    }
}

fn build_worker_command(config: &DoclingWorkerConfig) -> Command {
    let mut command = Command::new(&config.python_executable);
    #[cfg(test)]
    if let Some(arguments) = &config.test_arguments {
        command.args(arguments);
    } else {
        command.args(["-I", "-m", WORKER_MODULE]);
    }
    #[cfg(not(test))]
    command.args(["-I", "-m", WORKER_MODULE]);

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
        .env("MEETINGLENS_DOCLING_MODELS_ROOT", &config.models_root)
        .env(
            "MEETINGLENS_PARSER_STAGING_ROOT",
            &config.parser_staging_root,
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

fn drain_bounded(
    mut reader: impl Read,
    limit: usize,
) -> Result<BoundedCapture, DoclingWorkerError> {
    let mut capture = BoundedCapture::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserProtocolError))?;
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

fn join_writer(writer: thread::JoinHandle<std::io::Result<()>>) -> Result<(), DoclingWorkerError> {
    match writer.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        _ => Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserProtocolError,
        )),
    }
}

fn map_exit_status(status: ExitStatus) -> DoclingWorkerError {
    #[cfg(windows)]
    if matches!(status.code(), Some(-1_073_741_801) | Some(-1_073_741_523)) {
        return DoclingWorkerError::new(DoclingWorkerErrorCode::ParserResourceLimit);
    }
    DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerCrashed)
}

fn terminate_unassigned_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct DoclingProcessGuard {
    child: Child,
    job: Option<WindowsJob>,
}

impl DoclingProcessGuard {
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

impl Drop for DoclingProcessGuard {
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
    fn new(memory_limit_bytes: u64) -> Result<Self, DoclingWorkerError> {
        use std::mem::size_of;
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        let handle = unsafe { CreateJobObjectW(None, None) }.map_err(|_| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
        })?;
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_JOB_MEMORY;
        information.JobMemoryLimit = usize::try_from(memory_limit_bytes).map_err(|_| {
            DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed)
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
            return Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserWorkerStartFailed,
            ));
        }
        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> Result<(), DoclingWorkerError> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;

        let process = HANDLE(child.as_raw_handle());
        unsafe { AssignProcessToJobObject(self.handle, process) }
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserWorkerStartFailed))
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
    fn new(_memory_limit_bytes: u64) -> Result<Self, DoclingWorkerError> {
        Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserUnavailable,
        ))
    }

    fn assign(&self, _child: &Child) -> Result<(), DoclingWorkerError> {
        Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserUnavailable,
        ))
    }

    fn terminate(&self) {}
}

fn validate_client_request(
    request: &DoclingWorkerRequest,
    config: &DoclingWorkerConfig,
) -> Result<(), DoclingWorkerError> {
    if uuid::Uuid::parse_str(&request.request_id)
        .map(|value| value.to_string() != request.request_id)
        .unwrap_or(true)
        || uuid::Uuid::parse_str(&request.document_id)
            .map(|value| value.to_string() != request.document_id)
            .unwrap_or(true)
        || uuid::Uuid::parse_str(&request.meeting_id)
            .map(|value| value.to_string() != request.meeting_id)
            .unwrap_or(true)
        || request.expected_sha256.len() != 64
        || !request
            .expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || !matches!(
            request.safe_extension.as_str(),
            "pdf" | "docx" | "pptx" | "xlsx" | "txt" | "md" | "csv" | "png" | "jpg" | "jpeg"
        )
        || request.detected_media_type.trim().is_empty()
        || request.parser_options.max_pages == 0
        || request.parser_options.max_pages > 300
        || request.parser_options.document_timeout_seconds <= 0.0
        || request.parser_options.document_timeout_seconds > 120.0
    {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserInvalidInput,
        ));
    }

    let input = canonical_regular_file(&request.input_path)?;
    let allowed_input_root = request
        .allowed_input_root
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserInvalidInput))?;
    let output = request
        .output_root
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserInvalidInput))?;
    let parser_staging_root = config
        .parser_staging_root
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserInvalidInput))?;
    if !allowed_input_root.is_dir()
        || !parser_staging_root.is_dir()
        || !output.is_dir()
        || !input.starts_with(&allowed_input_root)
        || !output.starts_with(&parser_staging_root)
        || output == parser_staging_root
        || std::fs::read_dir(&output)
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserInvalidInput))?
            .next()
            .is_some()
        || input
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value != request.safe_extension)
            .unwrap_or(true)
    {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserInvalidInput,
        ));
    }
    if hash_file(&input)? != request.expected_sha256 {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserIntegrityFailed,
        ));
    }
    Ok(())
}

fn canonical_regular_file(path: &Path) -> Result<PathBuf, DoclingWorkerError> {
    let path = path
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserInvalidInput))?;
    if !path.is_file() {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserInvalidInput,
        ));
    }
    Ok(path)
}

fn validate_worker_result(
    request: &DoclingWorkerRequest,
    result: &DoclingWorkerResult,
) -> Result<(), DoclingWorkerError> {
    if result.manifest.schema_version != 1
        || result.manifest.protocol_version != PROTOCOL_VERSION
        || result.manifest.request_id != request.request_id
        || result.manifest.document_id != request.document_id
        || result.manifest.parser_name != "meetinglens-docling-worker"
        || result.manifest.parser_version != "2.117.0"
        || result.manifest.input_sha256 != request.expected_sha256
        || result.manifest.detected_media_type != request.detected_media_type
        || result.manifest.status != "success"
        || result.manifest.page_count.is_some_and(|pages| pages > 300)
        || result.manifest.warnings.iter().any(|warning| {
            warning.contains(&request.input_path.to_string_lossy().to_string())
                || warning.contains(&request.output_root.to_string_lossy().to_string())
        })
    {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserIntegrityFailed,
        ));
    }

    let mut artifacts = result.manifest.artifacts.clone();
    artifacts.push(result.manifest_artifact.clone());
    artifacts.sort_by(|left, right| left.kind.cmp(&right.kind));
    let kinds: Vec<&str> = artifacts
        .iter()
        .map(|artifact| artifact.kind.as_str())
        .collect();
    if kinds != ["document_json", "manifest", "markdown", "text"] {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserIntegrityFailed,
        ));
    }
    let mut total = 0_u64;
    for artifact in &artifacts {
        validate_artifact(&request.output_root, artifact)?;
        total = total
            .checked_add(artifact.size_bytes)
            .ok_or_else(|| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserOutputTooLarge))?;
    }
    if total > MAX_ARTIFACT_TOTAL_BYTES {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserOutputTooLarge,
        ));
    }
    Ok(())
}

fn validate_artifact(root: &Path, artifact: &DoclingArtifact) -> Result<(), DoclingWorkerError> {
    let relative = Path::new(&artifact.relative_path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)
            ) || matches!(component, Component::Normal(value) if value.to_string_lossy().contains(':'))
        })
        || artifact.sha256.len() != 64
        || !artifact
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserIntegrityFailed,
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserIntegrityFailed))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserIntegrityFailed))?;
    let metadata = path
        .metadata()
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserIntegrityFailed))?;
    if !path.starts_with(canonical_root)
        || !metadata.is_file()
        || metadata.len() != artifact.size_bytes
        || hash_file(&path)? != artifact.sha256
    {
        return Err(DoclingWorkerError::new(
            DoclingWorkerErrorCode::ParserIntegrityFailed,
        ));
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, DoclingWorkerError> {
    let mut file = File::open(path)
        .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserIntegrityFailed))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DoclingWorkerError::new(DoclingWorkerErrorCode::ParserIntegrityFailed))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn cleanup_failed_output(root: &Path) {
    for name in [
        "document.json",
        "content.md",
        "content.txt",
        "manifest.json",
        ".document.json.tmp",
        ".content.md.tmp",
        ".content.txt.tmp",
        ".manifest.json.tmp",
    ] {
        let _ = std::fs::remove_file(root.join(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use uuid::Uuid;

    struct TestWorkspace {
        root: PathBuf,
        input_root: PathBuf,
        staging_root: PathBuf,
        models_root: PathBuf,
        working_directory: PathBuf,
        python: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("meetinglens-docling-{}", Uuid::new_v4()));
            let input_root = root.join("inputs");
            let staging_root = root.join("parser-staging");
            let models_root = root.join("models");
            let working_directory = root.join("worker-temp");
            for directory in [&input_root, &staging_root, &models_root, &working_directory] {
                std::fs::create_dir_all(directory).expect("test directory should create");
            }
            let python = worker_python();
            Self {
                root,
                input_root,
                staging_root,
                models_root,
                working_directory,
                python,
            }
        }

        fn config(&self) -> DoclingWorkerConfig {
            DoclingWorkerConfig::new(
                &self.python,
                &self.working_directory,
                &self.models_root,
                &self.staging_root,
            )
            .expect("test config should validate")
        }

        fn request(&self) -> DoclingWorkerRequest {
            let input = self.input_root.join("synthetic.txt");
            std::fs::write(&input, b"Synthetic Rust client content.")
                .expect("test input should write");
            let output = self.staging_root.join(Uuid::new_v4().to_string());
            std::fs::create_dir(&output).expect("output should create");
            DoclingWorkerRequest::new(
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                input.clone(),
                self.input_root.clone(),
                hash_file(&input).expect("hash should calculate"),
                "text/plain".to_owned(),
                "txt".to_owned(),
                output,
                ParserOptions::default(),
            )
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[derive(Default)]
    struct FakeRunner {
        stdout: Mutex<Vec<u8>>,
        calls: AtomicUsize,
    }

    impl FakeRunner {
        fn with_stdout(stdout: Vec<u8>) -> Self {
            Self {
                stdout: Mutex::new(stdout),
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl WorkerProcessRunner for FakeRunner {
        fn run(
            &self,
            _config: &DoclingWorkerConfig,
            _request: &[u8],
            _cancellation: &CancellationToken,
        ) -> Result<ProcessOutput, DoclingWorkerError> {
            self.calls.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(ProcessOutput {
                stdout: self.stdout.lock().expect("fake lock").clone(),
                stderr: BoundedCapture::default(),
            })
        }
    }

    struct FailingArtifactRunner {
        output_root: PathBuf,
    }

    impl WorkerProcessRunner for FailingArtifactRunner {
        fn run(
            &self,
            _config: &DoclingWorkerConfig,
            _request: &[u8],
            _cancellation: &CancellationToken,
        ) -> Result<ProcessOutput, DoclingWorkerError> {
            std::fs::write(self.output_root.join("document.json"), b"partial")
                .expect("partial artifact should write");
            std::fs::write(self.output_root.join("unknown.keep"), b"keep")
                .expect("unknown artifact should write");
            Err(DoclingWorkerError::new(
                DoclingWorkerErrorCode::ParserWorkerCrashed,
            ))
        }
    }

    #[test]
    fn config_rejects_relative_paths() {
        assert_eq!(
            DoclingWorkerConfig::new("python", ".", ".", ".")
                .expect_err("relative config should fail")
                .code,
            DoclingWorkerErrorCode::ParserEnvironmentMissing
        );
    }

    #[test]
    fn config_debug_redacts_every_path() {
        let workspace = TestWorkspace::new();
        let rendered = format!("{:?}", workspace.config());
        assert!(!rendered.contains(&workspace.root.to_string_lossy().to_string()));
        assert_eq!(rendered.matches("[PRIVATE_PATH]").count(), 4);
    }

    #[test]
    fn request_debug_redacts_paths_and_hash() {
        let workspace = TestWorkspace::new();
        let request = workspace.request();
        let rendered = format!("{request:?}");
        assert!(!rendered.contains(&request.input_path.to_string_lossy().to_string()));
        assert!(!rendered.contains(&request.expected_sha256));
    }

    #[test]
    fn request_serialization_uses_protocol_field_names() {
        let workspace = TestWorkspace::new();
        let value = serde_json::to_value(workspace.request()).expect("request should serialize");
        assert_eq!(value["protocolVersion"], "1");
        assert_eq!(value["operation"], "parse");
        assert!(value.get("inputPath").is_some());
        assert!(value.get("parserOptions").is_some());
    }

    #[test]
    fn pre_cancelled_request_never_invokes_runner() {
        let workspace = TestWorkspace::new();
        let runner = Arc::new(FakeRunner::default());
        let client = DoclingWorkerClient::with_runner(workspace.config(), runner.clone());
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let error = client
            .health(&Uuid::new_v4().to_string(), &cancellation)
            .expect_err("cancelled request should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserCancelled);
        assert_eq!(runner.calls.load(AtomicOrdering::Relaxed), 0);
    }

    #[test]
    fn decodes_successful_health_response() {
        let workspace = TestWorkspace::new();
        let request_id = Uuid::new_v4().to_string();
        let runner = Arc::new(FakeRunner::with_stdout(health_response(&request_id)));
        let client = DoclingWorkerClient::with_runner(workspace.config(), runner);
        let health = client
            .health(&request_id, &CancellationToken::default())
            .expect("health should parse");
        assert_eq!(health.parser_version, "2.117.0");
        assert!(!health.network_listener);
    }

    #[test]
    fn rejects_wrong_protocol_version() {
        assert_protocol_error(json!({
            "protocolVersion": "2",
            "requestId": Uuid::new_v4().to_string(),
            "ok": true,
            "result": health_result()
        }));
    }

    #[test]
    fn rejects_wrong_response_request_id() {
        let expected = Uuid::new_v4().to_string();
        let output = response_bytes(json!({
            "protocolVersion": "1",
            "requestId": Uuid::new_v4().to_string(),
            "ok": true,
            "result": health_result()
        }));
        let error = decode_envelope::<DoclingWorkerHealth>(&expected, &output)
            .expect_err("wrong id should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserProtocolError);
    }

    #[test]
    fn rejects_multiple_stdout_messages() {
        let request_id = Uuid::new_v4().to_string();
        let mut output = health_response(&request_id);
        output.extend_from_slice(b"{}\n");
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output)
            .expect_err("multiple messages should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserProtocolError);
    }

    #[test]
    fn rejects_oversized_stdout() {
        let request_id = Uuid::new_v4().to_string();
        let output = vec![b'x'; MAX_PROTOCOL_MESSAGE_BYTES + 1];
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output)
            .expect_err("oversized response should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserProtocolError);
    }

    #[test]
    fn maps_stable_remote_error() {
        let request_id = Uuid::new_v4().to_string();
        let output = response_bytes(json!({
            "protocolVersion": "1",
            "requestId": request_id,
            "ok": false,
            "error": {
                "code": "parser_models_missing",
                "message": "Required local parser models are missing.",
                "retryable": false
            }
        }));
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output)
            .expect_err("remote error should map");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserModelsMissing);
    }

    #[test]
    fn rejects_remote_error_message_mismatch() {
        let request_id = Uuid::new_v4().to_string();
        let output = response_bytes(json!({
            "protocolVersion": "1",
            "requestId": request_id,
            "ok": false,
            "error": {
                "code": "parser_models_missing",
                "message": "private implementation detail",
                "retryable": false
            }
        }));
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output)
            .expect_err("unsafe message should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserProtocolError);
    }

    #[test]
    fn client_rejects_invalid_uuid_before_runner() {
        let workspace = TestWorkspace::new();
        let mut request = workspace.request();
        request.request_id = "invalid".to_owned();
        let runner = Arc::new(FakeRunner::default());
        let client = DoclingWorkerClient::with_runner(workspace.config(), runner.clone());
        let error = client
            .parse(&request, &CancellationToken::default())
            .expect_err("invalid uuid should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserInvalidInput);
        assert_eq!(runner.calls.load(AtomicOrdering::Relaxed), 0);
    }

    #[test]
    fn client_rejects_input_outside_allowed_root() {
        let workspace = TestWorkspace::new();
        let mut request = workspace.request();
        let outside = workspace.root.join("outside.txt");
        std::fs::write(&outside, b"outside").expect("outside should write");
        request.input_path = outside.clone();
        request.expected_sha256 = hash_file(&outside).expect("hash");
        assert_eq!(
            validate_client_request(&request, &workspace.config())
                .expect_err("outside should fail")
                .code,
            DoclingWorkerErrorCode::ParserInvalidInput
        );
    }

    #[test]
    fn client_rejects_input_hash_mismatch() {
        let workspace = TestWorkspace::new();
        let mut request = workspace.request();
        request.expected_sha256 = "0".repeat(64);
        assert_eq!(
            validate_client_request(&request, &workspace.config())
                .expect_err("hash should fail")
                .code,
            DoclingWorkerErrorCode::ParserIntegrityFailed
        );
    }

    #[test]
    fn client_rejects_output_outside_staging_root() {
        let workspace = TestWorkspace::new();
        let mut request = workspace.request();
        request.output_root = workspace.root.join("outside-output");
        std::fs::create_dir(&request.output_root).expect("outside output should create");
        assert_eq!(
            validate_client_request(&request, &workspace.config())
                .expect_err("outside output should fail")
                .code,
            DoclingWorkerErrorCode::ParserInvalidInput
        );
    }

    #[test]
    fn client_failure_removes_only_known_partial_artifacts() {
        let workspace = TestWorkspace::new();
        let request = workspace.request();
        let runner = Arc::new(FailingArtifactRunner {
            output_root: request.output_root.clone(),
        });
        let client = DoclingWorkerClient::with_runner(workspace.config(), runner);
        let error = client
            .parse(&request, &CancellationToken::default())
            .expect_err("worker crash should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserWorkerCrashed);
        assert!(!request.output_root.join("document.json").exists());
        assert!(request.output_root.join("unknown.keep").exists());
    }

    #[test]
    fn artifact_rejects_parent_traversal() {
        let workspace = TestWorkspace::new();
        let artifact = DoclingArtifact {
            kind: "text".to_owned(),
            relative_path: "../outside.txt".to_owned(),
            size_bytes: 1,
            sha256: "0".repeat(64),
        };
        assert_eq!(
            validate_artifact(&workspace.staging_root, &artifact)
                .expect_err("traversal should fail")
                .code,
            DoclingWorkerErrorCode::ParserIntegrityFailed
        );
    }

    #[test]
    fn artifact_rejects_absolute_path() {
        let workspace = TestWorkspace::new();
        let artifact = DoclingArtifact {
            kind: "text".to_owned(),
            relative_path: workspace
                .root
                .join("outside.txt")
                .to_string_lossy()
                .to_string(),
            size_bytes: 1,
            sha256: "0".repeat(64),
        };
        assert_eq!(
            validate_artifact(&workspace.staging_root, &artifact)
                .expect_err("absolute path should fail")
                .code,
            DoclingWorkerErrorCode::ParserIntegrityFailed
        );
    }

    #[test]
    fn artifact_rejects_size_mismatch() {
        let workspace = TestWorkspace::new();
        let path = workspace.staging_root.join("artifact.txt");
        std::fs::write(&path, b"synthetic").expect("artifact should write");
        let artifact = DoclingArtifact {
            kind: "text".to_owned(),
            relative_path: "artifact.txt".to_owned(),
            size_bytes: 1,
            sha256: hash_file(&path).expect("hash"),
        };
        assert_eq!(
            validate_artifact(&workspace.staging_root, &artifact)
                .expect_err("size should fail")
                .code,
            DoclingWorkerErrorCode::ParserIntegrityFailed
        );
    }

    #[test]
    fn artifact_rejects_hash_mismatch() {
        let workspace = TestWorkspace::new();
        let path = workspace.staging_root.join("artifact.txt");
        std::fs::write(&path, b"synthetic").expect("artifact should write");
        let artifact = DoclingArtifact {
            kind: "text".to_owned(),
            relative_path: "artifact.txt".to_owned(),
            size_bytes: 9,
            sha256: "0".repeat(64),
        };
        assert_eq!(
            validate_artifact(&workspace.staging_root, &artifact)
                .expect_err("hash should fail")
                .code,
            DoclingWorkerErrorCode::ParserIntegrityFailed
        );
    }

    #[test]
    fn worker_source_contains_no_database_or_tauri_command_boundary() {
        let source = include_str!("docling_worker.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source");
        assert!(!production.contains("rusqlite::"));
        assert!(!production.contains("#[tauri::command]"));
        assert!(!production.contains("parse_jobs"));
    }

    #[test]
    fn command_clears_environment_and_contains_no_secret_values() {
        let workspace = TestWorkspace::new();
        let command = build_worker_command(&workspace.config());
        let environment: BTreeMap<_, _> = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
            .collect();
        assert!(!environment.keys().any(|name| {
            let name = name.to_string_lossy().to_uppercase();
            name.contains("TOKEN") || name.contains("KEY") || name.contains("PROXY")
        }));
        let arguments = command
            .get_args()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(arguments, "-I -m meetinglens_docling_worker");
    }

    #[cfg(windows)]
    #[test]
    fn system_runner_executes_real_worker_health_over_stdio() {
        let workspace = TestWorkspace::new();
        let client = DoclingWorkerClient::new(workspace.config());
        let health = client
            .health(&Uuid::new_v4().to_string(), &CancellationToken::default())
            .expect("real worker health should succeed");
        assert_eq!(health.transport, "stdio");
        assert_eq!(health.device, "cpu");
        assert!(!health.remote_services);
        assert!(!health.external_plugins);
    }

    #[cfg(windows)]
    #[test]
    fn system_runner_parses_text_and_revalidates_every_artifact() {
        let workspace = TestWorkspace::new();
        let request = workspace.request();
        let client = DoclingWorkerClient::new(workspace.config());
        let result = client
            .parse(&request, &CancellationToken::default())
            .expect("real text parse should succeed");
        assert_eq!(result.manifest.status, "success");
        assert_eq!(result.manifest.artifacts.len(), 3);
        assert_eq!(result.manifest_artifact.relative_path, "manifest.json");
        assert_eq!(
            std::fs::read_to_string(request.output_root.join("content.txt"))
                .expect("text artifact should read"),
            "Synthetic Rust client content."
        );
    }

    #[cfg(windows)]
    #[test]
    fn system_runner_timeout_terminates_worker() {
        let workspace = TestWorkspace::new();
        let config = workspace
            .config()
            .with_test_limits(Duration::from_millis(100), 256 * 1024 * 1024)
            .with_test_arguments(vec![
                OsString::from("-I"),
                OsString::from("-c"),
                OsString::from("import time; time.sleep(300)"),
            ]);
        let request = response_bytes(json!({"synthetic": true}));
        let error = SystemWorkerProcessRunner
            .run(&config, &request, &CancellationToken::default())
            .expect_err("slow worker should time out");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserTimeout);
    }

    #[cfg(windows)]
    #[test]
    fn system_runner_cancel_terminates_running_worker() {
        let workspace = TestWorkspace::new();
        let config = workspace
            .config()
            .with_test_limits(Duration::from_secs(10), 256 * 1024 * 1024)
            .with_test_arguments(vec![
                OsString::from("-I"),
                OsString::from("-c"),
                OsString::from("import time; time.sleep(300)"),
            ]);
        let cancellation = CancellationToken::default();
        let trigger = cancellation.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            trigger.cancel();
        });
        let error = SystemWorkerProcessRunner
            .run(&config, b"{}\n", &cancellation)
            .expect_err("cancelled worker should stop");
        canceller.join().expect("canceller should join");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserCancelled);
    }

    #[cfg(windows)]
    #[test]
    fn system_runner_maps_abrupt_worker_exit() {
        let workspace = TestWorkspace::new();
        let config = workspace.config().with_test_arguments(vec![
            OsString::from("-I"),
            OsString::from("-c"),
            OsString::from("import os; os._exit(13)"),
        ]);
        let error = SystemWorkerProcessRunner
            .run(&config, b"{}\n", &CancellationToken::default())
            .expect_err("crashed worker should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserWorkerCrashed);
    }

    #[cfg(windows)]
    #[test]
    fn windows_job_memory_limit_is_enforced() {
        let workspace = TestWorkspace::new();
        let request_id = Uuid::new_v4().to_string();
        let script = concat!(
            "import json,sys; r=json.loads(sys.stdin.readline());\n",
            "try:\n x=bytearray(512*1024*1024)\nexcept MemoryError:\n",
            " print(json.dumps({'protocolVersion':'1','requestId':r.get('requestId','')",
            ",'ok':False,'error':{'code':'parser_resource_limit','message':",
            "'The document exceeded a parser resource limit.','retryable':False}}))\n",
            "else:\n sys.exit(17)"
        )
        .to_owned();
        let config = workspace
            .config()
            .with_test_limits(Duration::from_secs(30), 128 * 1024 * 1024)
            .with_test_arguments(vec![
                OsString::from("-I"),
                OsString::from("-c"),
                OsString::from(script),
            ]);
        let request = response_bytes(json!({"requestId": request_id}));
        let output = SystemWorkerProcessRunner
            .run(&config, &request, &CancellationToken::default())
            .expect("memory error helper should respond");
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output.stdout)
            .expect_err("memory response should be stable error");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserResourceLimit);
    }

    #[cfg(windows)]
    #[test]
    fn windows_job_timeout_cleans_child_process_tree() {
        let workspace = TestWorkspace::new();
        let pid_file = workspace.working_directory.join("child.pid");
        let pid_literal = format!("{:?}", pid_file.to_string_lossy().to_string());
        let script = format!(
            "import pathlib,subprocess,sys,time; c=subprocess.Popen([sys.executable,'-I','-c',\
             'import time; time.sleep(300)']); pathlib.Path({pid_literal}).write_text(\
             str(c.pid), encoding='ascii'); time.sleep(300)"
        );
        let config = workspace
            .config()
            .with_test_limits(Duration::from_millis(500), 256 * 1024 * 1024)
            .with_test_arguments(vec![
                OsString::from("-I"),
                OsString::from("-c"),
                OsString::from(script),
            ]);
        let error = SystemWorkerProcessRunner
            .run(&config, b"{}\n", &CancellationToken::default())
            .expect_err("parent should time out");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserTimeout);
        let pid: u32 = std::fs::read_to_string(&pid_file)
            .expect("child pid should be recorded")
            .parse()
            .expect("child pid should parse");
        thread::sleep(Duration::from_millis(100));
        assert!(
            !process_is_running(pid),
            "child process should be terminated"
        );
    }

    fn health_response(request_id: &str) -> Vec<u8> {
        response_bytes(json!({
            "protocolVersion": "1",
            "requestId": request_id,
            "ok": true,
            "result": health_result()
        }))
    }

    fn health_result() -> serde_json::Value {
        json!({
            "status": "ok",
            "service": "meetinglens-docling-worker",
            "workerVersion": "0.1.0",
            "parserVersion": "2.117.0",
            "protocolVersion": "1",
            "pythonVersion": "3.12.8",
            "transport": "stdio",
            "networkListener": false,
            "remoteServices": false,
            "externalPlugins": false,
            "device": "cpu"
        })
    }

    fn response_bytes(value: serde_json::Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&value).expect("response should serialize");
        bytes.push(b'\n');
        bytes
    }

    fn assert_protocol_error(value: serde_json::Value) {
        let request_id = value["requestId"].as_str().expect("request id").to_owned();
        let output = response_bytes(value);
        let error = decode_envelope::<DoclingWorkerHealth>(&request_id, &output)
            .expect_err("response should fail");
        assert_eq!(error.code, DoclingWorkerErrorCode::ParserProtocolError);
    }

    fn worker_python() -> PathBuf {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let executable = if cfg!(windows) {
            manifest
                .parent()
                .expect("workspace parent")
                .join("workers/docling/.venv/Scripts/python.exe")
        } else {
            manifest
                .parent()
                .expect("workspace parent")
                .join("workers/docling/.venv/bin/python")
        };
        executable
            .canonicalize()
            .expect("Docling test environment must be synchronized before Rust tests")
    }

    #[cfg(windows)]
    fn process_is_running(pid: u32) -> bool {
        use windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
        };
        let Ok(process) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
            return false;
        };
        let result = unsafe { WaitForSingleObject(process, 0) };
        unsafe {
            let _ = CloseHandle(process);
        }
        result == WAIT_TIMEOUT
    }
}
