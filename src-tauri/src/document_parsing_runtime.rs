use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::data::{
    Document, DocumentChunkInput, DocumentChunkKind, DocumentError, DocumentResult,
    DocumentService, MeetingService,
};
use crate::docling_worker::{
    CancellationToken, DoclingChunkKind, DoclingWorkerClient, DoclingWorkerConfig,
    DoclingWorkerError, DoclingWorkerErrorCode, DoclingWorkerRequest, ParserOptions,
};

const PARSER_STAGING_DIRECTORY: &str = "document-parser-staging";
const DOCLING_MODELS_DIRECTORY: &str = "MeetingLens/docling/models/2.117.0";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(30);
const WORK_CLEANUP_MAX_DEPTH: usize = 32;
const WORK_CLEANUP_MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParsePhase {
    ValidatingInput,
    Parsing,
    Retrying,
    ValidatingOutput,
    Committing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentParseStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentParseSnapshot {
    pub job_id: String,
    pub document_id: String,
    pub phase: DocumentParsePhase,
    pub status: DocumentParseStatus,
    pub attempt: u8,
    pub error_code: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentParserPolicy {
    pub worker_available: bool,
    pub models_available: bool,
    pub busy: bool,
}

#[derive(Clone)]
pub struct DocumentParsingRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    meetings: MeetingService,
    storage_root: PathBuf,
    staging_root: PathBuf,
    staging_ready: bool,
    models_root: Option<PathBuf>,
    registry: Mutex<JobRegistry>,
    changed: Condvar,
}

struct JobRegistry {
    active: Option<ActiveJob>,
    last: Option<DocumentParseSnapshot>,
}

struct ActiveJob {
    job_id: String,
    meeting_id: String,
    document_id: String,
    cancellation: CancellationToken,
    commit_started: bool,
    snapshot: DocumentParseSnapshot,
}

impl DocumentParsingRuntime {
    pub fn initialize(meetings: MeetingService) -> Self {
        let storage_root = meetings.storage_paths().root().to_path_buf();
        let staging_candidate = meetings
            .storage_paths()
            .resolve_relative(PARSER_STAGING_DIRECTORY);
        let (staging_root, staging_ready) = match staging_candidate {
            Ok(staging_root) => {
                let ready = initialize_staging_root(&storage_root, &staging_root).is_ok();
                (staging_root, ready)
            }
            Err(_) => (storage_root.join(PARSER_STAGING_DIRECTORY), false),
        };
        let models_root = std::env::var_os("LOCALAPPDATA")
            .filter(|root| !root.is_empty())
            .map(|root| PathBuf::from(root).join(DOCLING_MODELS_DIRECTORY));
        let runtime = Self {
            inner: Arc::new(RuntimeInner {
                meetings,
                storage_root,
                staging_root,
                staging_ready,
                models_root,
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

    pub fn policy(&self) -> DocumentParserPolicy {
        let worker_available = self.inner.staging_ready
            && validate_direct_child_directory(&self.inner.storage_root, &self.inner.staging_root)
                .is_ok()
            && resolve_worker_environment().is_some();
        let models_available = self
            .inner
            .models_root
            .as_ref()
            .is_some_and(|root| root.join("meetinglens-docling-models.json").is_file());
        let busy = self
            .inner
            .registry
            .lock()
            .map(|registry| registry.active.is_some())
            .unwrap_or(true);
        DocumentParserPolicy {
            worker_available,
            models_available,
            busy,
        }
    }

    pub fn status(&self, job_id: &str) -> DocumentResult<DocumentParseSnapshot> {
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
        Err(DocumentError::NotFound)
    }

    pub fn cancel(&self, job_id: &str) -> DocumentResult<DocumentParseSnapshot> {
        validate_uuid(job_id)?;
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        if let Some(active) = registry.active.as_mut()
            && active.job_id == job_id
        {
            if active.commit_started {
                return Ok(active.snapshot.clone());
            }
            active.cancellation.cancel();
            active.snapshot.phase = DocumentParsePhase::Cancelled;
            active.snapshot.status = DocumentParseStatus::Cancelled;
            active.snapshot.error_code = Some("parser_cancelled");
            return Ok(active.snapshot.clone());
        }
        if let Some(last) = &registry.last
            && last.job_id == job_id
        {
            return Ok(last.clone());
        }
        Err(DocumentError::NotFound)
    }

    pub fn parse(
        &self,
        document_id: &str,
        mut on_progress: impl FnMut(DocumentParseSnapshot),
    ) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        if !self.inner.staging_ready
            || validate_direct_child_directory(&self.inner.storage_root, &self.inner.staging_root)
                .is_err()
        {
            return Err(parser_error(
                "parser_environment_missing",
                "The document parser environment is missing.",
                false,
            ));
        }
        let current = self.inner.meetings.documents().get(document_id)?;
        let (job_id, cancellation) = self.begin_job(&current)?;
        on_progress(self.update_progress(&job_id, DocumentParsePhase::ValidatingInput, 1, None)?);
        let result = self.parse_registered(&job_id, &cancellation, document_id, &mut on_progress);
        match result {
            Ok(document) => {
                let attempt = self.current_attempt(&job_id).unwrap_or(1);
                let snapshot = self.finish_job(
                    &job_id,
                    DocumentParsePhase::Completed,
                    DocumentParseStatus::Completed,
                    attempt,
                    None,
                )?;
                on_progress(snapshot);
                Ok(document)
            }
            Err(error) => {
                let cancelled = error.code() == "parser_cancelled";
                let snapshot = self.finish_job(
                    &job_id,
                    if cancelled {
                        DocumentParsePhase::Cancelled
                    } else {
                        DocumentParsePhase::Failed
                    },
                    if cancelled {
                        DocumentParseStatus::Cancelled
                    } else {
                        DocumentParseStatus::Failed
                    },
                    self.current_attempt(&job_id).unwrap_or(1),
                    Some(error.code()),
                )?;
                on_progress(snapshot);
                Err(error)
            }
        }
    }

    pub fn cancel_for_document_and_wait(&self, document_id: &str) -> DocumentResult<()> {
        self.cancel_matching_and_wait(|active| active.document_id == document_id)
    }

    pub fn cancel_for_meeting_and_wait(&self, meeting_id: &str) -> DocumentResult<()> {
        self.cancel_matching_and_wait(|active| active.meeting_id == meeting_id)
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

    fn parse_registered(
        &self,
        job_id: &str,
        cancellation: &CancellationToken,
        document_id: &str,
        on_progress: &mut impl FnMut(DocumentParseSnapshot),
    ) -> DocumentResult<Document> {
        let documents = self.inner.meetings.documents();
        let document = documents.begin_parse(document_id)?;

        let mut last_error = None;
        for attempt in 1..=2u8 {
            if cancellation.is_cancelled() {
                let _ = documents.cancel_parse(document_id);
                return Err(cancelled_error());
            }
            if attempt == 2 {
                on_progress(self.update_progress(
                    job_id,
                    DocumentParsePhase::Retrying,
                    attempt,
                    last_error.as_ref().map(DocumentError::code),
                )?);
            }
            let attempt_result =
                self.run_attempt(job_id, attempt, cancellation, &document, on_progress);
            match attempt_result {
                Ok((parser_version, page_count, chunks)) => {
                    if cancellation.is_cancelled() {
                        let _ = documents.cancel_parse(document_id);
                        return Err(cancelled_error());
                    }
                    on_progress(self.begin_commit_or_restore(
                        &documents,
                        document_id,
                        job_id,
                        attempt,
                    )?);
                    return commit_parse_or_fail(
                        &documents,
                        document_id,
                        &parser_version,
                        page_count.map(u64::from),
                        chunks,
                    );
                }
                Err(error) if attempt_was_cancelled(cancellation, &error) => {
                    let _ = documents.cancel_parse(document_id);
                    return Err(cancelled_error());
                }
                Err(error) if error.retryable() && attempt == 1 => {
                    last_error = Some(error);
                }
                Err(error) => {
                    let _ = documents.fail_parse(document_id, error.clone());
                    return Err(error);
                }
            }
        }
        let error = last_error.unwrap_or_else(internal_error);
        let _ = documents.fail_parse(document_id, error.clone());
        Err(error)
    }

    fn run_attempt(
        &self,
        job_id: &str,
        attempt: u8,
        cancellation: &CancellationToken,
        document: &Document,
        on_progress: &mut impl FnMut(DocumentParseSnapshot),
    ) -> DocumentResult<(String, Option<u32>, Vec<DocumentChunkInput>)> {
        let attempt_id = Uuid::new_v4().to_string();
        let attempt_root = self.inner.staging_root.join(&attempt_id);
        let input_root = attempt_root.join("input");
        let output_root = attempt_root.join("output");
        let work_root = attempt_root.join("work");
        if let Err(error) = prepare_attempt_root(
            &self.inner.storage_root,
            &self.inner.staging_root,
            &attempt_root,
            &input_root,
            &output_root,
            &work_root,
        ) {
            let _ = cleanup_attempt_root(
                &self.inner.storage_root,
                &self.inner.staging_root,
                &attempt_root,
            );
            return Err(error);
        }
        let result = (|| {
            let source_path = self
                .inner
                .meetings
                .storage_paths()
                .resolve_relative(&document.storage_relative_path)
                .map_err(|_| DocumentError::IntegrityFailed)?;
            let input_path = input_root.join(format!("document.{}", document.safe_extension));
            copy_registered_document(&source_path, &input_path, document, cancellation)?;
            on_progress(self.update_progress(
                job_id,
                DocumentParsePhase::Parsing,
                attempt,
                None,
            )?);
            let (python, _worker_root) = resolve_worker_environment().ok_or_else(|| {
                parser_error(
                    "parser_environment_missing",
                    "The document parser environment is missing.",
                    false,
                )
            })?;
            let models_root = self.inner.models_root.as_ref().ok_or_else(|| {
                parser_error(
                    "parser_environment_missing",
                    "The document parser environment is missing.",
                    false,
                )
            })?;
            let work_root = worker_compatible_path(&work_root)?;
            let models_root = worker_compatible_path(models_root)?;
            let staging_root = worker_compatible_path(&self.inner.staging_root)?;
            let input_path_for_worker = worker_compatible_path(&input_path)?;
            let input_root_for_worker = worker_compatible_path(&input_root)?;
            let output_root_for_worker = worker_compatible_path(&output_root)?;
            let config = DoclingWorkerConfig::new(python, work_root, models_root, staging_root)
                .map_err(map_worker_error)?;
            let request = DoclingWorkerRequest::new(
                attempt_id,
                document.id.clone(),
                document.meeting_id.clone(),
                input_path_for_worker,
                input_root_for_worker,
                document
                    .sha256
                    .clone()
                    .ok_or(DocumentError::IntegrityFailed)?,
                document
                    .detected_media_type
                    .clone()
                    .ok_or(DocumentError::IntegrityFailed)?,
                document.safe_extension.clone(),
                output_root_for_worker,
                ParserOptions {
                    ocr: matches!(
                        document.safe_extension.as_str(),
                        "pdf" | "png" | "jpg" | "jpeg"
                    ),
                    ..ParserOptions::default()
                },
            );
            let result = DoclingWorkerClient::new(config)
                .parse(&request, cancellation)
                .map_err(map_worker_error)?;
            on_progress(self.update_progress(
                job_id,
                DocumentParsePhase::ValidatingOutput,
                attempt,
                None,
            )?);
            let chunks = result
                .chunks
                .into_iter()
                .map(|chunk| {
                    let kind = match chunk.kind {
                        DoclingChunkKind::Heading => DocumentChunkKind::Heading,
                        DoclingChunkKind::Paragraph => DocumentChunkKind::Paragraph,
                        DoclingChunkKind::ListItem => DocumentChunkKind::ListItem,
                        DoclingChunkKind::Table => DocumentChunkKind::Table,
                    };
                    let section_path_json = serde_json::to_string(&chunk.locator.section_path)
                        .map_err(|_| DocumentError::IntegrityFailed)?;
                    Ok(DocumentChunkInput {
                        kind,
                        text: chunk.text,
                        content_sha256: chunk.content_sha256,
                        page_start: chunk.locator.page_start.map(u64::from),
                        page_end: chunk.locator.page_end.map(u64::from),
                        section_path_json,
                        table_ordinal: chunk.locator.table_ordinal.map(u64::from),
                        chunk_schema_version: u64::from(chunk.schema_version),
                    })
                })
                .collect::<DocumentResult<Vec<_>>>()?;
            Ok((
                result.manifest.parser_version,
                result.manifest.page_count,
                chunks,
            ))
        })();
        let cleanup = cleanup_attempt_root(
            &self.inner.storage_root,
            &self.inner.staging_root,
            &attempt_root,
        );
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    fn begin_job(&self, document: &Document) -> DocumentResult<(String, CancellationToken)> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        if registry.active.is_some() {
            return Err(DocumentError::InvalidState);
        }
        let job_id = Uuid::new_v4().to_string();
        let cancellation = CancellationToken::default();
        registry.active = Some(ActiveJob {
            job_id: job_id.clone(),
            meeting_id: document.meeting_id.clone(),
            document_id: document.id.clone(),
            cancellation: cancellation.clone(),
            commit_started: false,
            snapshot: DocumentParseSnapshot {
                job_id: job_id.clone(),
                document_id: document.id.clone(),
                phase: DocumentParsePhase::ValidatingInput,
                status: DocumentParseStatus::Running,
                attempt: 1,
                error_code: None,
            },
        });
        Ok((job_id, cancellation))
    }

    fn update_progress(
        &self,
        job_id: &str,
        phase: DocumentParsePhase,
        attempt: u8,
        error_code: Option<&'static str>,
    ) -> DocumentResult<DocumentParseSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let active = registry.active.as_mut().ok_or_else(internal_error)?;
        if active.job_id != job_id {
            return Err(internal_error());
        }
        if active.cancellation.is_cancelled() {
            return Ok(active.snapshot.clone());
        }
        active.snapshot.phase = phase;
        active.snapshot.status = DocumentParseStatus::Running;
        active.snapshot.attempt = attempt;
        active.snapshot.error_code = error_code;
        Ok(active.snapshot.clone())
    }

    fn begin_commit(&self, job_id: &str, attempt: u8) -> DocumentResult<DocumentParseSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let active = registry.active.as_mut().ok_or_else(internal_error)?;
        if active.job_id != job_id || active.cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        active.commit_started = true;
        active.snapshot.phase = DocumentParsePhase::Committing;
        active.snapshot.status = DocumentParseStatus::Running;
        active.snapshot.attempt = attempt;
        active.snapshot.error_code = None;
        Ok(active.snapshot.clone())
    }

    fn begin_commit_or_restore(
        &self,
        documents: &DocumentService,
        document_id: &str,
        job_id: &str,
        attempt: u8,
    ) -> DocumentResult<DocumentParseSnapshot> {
        match self.begin_commit(job_id, attempt) {
            Ok(snapshot) => Ok(snapshot),
            Err(error) if error.code() == "parser_cancelled" => {
                let _ = documents.cancel_parse(document_id);
                Err(cancelled_error())
            }
            Err(error) => {
                let _ = documents.fail_parse(document_id, error.clone());
                Err(error)
            }
        }
    }

    fn finish_job(
        &self,
        job_id: &str,
        phase: DocumentParsePhase,
        status: DocumentParseStatus,
        attempt: u8,
        error_code: Option<&'static str>,
    ) -> DocumentResult<DocumentParseSnapshot> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let active = registry.active.take().ok_or_else(internal_error)?;
        if active.job_id != job_id {
            registry.active = Some(active);
            return Err(internal_error());
        }
        let snapshot = DocumentParseSnapshot {
            job_id: job_id.to_string(),
            document_id: active.document_id,
            phase,
            status,
            attempt,
            error_code,
        };
        registry.last = Some(snapshot.clone());
        self.inner.changed.notify_all();
        Ok(snapshot)
    }

    fn current_attempt(&self, job_id: &str) -> Option<u8> {
        self.inner
            .registry
            .lock()
            .ok()?
            .active
            .as_ref()
            .filter(|active| active.job_id == job_id)
            .map(|active| active.snapshot.attempt)
    }

    fn cancel_matching_and_wait(
        &self,
        predicate: impl Fn(&ActiveJob) -> bool,
    ) -> DocumentResult<()> {
        let mut registry = self.inner.registry.lock().map_err(|_| internal_error())?;
        let should_wait = registry.active.as_ref().is_some_and(&predicate);
        if !should_wait {
            return Ok(());
        }
        if let Some(active) = registry.active.as_mut() {
            if active.commit_started {
                return Err(DocumentError::DeletePending);
            }
            active.cancellation.cancel();
        }
        let (registry, timeout) = self
            .inner
            .changed
            .wait_timeout_while(registry, SHUTDOWN_WAIT, |state| {
                state.active.as_ref().is_some_and(&predicate)
            })
            .map_err(|_| internal_error())?;
        if timeout.timed_out() && registry.active.as_ref().is_some_and(predicate) {
            return Err(DocumentError::DeletePending);
        }
        Ok(())
    }

    fn reconcile_staging(&self) {
        if !self.inner.staging_ready
            || validate_direct_child_directory(&self.inner.storage_root, &self.inner.staging_root)
                .is_err()
        {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&self.inner.staging_root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if validate_uuid(&name).is_ok() {
                let _ = cleanup_attempt_root(
                    &self.inner.storage_root,
                    &self.inner.staging_root,
                    &entry.path(),
                );
            }
        }
    }
}

fn initialize_staging_root(storage_root: &Path, staging_root: &Path) -> DocumentResult<()> {
    if staging_root.parent() != Some(storage_root) {
        return Err(DocumentError::IntegrityFailed);
    }
    if !staging_root.exists() {
        std::fs::create_dir(staging_root).map_err(|_| DocumentError::StorageUnavailable)?;
    }
    validate_direct_child_directory(storage_root, staging_root)
}

fn validate_direct_child_directory(parent: &Path, child: &Path) -> DocumentResult<()> {
    if child.parent() != Some(parent) {
        return Err(DocumentError::IntegrityFailed);
    }
    let metadata = std::fs::symlink_metadata(child).map_err(|_| DocumentError::IntegrityFailed)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| DocumentError::IntegrityFailed)?;
    let canonical_child = child
        .canonicalize()
        .map_err(|_| DocumentError::IntegrityFailed)?;
    if canonical_child.parent() != Some(canonical_parent.as_path()) {
        return Err(DocumentError::IntegrityFailed);
    }
    Ok(())
}

fn prepare_attempt_root(
    storage_root: &Path,
    root: &Path,
    attempt: &Path,
    input: &Path,
    output: &Path,
    work: &Path,
) -> DocumentResult<()> {
    validate_direct_child_directory(storage_root, root)?;
    if attempt.parent() != Some(root) {
        return Err(DocumentError::IntegrityFailed);
    }
    std::fs::create_dir(attempt).map_err(|_| DocumentError::StorageUnavailable)?;
    validate_direct_child_directory(root, attempt)?;
    std::fs::create_dir(input).map_err(|_| DocumentError::StorageUnavailable)?;
    std::fs::create_dir(output).map_err(|_| DocumentError::StorageUnavailable)?;
    std::fs::create_dir(work).map_err(|_| DocumentError::StorageUnavailable)?;
    validate_direct_child_directory(attempt, input)?;
    validate_direct_child_directory(attempt, output)?;
    validate_direct_child_directory(attempt, work)?;
    Ok(())
}

fn copy_registered_document(
    source: &Path,
    target: &Path,
    document: &Document,
    cancellation: &CancellationToken,
) -> DocumentResult<()> {
    let expected_size = document.size_bytes.ok_or(DocumentError::IntegrityFailed)?;
    let expected_sha = document
        .sha256
        .as_deref()
        .ok_or(DocumentError::IntegrityFailed)?;
    let source_metadata =
        std::fs::symlink_metadata(source).map_err(|_| DocumentError::IntegrityFailed)?;
    if !source_metadata.is_file() || is_reparse_point(&source_metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    let source = OpenOptions::new()
        .read(true)
        .open(source)
        .map_err(|_| DocumentError::IntegrityFailed)?;
    let mut reader = BufReader::new(source);
    let target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| DocumentError::StorageUnavailable)?;
    let mut writer = BufWriter::new(target);
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DocumentError::IntegrityFailed)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| DocumentError::TooLarge)?)
            .ok_or(DocumentError::TooLarge)?;
        if total > expected_size {
            return Err(DocumentError::IntegrityFailed);
        }
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|_| DocumentError::StorageUnavailable)?;
    }
    writer
        .flush()
        .map_err(|_| DocumentError::StorageUnavailable)?;
    if total != expected_size || encode_hex(&hasher.finalize()) != expected_sha {
        return Err(DocumentError::IntegrityFailed);
    }
    Ok(())
}

fn cleanup_attempt_root(storage_root: &Path, root: &Path, attempt: &Path) -> DocumentResult<()> {
    validate_direct_child_directory(storage_root, root)?;
    if attempt.parent() != Some(root) {
        return Err(DocumentError::IntegrityFailed);
    }
    let name = attempt
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(DocumentError::IntegrityFailed)?;
    validate_uuid(name)?;
    if !attempt.exists() {
        return Ok(());
    }
    let metadata =
        std::fs::symlink_metadata(attempt).map_err(|_| DocumentError::IntegrityFailed)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    validate_direct_child_directory(root, attempt)?;
    let entries = std::fs::read_dir(attempt).map_err(|_| DocumentError::StorageUnavailable)?;
    for entry in entries {
        let path = entry.map_err(|_| DocumentError::StorageUnavailable)?.path();
        let leaf = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(DocumentError::IntegrityFailed)?;
        if leaf == "input" {
            cleanup_input(&path)?;
        } else if leaf == "output" {
            cleanup_output(&path)?;
        } else if leaf == "work" {
            let mut entries_seen = 0usize;
            cleanup_work_tree(&path, 0, &mut entries_seen)?;
        } else {
            return Err(DocumentError::IntegrityFailed);
        }
    }
    std::fs::remove_dir(attempt).map_err(|_| DocumentError::StorageUnavailable)
}

fn cleanup_work_tree(path: &Path, depth: usize, entries_seen: &mut usize) -> DocumentResult<()> {
    if depth > WORK_CLEANUP_MAX_DEPTH {
        return Err(DocumentError::IntegrityFailed);
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| DocumentError::IntegrityFailed)?;
    if is_reparse_point(&metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    *entries_seen = entries_seen
        .checked_add(1)
        .ok_or(DocumentError::IntegrityFailed)?;
    if *entries_seen > WORK_CLEANUP_MAX_ENTRIES {
        return Err(DocumentError::IntegrityFailed);
    }
    if metadata.is_file() {
        return std::fs::remove_file(path).map_err(|_| DocumentError::StorageUnavailable);
    }
    if !metadata.is_dir() {
        return Err(DocumentError::IntegrityFailed);
    }
    for entry in std::fs::read_dir(path).map_err(|_| DocumentError::StorageUnavailable)? {
        let child = entry.map_err(|_| DocumentError::StorageUnavailable)?.path();
        cleanup_work_tree(&child, depth + 1, entries_seen)?;
    }
    std::fs::remove_dir(path).map_err(|_| DocumentError::StorageUnavailable)
}

fn cleanup_input(input: &Path) -> DocumentResult<()> {
    let metadata = std::fs::symlink_metadata(input).map_err(|_| DocumentError::IntegrityFailed)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    let mut entries = std::fs::read_dir(input).map_err(|_| DocumentError::StorageUnavailable)?;
    let path = entries
        .next()
        .transpose()
        .map_err(|_| DocumentError::StorageUnavailable)?
        .map(|entry| entry.path());
    if entries.next().is_some() {
        return Err(DocumentError::IntegrityFailed);
    }
    if let Some(path) = path {
        let leaf = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(DocumentError::IntegrityFailed)?;
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .ok_or(DocumentError::IntegrityFailed)?;
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|_| DocumentError::IntegrityFailed)?;
        if !leaf.starts_with("document.")
            || !crate::data::ALLOWED_DOCUMENT_EXTENSIONS.contains(&extension)
            || !metadata.is_file()
            || is_reparse_point(&metadata)
        {
            return Err(DocumentError::IntegrityFailed);
        }
        std::fs::remove_file(path).map_err(|_| DocumentError::StorageUnavailable)?;
    }
    std::fs::remove_dir(input).map_err(|_| DocumentError::StorageUnavailable)
}

fn cleanup_output(output: &Path) -> DocumentResult<()> {
    let metadata = std::fs::symlink_metadata(output).map_err(|_| DocumentError::IntegrityFailed)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(DocumentError::IntegrityFailed);
    }
    const ALLOWED: &[&str] = &[
        "document.json",
        "content.md",
        "content.txt",
        "chunks.jsonl",
        "manifest.json",
        ".document.json.tmp",
        ".content.md.tmp",
        ".content.txt.tmp",
        ".chunks.jsonl.tmp",
        ".manifest.json.tmp",
    ];
    for entry in std::fs::read_dir(output).map_err(|_| DocumentError::StorageUnavailable)? {
        let path = entry.map_err(|_| DocumentError::StorageUnavailable)?.path();
        let leaf = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(DocumentError::IntegrityFailed)?;
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|_| DocumentError::IntegrityFailed)?;
        if !ALLOWED.contains(&leaf) || !metadata.is_file() || is_reparse_point(&metadata) {
            return Err(DocumentError::IntegrityFailed);
        }
        std::fs::remove_file(path).map_err(|_| DocumentError::StorageUnavailable)?;
    }
    std::fs::remove_dir(output).map_err(|_| DocumentError::StorageUnavailable)
}

fn commit_parse_or_fail(
    documents: &DocumentService,
    document_id: &str,
    parser_version: &str,
    page_count: Option<u64>,
    chunks: Vec<DocumentChunkInput>,
) -> DocumentResult<Document> {
    match documents.commit_parse(document_id, parser_version, page_count, chunks) {
        Ok(document) => Ok(document),
        Err(error) => {
            let _ = documents.fail_parse(document_id, error.clone());
            Err(error)
        }
    }
}

#[cfg(windows)]
fn worker_compatible_path(path: &Path) -> DocumentResult<PathBuf> {
    use std::path::{Component, Prefix};

    if !path.is_absolute() {
        return Err(DocumentError::IntegrityFailed);
    }
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            Prefix::UNC(..)
            | Prefix::VerbatimUNC(..)
            | Prefix::DeviceNS(..)
            | Prefix::Verbatim(..) => return Err(DocumentError::IntegrityFailed),
        },
        _ => return Err(DocumentError::IntegrityFailed),
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(DocumentError::IntegrityFailed);
    }
    let mut normalized = PathBuf::from(format!("{}:\\", char::from(drive)));
    for component in components {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => return Err(DocumentError::IntegrityFailed),
        }
    }
    Ok(normalized)
}

#[cfg(not(windows))]
fn worker_compatible_path(path: &Path) -> DocumentResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Err(DocumentError::IntegrityFailed)
    }
}

fn resolve_worker_environment() -> Option<(PathBuf, PathBuf)> {
    #[cfg(debug_assertions)]
    {
        let worker = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("workers/docling");
        let metadata = std::fs::symlink_metadata(&worker).ok()?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return None;
        }
        let worker = worker.canonicalize().ok()?;
        #[cfg(windows)]
        let python = worker.join(".venv/Scripts/python.exe");
        #[cfg(not(windows))]
        let python = worker.join(".venv/bin/python");
        let metadata = std::fs::symlink_metadata(&python).ok()?;
        if !metadata.is_file() || is_reparse_point(&metadata) {
            return None;
        }
        let python = python.canonicalize().ok()?;
        if worker.join("pyproject.toml").is_file() && python.starts_with(&worker) {
            return Some((python, worker));
        }
    }
    None
}

fn map_worker_error(error: DoclingWorkerError) -> DocumentError {
    let code = match error.code {
        DoclingWorkerErrorCode::ParserUnavailable => "parser_unavailable",
        DoclingWorkerErrorCode::ParserEnvironmentMissing => "parser_environment_missing",
        DoclingWorkerErrorCode::ParserDependencyIncompatible => "parser_dependency_incompatible",
        DoclingWorkerErrorCode::ParserModelsMissing => "parser_models_missing",
        DoclingWorkerErrorCode::ParserModelsIncompatible => "parser_models_incompatible",
        DoclingWorkerErrorCode::ParserInvalidInput => "parser_invalid_input",
        DoclingWorkerErrorCode::ParserUnsupportedType => "parser_unsupported_type",
        DoclingWorkerErrorCode::ParserOcrUnavailable => "parser_ocr_unavailable",
        DoclingWorkerErrorCode::ParserProtocolError => "parser_protocol_error",
        DoclingWorkerErrorCode::ParserWorkerStartFailed => "parser_worker_start_failed",
        DoclingWorkerErrorCode::ParserWorkerCrashed => "parser_worker_crashed",
        DoclingWorkerErrorCode::ParserTimeout => "parser_timeout",
        DoclingWorkerErrorCode::ParserCancelled => "parser_cancelled",
        DoclingWorkerErrorCode::ParserResourceLimit => "parser_resource_limit",
        DoclingWorkerErrorCode::ParserPageLimitExceeded => "parser_page_limit_exceeded",
        DoclingWorkerErrorCode::ParserOutputTooLarge => "parser_output_too_large",
        DoclingWorkerErrorCode::ParserPartialResult => "parser_partial_result",
        DoclingWorkerErrorCode::ParserIntegrityFailed => "parser_integrity_failed",
        DoclingWorkerErrorCode::ParserFailed => "parser_failed",
        DoclingWorkerErrorCode::InternalError => "internal_error",
    };
    parser_error(code, error.message, error.retryable)
}

fn parser_error(code: &'static str, message: &'static str, retryable: bool) -> DocumentError {
    DocumentError::Parser {
        code,
        message,
        retryable,
    }
}

fn cancelled_error() -> DocumentError {
    parser_error("parser_cancelled", "Document parsing was cancelled.", false)
}

fn attempt_was_cancelled(cancellation: &CancellationToken, error: &DocumentError) -> bool {
    cancellation.is_cancelled() || error.code() == "parser_cancelled"
}

fn internal_error() -> DocumentError {
    DocumentError::Internal
}

fn validate_uuid(value: &str) -> DocumentResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| DocumentError::InvalidState)?;
    if parsed.to_string() != value {
        return Err(DocumentError::InvalidState);
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{CreateMeeting, DocumentStatus};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "meetinglens-document-parser-runtime-{}",
                Uuid::new_v4()
            )))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn synthetic_document() -> Document {
        Document {
            id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            original_filename: "synthetic.txt".to_string(),
            safe_extension: "txt".to_string(),
            detected_media_type: Some("text/plain".to_string()),
            size_bytes: Some(9),
            sha256: Some(
                "1bc8184af7b0d10e29ad7e3c20f47f10b823948e5e4a3e0014eec9b2cead921b".to_string(),
            ),
            status: DocumentStatus::Staged,
            storage_relative_path: format!(
                "meeting-data/meetings/{}/documents/{}.txt",
                Uuid::new_v4(),
                Uuid::new_v4()
            ),
            imported_at: Some("2026-08-26T00:00:00.000Z".to_string()),
            updated_at: "2026-08-26T00:00:00.000Z".to_string(),
            failure_code: None,
            failure_message_safe: None,
            parser_version_optional: None,
            page_count_optional: None,
            created_at: "2026-08-26T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn registry_exposes_one_cancellable_job_and_retains_only_safe_status() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let runtime = DocumentParsingRuntime::initialize(meetings);
        let document = synthetic_document();
        let (job_id, token) = runtime.begin_job(&document).expect("job should register");
        assert!(matches!(
            runtime.begin_job(&document),
            Err(DocumentError::InvalidState)
        ));
        let running = runtime.status(&job_id).expect("running status should load");
        assert_eq!(running.document_id, document.id);
        assert_eq!(running.phase, DocumentParsePhase::ValidatingInput);
        assert_eq!(running.status, DocumentParseStatus::Running);
        assert_eq!(running.attempt, 1);
        assert_eq!(running.error_code, None);

        let cancelled = runtime.cancel(&job_id).expect("job should cancel");
        assert!(token.is_cancelled());
        assert_eq!(cancelled.status, DocumentParseStatus::Cancelled);
        assert_eq!(cancelled.error_code, Some("parser_cancelled"));
        let terminal = runtime
            .finish_job(
                &job_id,
                DocumentParsePhase::Cancelled,
                DocumentParseStatus::Cancelled,
                1,
                Some("parser_cancelled"),
            )
            .expect("job should finish");
        assert_eq!(runtime.status(&job_id), Ok(terminal.clone()));
        assert_eq!(runtime.cancel(&job_id), Ok(terminal));
    }

    #[test]
    fn staging_reconciliation_removes_only_the_fixed_known_layout() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let runtime = DocumentParsingRuntime::initialize(meetings);
        let attempt_id = Uuid::new_v4().to_string();
        let attempt = runtime.inner.staging_root.join(&attempt_id);
        let input = attempt.join("input");
        let output = attempt.join("output");
        let work = attempt.join("work");
        prepare_attempt_root(
            &runtime.inner.storage_root,
            &runtime.inner.staging_root,
            &attempt,
            &input,
            &output,
            &work,
        )
        .expect("attempt layout should create");
        std::fs::write(input.join("document.txt"), b"synthetic").expect("input should write");
        std::fs::create_dir(work.join("cache")).expect("worker cache should create");
        std::fs::write(work.join("cache/synthetic.tmp"), b"synthetic")
            .expect("worker temporary file should write");
        for artifact in [
            "document.json",
            "content.md",
            "content.txt",
            "chunks.jsonl",
            "manifest.json",
        ] {
            std::fs::write(output.join(artifact), b"synthetic")
                .expect("known artifact should write");
        }
        runtime.reconcile_staging();
        assert!(!attempt.exists());

        let unknown_id = Uuid::new_v4().to_string();
        let unknown = runtime.inner.staging_root.join(unknown_id);
        std::fs::create_dir(&unknown).expect("unknown attempt should create");
        std::fs::write(unknown.join("unexpected.bin"), b"synthetic")
            .expect("unknown file should write");
        runtime.reconcile_staging();
        assert!(
            unknown.exists(),
            "unknown layouts must fail closed, not recurse"
        );
    }

    #[test]
    fn failed_commit_transitions_document_out_of_parsing() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic failed document commit".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create");
        let source_path = std::env::temp_dir().join(format!(
            "meetinglens-document-parser-failed-commit-{}.txt",
            Uuid::new_v4()
        ));
        std::fs::write(&source_path, b"Synthetic parser commit failure.")
            .expect("synthetic source should write");
        let documents = meetings.documents();
        let document = documents
            .import_document(&meeting.id, &source_path)
            .expect("synthetic document should import");
        std::fs::remove_file(&source_path).expect("synthetic source should clean up");
        documents
            .begin_parse(&document.id)
            .expect("document should enter parsing");

        let error = commit_parse_or_fail(&documents, &document.id, "2.117.0", Some(1), Vec::new())
            .expect_err("empty parser output must fail");
        assert_eq!(error, DocumentError::IntegrityFailed);
        let failed = documents
            .get(&document.id)
            .expect("failed document should remain readable");
        assert_eq!(failed.status, DocumentStatus::ParseFailed);
        assert_eq!(
            failed.failure_code.as_deref(),
            Some("document_integrity_failed")
        );
    }

    #[test]
    fn user_cancellation_wins_a_race_with_a_worker_failure() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let worker_failure = parser_error(
            "parser_worker_crashed",
            "The document parser stopped unexpectedly.",
            true,
        );
        assert!(attempt_was_cancelled(&cancellation, &worker_failure));
    }

    #[test]
    fn cancellation_at_the_commit_boundary_restores_the_staged_document() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic commit-boundary cancellation".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create");
        let source_path = std::env::temp_dir().join(format!(
            "meetinglens-document-parser-cancel-commit-{}.txt",
            Uuid::new_v4()
        ));
        std::fs::write(&source_path, b"Synthetic commit cancellation.")
            .expect("synthetic source should write");
        let documents = meetings.documents();
        let document = documents
            .import_document(&meeting.id, &source_path)
            .expect("synthetic document should import");
        std::fs::remove_file(&source_path).expect("synthetic source should clean up");
        let runtime = DocumentParsingRuntime::initialize(meetings);
        let (job_id, _) = runtime.begin_job(&document).expect("job should register");
        documents
            .begin_parse(&document.id)
            .expect("document should enter parsing");
        runtime.cancel(&job_id).expect("job should cancel");

        assert_eq!(
            runtime.begin_commit_or_restore(&documents, &document.id, &job_id, 1),
            Err(cancelled_error())
        );
        assert_eq!(
            documents
                .get(&document.id)
                .expect("document should remain readable")
                .status,
            DocumentStatus::Staged
        );
        runtime
            .finish_job(
                &job_id,
                DocumentParsePhase::Cancelled,
                DocumentParseStatus::Cancelled,
                1,
                Some("parser_cancelled"),
            )
            .expect("test job should finish");
    }

    #[cfg(windows)]
    #[test]
    fn preexisting_staging_reparse_point_disables_the_parser() {
        use std::os::windows::fs::symlink_dir;

        let directory = TestDirectory::new();
        let outside = TestDirectory::new();
        std::fs::create_dir_all(&directory.0).expect("app-data root should create");
        std::fs::create_dir_all(&outside.0).expect("outside root should create");
        let staging = directory.0.join(PARSER_STAGING_DIRECTORY);
        if symlink_dir(&outside.0, &staging).is_err() {
            return;
        }
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let runtime = DocumentParsingRuntime::initialize(meetings);
        assert!(!runtime.inner.staging_ready);
        assert!(!runtime.policy().worker_available);
        std::fs::remove_dir(&staging).expect("test reparse point should clean up");
    }

    #[cfg(windows)]
    #[test]
    fn worker_paths_normalize_local_verbatim_drives_and_reject_network_namespaces() {
        assert_eq!(
            worker_compatible_path(Path::new(r"\\?\C:\safe\root")),
            Ok(PathBuf::from(r"C:\safe\root"))
        );
        assert!(worker_compatible_path(Path::new(r"\\?\UNC\server\share")).is_err());
        assert!(worker_compatible_path(Path::new(r"\\server\share")).is_err());
        assert!(worker_compatible_path(Path::new(r"\\.\PIPE\meetinglens")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn real_text_worker_commits_chunks_through_the_document_service() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic document parser runtime".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create");
        let source_path = std::env::temp_dir().join(format!(
            "meetinglens-document-parser-source-{}.txt",
            Uuid::new_v4()
        ));
        std::fs::write(&source_path, b"Synthetic local document parser content.")
            .expect("synthetic source should write");
        let document = meetings
            .documents()
            .import_document(&meeting.id, &source_path)
            .expect("synthetic document should import");
        std::fs::remove_file(&source_path).expect("synthetic source should clean up");
        let mut runtime = DocumentParsingRuntime::initialize(meetings.clone());
        let missing_models = directory.0.join("models-not-provisioned");
        assert!(!missing_models.exists());
        Arc::get_mut(&mut runtime.inner)
            .expect("test runtime should have one owner")
            .models_root = Some(missing_models);
        let mut progress = Vec::new();
        let parsed = runtime
            .parse(&document.id, |snapshot| progress.push(snapshot))
            .expect("real local text parse should complete");
        assert_eq!(parsed.status, DocumentStatus::Parsed);
        assert_eq!(parsed.parser_version_optional.as_deref(), Some("2.117.0"));
        let chunks = meetings
            .documents()
            .chunks(&document.id)
            .expect("parsed chunks should load");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Synthetic local document parser content.");
        assert!(
            progress
                .iter()
                .any(|item| item.phase == DocumentParsePhase::Parsing)
        );
        assert_eq!(
            progress.first().map(|item| item.phase),
            Some(DocumentParsePhase::ValidatingInput),
            "the first event must expose the job id before database validation"
        );
        assert!(
            progress
                .iter()
                .any(|item| item.phase == DocumentParsePhase::Committing)
        );
        assert_eq!(
            progress.last().map(|item| item.status),
            Some(DocumentParseStatus::Completed)
        );
        assert_eq!(
            std::fs::read_dir(&runtime.inner.staging_root)
                .expect("staging root should read")
                .count(),
            0,
            "successful parsing must remove its staging attempt"
        );
    }
}
