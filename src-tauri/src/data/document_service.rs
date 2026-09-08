use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::TransactionBehavior;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::database::Database;
use super::document_detection::{detect_media_type, validate_source_name};
use super::document_error::{DocumentError, DocumentResult};
use super::document_model::{
    DOCUMENT_CHUNK_MAX_BYTES, DOCUMENT_CHUNK_MAX_COUNT, DOCUMENT_CHUNK_SCHEMA_VERSION,
    DOCUMENT_CHUNKS_MAX_TOTAL_BYTES, Document, DocumentChunk, DocumentChunkInput,
    DocumentChunkKind, DocumentImportOutcome, DocumentImportPhase, DocumentImportPolicy,
    DocumentImportProgress, DocumentStatus, ReconcileReport,
};
use super::document_repository::{self, NewDocumentChunk, NewImport};
use super::error::{DataError, DataResult};
use super::model::MeetingStatus;
use super::repository;
use super::summary_repository;

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const COPY_PROGRESS_INTERVAL_BYTES: u64 = 1024 * 1024;
const MAX_DELETE_ATTEMPTS: u64 = 3;

struct ImportProgressContext<'a> {
    document_id: &'a str,
    file_index: usize,
    total_files: usize,
    original_filename: &'a str,
}

#[derive(Clone, Debug)]
pub struct DocumentService {
    database: Database,
    policy: DocumentImportPolicy,
    #[cfg(test)]
    fail_delete_after_move: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DocumentService {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database,
            policy: DocumentImportPolicy::default(),
            #[cfg(test)]
            fail_delete_after_move: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_policy(database: Database, policy: DocumentImportPolicy) -> Self {
        Self {
            database,
            policy,
            fail_delete_after_move: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    fn inject_delete_failure_after_move(&self) {
        self.fail_delete_after_move
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub const fn policy(&self) -> DocumentImportPolicy {
        self.policy
    }

    pub fn import_batch(
        &self,
        meeting_id: &str,
        source_paths: &[PathBuf],
    ) -> DocumentResult<Vec<Document>> {
        if source_paths.is_empty() || source_paths.len() > self.policy.max_batch_files {
            return Err(DocumentError::QuotaExceeded);
        }

        let mut imported = Vec::with_capacity(source_paths.len());
        for source_path in source_paths {
            imported.push(self.import_document(meeting_id, source_path)?);
        }
        Ok(imported)
    }

    pub fn import_document(
        &self,
        meeting_id: &str,
        source_path: impl AsRef<Path>,
    ) -> DocumentResult<Document> {
        let source_path = source_path.as_ref();
        let original_filename = progress_filename(source_path, self.policy.max_filename_bytes);
        let document_id = Uuid::new_v4().to_string();
        self.import_document_with_progress(
            meeting_id,
            source_path,
            ImportProgressContext {
                document_id: &document_id,
                file_index: 1,
                total_files: 1,
                original_filename: &original_filename,
            },
            &mut |_| {},
        )
    }

    pub fn import_batch_with_progress<F>(
        &self,
        meeting_id: &str,
        source_paths: &[PathBuf],
        mut observer: F,
    ) -> DocumentResult<Vec<DocumentImportOutcome>>
    where
        F: FnMut(DocumentImportProgress),
    {
        if source_paths.is_empty() || source_paths.len() > self.policy.max_batch_files {
            return Err(DocumentError::QuotaExceeded);
        }
        validate_uuid(meeting_id)?;
        self.ensure_meeting_accepts_import(meeting_id)?;

        let total_files = source_paths.len();
        let mut outcomes = Vec::with_capacity(total_files);
        for (offset, source_path) in source_paths.iter().enumerate() {
            let file_index = offset + 1;
            let original_filename = progress_filename(source_path, self.policy.max_filename_bytes);
            let document_id = Uuid::new_v4().to_string();
            observer(import_progress(
                file_index,
                total_files,
                &original_filename,
                DocumentImportPhase::Selected,
                0,
                None,
                None,
                None,
            ));
            match self.import_document_with_progress(
                meeting_id,
                source_path,
                ImportProgressContext {
                    document_id: &document_id,
                    file_index,
                    total_files,
                    original_filename: &original_filename,
                },
                &mut observer,
            ) {
                Ok(document) => outcomes.push(DocumentImportOutcome::Imported(Box::new(document))),
                Err(DocumentError::AlreadyImported { document_id }) => {
                    outcomes.push(DocumentImportOutcome::Duplicate {
                        document_id,
                        original_filename,
                    });
                }
                Err(error) => outcomes.push(DocumentImportOutcome::Failed {
                    document_id,
                    original_filename,
                    error,
                }),
            }
        }
        observer(import_progress(
            total_files,
            total_files,
            "",
            DocumentImportPhase::Complete,
            0,
            None,
            None,
            None,
        ));
        Ok(outcomes)
    }

    fn import_document_with_progress<F>(
        &self,
        meeting_id: &str,
        source_path: &Path,
        progress_context: ImportProgressContext<'_>,
        observer: &mut F,
    ) -> DocumentResult<Document>
    where
        F: FnMut(DocumentImportProgress),
    {
        let ImportProgressContext {
            document_id,
            file_index,
            total_files,
            original_filename,
        } = progress_context;
        let mut bytes_processed = 0;
        let mut total_bytes = None;
        let progress_document_id = Some(document_id.to_string());
        observer(import_progress(
            file_index,
            total_files,
            original_filename,
            DocumentImportPhase::Validating,
            bytes_processed,
            total_bytes,
            None,
            None,
        ));

        let result = (|| {
            validate_uuid(meeting_id)?;
            let _guard = self.database.mutation_guard().map_err(map_data_error)?;
            self.ensure_meeting_accepts_import(meeting_id)?;
            let source_name = validate_source_name(source_path, self.policy.max_filename_bytes)?;
            let source_metadata = validate_source_file(source_path)?;
            total_bytes = Some(source_metadata.len());
            if source_metadata.len() == 0 {
                return Err(DocumentError::Empty);
            }
            if source_metadata.len() > self.policy.max_file_bytes {
                return Err(DocumentError::TooLarge);
            }

            let staging_relative = self
                .database
                .paths()
                .staging_relative(document_id)
                .map_err(map_data_error)?;
            let staging_path = self
                .database
                .paths()
                .resolve_relative(&staging_relative)
                .map_err(map_data_error)?;

            self.insert_importing(
                document_id,
                meeting_id,
                &source_name.original_filename,
                &source_name.safe_extension,
                &staging_relative,
            )?;

            observer(import_progress(
                file_index,
                total_files,
                &source_name.original_filename,
                DocumentImportPhase::Copying,
                bytes_processed,
                total_bytes,
                progress_document_id.clone(),
                None,
            ));
            let copied = copy_and_hash(
                source_path,
                &staging_path,
                source_metadata,
                self.policy.max_file_bytes,
                |processed| {
                    bytes_processed = processed;
                    observer(import_progress(
                        file_index,
                        total_files,
                        &source_name.original_filename,
                        DocumentImportPhase::Copying,
                        processed,
                        total_bytes,
                        progress_document_id.clone(),
                        None,
                    ));
                },
            );
            let (size_bytes, sha256) = match copied {
                Ok(value) => value,
                Err(error) => {
                    self.fail_import_and_cleanup(document_id, &staging_path, &error);
                    return Err(error);
                }
            };

            let detected_media_type =
                match detect_media_type(&staging_path, &source_name.safe_extension) {
                    Ok(value) => value,
                    Err(error) => {
                        self.fail_import_and_cleanup(document_id, &staging_path, &error);
                        return Err(error);
                    }
                };
            let (verified_size, verified_hash) =
                match hash_file(&staging_path, self.policy.max_file_bytes) {
                    Ok(value) if value.0 == size_bytes && value.1 == sha256 => value,
                    Ok(_) | Err(_) => {
                        let error = DocumentError::IntegrityFailed;
                        self.fail_import_and_cleanup(document_id, &staging_path, &error);
                        return Err(error);
                    }
                };

            observer(import_progress(
                file_index,
                total_files,
                &source_name.original_filename,
                DocumentImportPhase::Committing,
                bytes_processed,
                total_bytes,
                progress_document_id.clone(),
                None,
            ));
            match self.finish_import(
                document_id,
                meeting_id,
                &source_name.safe_extension,
                detected_media_type,
                verified_size,
                &verified_hash,
                &staging_path,
            ) {
                Ok(document) => Ok(document),
                Err(error @ DocumentError::AlreadyImported { .. }) => {
                    let _ = std::fs::remove_file(&staging_path);
                    Err(error)
                }
                Err(error) => {
                    self.fail_import_and_cleanup(document_id, &staging_path, &error);
                    Err(error)
                }
            }
        })();

        match &result {
            Ok(document) => observer(import_progress(
                file_index,
                total_files,
                &document.original_filename,
                DocumentImportPhase::Imported,
                document.size_bytes.unwrap_or(bytes_processed),
                document.size_bytes.or(total_bytes),
                Some(document.id.clone()),
                None,
            )),
            Err(DocumentError::AlreadyImported { document_id }) => observer(import_progress(
                file_index,
                total_files,
                original_filename,
                DocumentImportPhase::Duplicate,
                bytes_processed,
                total_bytes,
                Some(document_id.clone()),
                Some(
                    DocumentError::AlreadyImported {
                        document_id: document_id.clone(),
                    }
                    .code(),
                ),
            )),
            Err(error) => observer(import_progress(
                file_index,
                total_files,
                original_filename,
                DocumentImportPhase::Failed,
                bytes_processed,
                total_bytes,
                progress_document_id,
                Some(error.code()),
            )),
        }
        result
    }

    pub fn get(&self, document_id: &str) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        document_repository::get(
            &self.database.connect().map_err(map_data_error)?,
            document_id,
        )
    }

    pub fn list(&self, meeting_id: &str) -> DocumentResult<Vec<Document>> {
        validate_uuid(meeting_id)?;
        repository::get_including_deleted(
            &self.database.connect().map_err(map_data_error)?,
            meeting_id,
        )
        .map_err(map_data_error)?;
        document_repository::list(
            &self.database.connect().map_err(map_data_error)?,
            meeting_id,
        )
    }

    pub fn begin_parse(&self, document_id: &str) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let document = document_repository::get(&connection, document_id)?;
        let meeting = repository::get_including_deleted(&connection, &document.meeting_id)
            .map_err(map_data_error)?;
        if !matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) || document.status != DocumentStatus::Staged
        {
            return Err(DocumentError::InvalidState);
        }
        let path = self
            .database
            .paths()
            .resolve_relative(&document.storage_relative_path)
            .map_err(map_data_error)?;
        verify_committed_document(&document, &path)?;

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        document_repository::begin_parse(&transaction, document_id)?;
        let parsing = document_repository::get(&transaction, document_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        Ok(parsing)
    }

    pub fn commit_parse(
        &self,
        document_id: &str,
        parser_version: &str,
        page_count: Option<u64>,
        chunks: Vec<DocumentChunkInput>,
    ) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        validate_parser_output(parser_version, page_count, &chunks)?;
        let chunk_ids = (0..chunks.len())
            .map(|_| Uuid::new_v4().to_string())
            .collect::<Vec<_>>();
        let new_chunks = chunks
            .iter()
            .enumerate()
            .map(|(ordinal, input)| {
                Ok(NewDocumentChunk {
                    id: &chunk_ids[ordinal],
                    ordinal: u64::try_from(ordinal).map_err(|_| DocumentError::IntegrityFailed)?,
                    input,
                })
            })
            .collect::<DocumentResult<Vec<_>>>()?;

        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        let document = document_repository::get(&transaction, document_id)?;
        let meeting = repository::get_including_deleted(&transaction, &document.meeting_id)
            .map_err(map_data_error)?;
        if !matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) || document.status != DocumentStatus::Parsing
        {
            return Err(DocumentError::InvalidState);
        }
        document_repository::commit_parse(
            &transaction,
            &document,
            parser_version,
            page_count,
            &new_chunks,
        )?;
        let parsed = document_repository::get(&transaction, document_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        Ok(parsed)
    }

    pub fn fail_parse(&self, document_id: &str, error: DocumentError) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        document_repository::mark_parse_failed(&transaction, document_id, &error)?;
        let failed = document_repository::get(&transaction, document_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        Ok(failed)
    }

    pub fn cancel_parse(&self, document_id: &str) -> DocumentResult<Document> {
        validate_uuid(document_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        document_repository::cancel_parse(&transaction, document_id)?;
        let staged = document_repository::get(&transaction, document_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        Ok(staged)
    }

    pub fn chunks(&self, document_id: &str) -> DocumentResult<Vec<DocumentChunk>> {
        validate_uuid(document_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        let document = document_repository::get(&connection, document_id)?;
        if document.status != DocumentStatus::Parsed {
            return Err(DocumentError::InvalidState);
        }
        let chunks = document_repository::list_chunks(&connection, document_id)?;
        if chunks.is_empty() {
            return Err(DocumentError::IntegrityFailed);
        }
        Ok(chunks)
    }

    pub fn chunk(&self, document_id: &str, chunk_id: &str) -> DocumentResult<DocumentChunk> {
        validate_uuid(document_id)?;
        validate_uuid(chunk_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        let document = document_repository::get(&connection, document_id)?;
        if document.status != DocumentStatus::Parsed {
            return Err(DocumentError::InvalidState);
        }
        document_repository::get_chunk(&connection, document_id, chunk_id)
    }

    pub fn delete(&self, document_id: &str) -> DocumentResult<()> {
        validate_uuid(document_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        self.delete_locked(document_id, false)
    }

    pub fn retry_delete(&self, document_id: &str) -> DocumentResult<()> {
        validate_uuid(document_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        if self.get(document_id)?.status != DocumentStatus::DeleteFailed {
            return Err(DocumentError::InvalidState);
        }
        self.delete_locked(document_id, false)
    }

    pub fn reconcile(&self) -> DocumentResult<ReconcileReport> {
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        self.reconcile_locked()
    }

    fn ensure_meeting_accepts_import(&self, meeting_id: &str) -> DocumentResult<()> {
        let connection = self.database.connect().map_err(map_data_error)?;
        let meeting =
            repository::get_including_deleted(&connection, meeting_id).map_err(map_data_error)?;
        if matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) {
            Ok(())
        } else {
            Err(DocumentError::InvalidState)
        }
    }

    fn insert_importing(
        &self,
        document_id: &str,
        meeting_id: &str,
        original_filename: &str,
        extension: &str,
        staging_relative: &str,
    ) -> DocumentResult<()> {
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::ImportFailed)?;
        let (count, bytes) = document_repository::usage(&transaction, meeting_id, None)?;
        if count >= self.policy.max_meeting_files || bytes >= self.policy.max_meeting_bytes {
            return Err(DocumentError::QuotaExceeded);
        }
        document_repository::insert_importing(
            &transaction,
            NewImport {
                id: document_id,
                meeting_id,
                original_filename,
                safe_extension: extension,
                storage_relative_path: staging_relative,
            },
        )?;
        transaction
            .commit()
            .map_err(|_| DocumentError::ImportFailed)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_import(
        &self,
        document_id: &str,
        meeting_id: &str,
        extension: &str,
        media_type: &str,
        size_bytes: u64,
        sha256: &str,
        staging_path: &Path,
    ) -> DocumentResult<Document> {
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::ImportFailed)?;
        let meeting = repository::get(&transaction, meeting_id).map_err(map_data_error)?;
        if !matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) {
            return Err(DocumentError::InvalidState);
        }
        let (count, bytes) =
            document_repository::usage(&transaction, meeting_id, Some(document_id))?;
        if count >= self.policy.max_meeting_files
            || bytes.saturating_add(size_bytes) > self.policy.max_meeting_bytes
        {
            return Err(DocumentError::QuotaExceeded);
        }
        if let Some(existing) =
            document_repository::find_by_hash(&transaction, meeting_id, sha256, document_id)?
        {
            document_repository::remove_importing(&transaction, document_id)?;
            transaction
                .commit()
                .map_err(|_| DocumentError::ImportFailed)?;
            return Err(DocumentError::AlreadyImported {
                document_id: existing.id,
            });
        }

        let final_relative = self
            .database
            .paths()
            .document_relative(meeting_id, document_id, extension)
            .map_err(map_data_error)?;
        let final_path = self
            .database
            .paths()
            .resolve_relative(&final_relative)
            .map_err(map_data_error)?;
        let parent = final_path
            .parent()
            .ok_or(DocumentError::StorageUnavailable)?;
        std::fs::create_dir_all(parent).map_err(|_| DocumentError::StorageUnavailable)?;
        std::fs::rename(staging_path, &final_path)
            .map_err(|_| DocumentError::StorageUnavailable)?;

        if let Err(error) = document_repository::mark_staged(
            &transaction,
            document_id,
            media_type,
            size_bytes,
            sha256,
            &final_relative,
        ) {
            let _ = std::fs::rename(&final_path, staging_path);
            return Err(error);
        }
        let document = document_repository::get(&transaction, document_id)?;
        transaction
            .commit()
            .map_err(|_| DocumentError::ImportFailed)?;
        Ok(document)
    }

    fn fail_import(&self, document_id: &str, error: &DocumentError) {
        let Ok(mut connection) = self.database.connect() else {
            return;
        };
        let Ok(transaction) = connection.transaction_with_behavior(TransactionBehavior::Immediate)
        else {
            return;
        };
        if document_repository::mark_import_failed(&transaction, document_id, error).is_ok() {
            let _ = transaction.commit();
        }
    }

    fn fail_import_and_cleanup(
        &self,
        document_id: &str,
        staging_path: &Path,
        error: &DocumentError,
    ) {
        self.fail_import(document_id, error);
        if staging_path.is_file() {
            let _ = std::fs::remove_file(staging_path);
        }
    }

    fn delete_locked(&self, document_id: &str, from_recovery: bool) -> DocumentResult<()> {
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let current = document_repository::get(&connection, document_id)?;
        let meeting = repository::get_including_deleted(&connection, &current.meeting_id)
            .map_err(map_data_error)?;
        if !matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) {
            return Err(DocumentError::InvalidState);
        }
        if current.status == DocumentStatus::Deleting {
            return Err(DocumentError::DeletePending);
        }
        if current.status == DocumentStatus::DeleteFailed && !from_recovery {
            // Explicit user retries are allowed; recovery retries are bounded below.
        } else if !matches!(
            current.status,
            DocumentStatus::Staged
                | DocumentStatus::Parsed
                | DocumentStatus::ParseFailed
                | DocumentStatus::ImportFailed
                | DocumentStatus::DeleteFailed
        ) {
            return Err(DocumentError::InvalidState);
        }

        let job_id = Uuid::new_v4().to_string();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        if summary_repository::document_has_active_summary(
            &transaction,
            &current.meeting_id,
            &current.id,
        )
        .map_err(|_| DocumentError::Internal)?
        {
            return Err(DocumentError::InvalidState);
        }
        summary_repository::redact_terminal_document(
            &transaction,
            &current.meeting_id,
            &current.id,
        )
        .map_err(|_| DocumentError::Internal)?;
        document_repository::begin_delete(&transaction, &current, &job_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        self.execute_document_delete(document_id)
    }

    fn execute_document_delete(&self, document_id: &str) -> DocumentResult<()> {
        let connection = self.database.connect().map_err(map_data_error)?;
        let document = document_repository::get(&connection, document_id)?;
        let job = document_repository::deletion_job_for_document(&connection, document_id)?
            .ok_or(DocumentError::Internal)?;
        drop(connection);

        let source = self
            .database
            .paths()
            .resolve_relative(&document.storage_relative_path)
            .map_err(map_data_error)?;
        let trash_relative = self
            .database
            .paths()
            .trash_relative(&job.id)
            .map_err(map_data_error)?;
        let trash = self
            .database
            .paths()
            .resolve_relative(&trash_relative)
            .map_err(map_data_error)?;

        let operation = (|| -> DocumentResult<()> {
            if source.exists() {
                if trash.exists() {
                    remove_path(&trash)?;
                }
                std::fs::rename(&source, &trash).map_err(|_| DocumentError::DeleteFailed)?;
            }
            #[cfg(test)]
            if self
                .fail_delete_after_move
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                return Err(DocumentError::DeleteFailed);
            }
            if trash.exists() {
                remove_path(&trash)?;
            }
            Ok(())
        })();

        let finalized = if operation.is_ok() {
            (|| -> DocumentResult<()> {
                let mut connection = self.database.connect().map_err(map_data_error)?;
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| DocumentError::Internal)?;
                document_repository::update_trash_path(&transaction, document_id, &trash_relative)?;
                document_repository::finalize_delete(&transaction, document_id)?;
                transaction.commit().map_err(|_| DocumentError::Internal)
            })()
        } else {
            Err(DocumentError::DeleteFailed)
        };
        if finalized.is_ok() {
            return Ok(());
        }

        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        document_repository::mark_delete_failed(&transaction, document_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)?;
        Err(DocumentError::DeleteFailed)
    }

    fn reconcile_locked(&self) -> DocumentResult<ReconcileReport> {
        let mut report = ReconcileReport::default();
        self.reconcile_meeting_deletions(&mut report)?;

        let documents =
            document_repository::all(&self.database.connect().map_err(map_data_error)?)?;
        for document in documents {
            match document.status {
                DocumentStatus::Importing => self.reconcile_import(&document, &mut report)?,
                DocumentStatus::Staged => {
                    let path = self
                        .database
                        .paths()
                        .resolve_relative(&document.storage_relative_path)
                        .map_err(map_data_error)?;
                    if !path.is_file() || verify_committed_document(&document, &path).is_err() {
                        let mut connection = self.database.connect().map_err(map_data_error)?;
                        let transaction = connection
                            .transaction_with_behavior(TransactionBehavior::Immediate)
                            .map_err(|_| DocumentError::Internal)?;
                        document_repository::mark_integrity_failed(&transaction, &document.id)?;
                        transaction.commit().map_err(|_| DocumentError::Internal)?;
                        report.missing_committed_files += 1;
                    }
                }
                DocumentStatus::Deleting | DocumentStatus::DeleteFailed => {
                    let mut job = document_repository::deletion_job_for_document(
                        &self.database.connect().map_err(map_data_error)?,
                        &document.id,
                    )?;
                    if job.is_none() && document.status == DocumentStatus::Deleting {
                        let mut connection = self.database.connect().map_err(map_data_error)?;
                        let transaction = connection
                            .transaction_with_behavior(TransactionBehavior::Immediate)
                            .map_err(|_| DocumentError::Internal)?;
                        document_repository::ensure_delete_job(
                            &transaction,
                            &document,
                            &Uuid::new_v4().to_string(),
                        )?;
                        transaction.commit().map_err(|_| DocumentError::Internal)?;
                        job = document_repository::deletion_job_for_document(
                            &self.database.connect().map_err(map_data_error)?,
                            &document.id,
                        )?;
                    }
                    if job
                        .as_ref()
                        .is_none_or(|value| value.attempt_count < MAX_DELETE_ATTEMPTS)
                    {
                        let result = if document.status == DocumentStatus::Deleting {
                            self.execute_document_delete(&document.id)
                        } else {
                            self.delete_locked(&document.id, true)
                        };
                        if result.is_ok() {
                            report.deletions_completed += 1;
                        } else {
                            report.deletions_failed += 1;
                        }
                    }
                }
                DocumentStatus::Parsing => {
                    let mut connection = self.database.connect().map_err(map_data_error)?;
                    let transaction = connection
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(|_| DocumentError::Internal)?;
                    document_repository::mark_parse_interrupted(&transaction, &document.id)?;
                    transaction.commit().map_err(|_| DocumentError::Internal)?;
                    report.parses_failed_closed += 1;
                }
                DocumentStatus::Parsed => {
                    let path = self
                        .database
                        .paths()
                        .resolve_relative(&document.storage_relative_path)
                        .map_err(map_data_error)?;
                    let connection = self.database.connect().map_err(map_data_error)?;
                    let valid = verify_committed_document(&document, &path).is_ok()
                        && document_repository::has_chunks(&connection, &document.id)?;
                    drop(connection);
                    if !valid {
                        let mut connection = self.database.connect().map_err(map_data_error)?;
                        let transaction = connection
                            .transaction_with_behavior(TransactionBehavior::Immediate)
                            .map_err(|_| DocumentError::Internal)?;
                        document_repository::mark_parsed_integrity_failed(
                            &transaction,
                            &document.id,
                        )?;
                        transaction.commit().map_err(|_| DocumentError::Internal)?;
                        report.parsed_integrity_failures += 1;
                    }
                }
                DocumentStatus::ImportFailed | DocumentStatus::ParseFailed => {}
            }
        }
        self.exhaust_inconsistent_document_jobs()?;
        self.quarantine_unknown_files(&mut report)?;
        Ok(report)
    }

    fn reconcile_import(
        &self,
        document: &Document,
        report: &mut ReconcileReport,
    ) -> DocumentResult<()> {
        let staging_path = self
            .database
            .paths()
            .resolve_relative(&document.storage_relative_path)
            .map_err(map_data_error)?;
        let final_relative = self
            .database
            .paths()
            .document_relative(&document.meeting_id, &document.id, &document.safe_extension)
            .map_err(map_data_error)?;
        let final_path = self
            .database
            .paths()
            .resolve_relative(&final_relative)
            .map_err(map_data_error)?;

        if final_path.is_file() {
            let recovered = (|| -> DocumentResult<(&'static str, u64, String)> {
                let metadata = final_path
                    .metadata()
                    .map_err(|_| DocumentError::IntegrityFailed)?;
                let (size, hash) = hash_file(&final_path, self.policy.max_file_bytes)?;
                if size != metadata.len() {
                    return Err(DocumentError::IntegrityFailed);
                }
                let media_type = detect_media_type(&final_path, &document.safe_extension)?;
                Ok((media_type, size, hash))
            })();
            match recovered.and_then(|(media_type, size, hash)| {
                self.finish_recovered_import(document, media_type, size, &hash, &final_relative)
            }) {
                Ok(()) => report.imports_completed += 1,
                Err(error) => {
                    self.fail_import(&document.id, &error);
                    report.imports_failed_closed += 1;
                }
            }
        } else {
            self.fail_import(&document.id, &DocumentError::ImportFailed);
            report.imports_failed_closed += 1;
        }
        let _ = staging_path;
        Ok(())
    }

    fn finish_recovered_import(
        &self,
        document: &Document,
        media_type: &str,
        size: u64,
        hash: &str,
        final_relative: &str,
    ) -> DocumentResult<()> {
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        let meeting =
            repository::get(&transaction, &document.meeting_id).map_err(map_data_error)?;
        if !matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        ) {
            return Err(DocumentError::InvalidState);
        }
        let (count, bytes) =
            document_repository::usage(&transaction, &document.meeting_id, Some(&document.id))?;
        if count >= self.policy.max_meeting_files
            || bytes.saturating_add(size) > self.policy.max_meeting_bytes
        {
            return Err(DocumentError::QuotaExceeded);
        }
        if document_repository::find_by_hash(
            &transaction,
            &document.meeting_id,
            hash,
            &document.id,
        )?
        .is_some()
        {
            return Err(DocumentError::AlreadyImported {
                document_id: document.id.clone(),
            });
        }
        document_repository::mark_staged(
            &transaction,
            &document.id,
            media_type,
            size,
            hash,
            final_relative,
        )?;
        transaction.commit().map_err(|_| DocumentError::Internal)
    }

    fn reconcile_meeting_deletions(&self, report: &mut ReconcileReport) -> DocumentResult<()> {
        let jobs =
            document_repository::deletion_jobs(&self.database.connect().map_err(map_data_error)?)?;
        for job in jobs
            .into_iter()
            .filter(|value| value.entity_kind == "meeting")
        {
            if job.attempt_count >= MAX_DELETE_ATTEMPTS {
                continue;
            }
            match execute_meeting_delete(&self.database, &job.meeting_id) {
                Ok(()) => report.deletions_completed += 1,
                Err(DataError::InvalidState | DataError::NotFound) => {
                    self.exhaust_job(&job.id)?;
                    report.deletions_failed += 1;
                }
                Err(_) => report.deletions_failed += 1,
            }
        }
        Ok(())
    }

    fn exhaust_inconsistent_document_jobs(&self) -> DocumentResult<()> {
        let connection = self.database.connect().map_err(map_data_error)?;
        let jobs = document_repository::deletion_jobs(&connection)?;
        for job in jobs
            .into_iter()
            .filter(|value| value.entity_kind == "document")
        {
            let Some(document_id) = job.document_id.as_deref() else {
                self.exhaust_job(&job.id)?;
                continue;
            };
            let document = document_repository::get(&connection, document_id)?;
            if !matches!(
                document.status,
                DocumentStatus::Deleting | DocumentStatus::DeleteFailed
            ) {
                self.exhaust_job(&job.id)?;
            }
        }
        Ok(())
    }

    fn exhaust_job(&self, job_id: &str) -> DocumentResult<()> {
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| DocumentError::Internal)?;
        document_repository::exhaust_inconsistent_job(&transaction, job_id)?;
        transaction.commit().map_err(|_| DocumentError::Internal)
    }

    fn quarantine_unknown_files(&self, report: &mut ReconcileReport) -> DocumentResult<()> {
        let connection = self.database.connect().map_err(map_data_error)?;
        let documents = document_repository::all(&connection)?;
        let known_paths = documents
            .iter()
            .map(|document| {
                self.database
                    .paths()
                    .resolve_relative(&document.storage_relative_path)
                    .map(|path| normalize_path(&path))
                    .map_err(map_data_error)
            })
            .collect::<DocumentResult<HashSet<_>>>()?;
        let meeting_ids = repository::all_ids_including_deleted(&connection)
            .map_err(map_data_error)?
            .into_iter()
            .collect::<HashSet<_>>();
        drop(connection);

        for entry in read_directory(self.database.paths().staging())? {
            if !known_paths.contains(&normalize_path(&entry))
                && quarantine_path(&self.database, &entry).is_ok()
            {
                report.quarantined_files += 1;
            }
        }
        for entry in read_directory(self.database.paths().meetings())? {
            let entry_metadata =
                std::fs::symlink_metadata(&entry).map_err(|_| DocumentError::StorageUnavailable)?;
            if !entry_metadata.is_dir() || is_redirected(&entry_metadata) {
                if quarantine_path(&self.database, &entry).is_ok() {
                    report.quarantined_files += 1;
                }
                continue;
            }
            let name = entry
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if !meeting_ids.contains(name) {
                if quarantine_path(&self.database, &entry).is_ok() {
                    report.quarantined_files += 1;
                }
                continue;
            }
            for file in collect_files(&entry)? {
                if !known_paths.contains(&normalize_path(&file))
                    && quarantine_path(&self.database, &file).is_ok()
                {
                    report.quarantined_files += 1;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn execute_meeting_delete(
    database: &Database,
    meeting_id: &str,
) -> Result<(), DataError> {
    let connection = database.connect()?;
    let meeting = repository::get_including_deleted(&connection, meeting_id)?;
    if !matches!(
        meeting.status,
        MeetingStatus::Deleting | MeetingStatus::DeleteFailed
    ) {
        return Err(DataError::InvalidState);
    }
    let documents =
        document_repository::list(&connection, meeting_id).map_err(|_| DataError::Database)?;
    let jobs = document_repository::deletion_jobs(&connection).map_err(|_| DataError::Database)?;
    let job = jobs
        .iter()
        .find(|job| job.entity_kind == "meeting" && job.meeting_id == meeting_id)
        .cloned()
        .ok_or(DataError::Database)?;
    drop(connection);

    if meeting.status == MeetingStatus::DeleteFailed {
        let mut connection = database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        repository::begin_delete(&transaction, meeting_id, &job.id)?;
        transaction.commit().map_err(DataError::from)?;
    }

    let trash_relative = database.paths().trash_relative(&job.id)?;
    let trash = database.paths().resolve_relative(&trash_relative)?;
    let operation = (|| -> Result<(), DataError> {
        std::fs::create_dir_all(&trash).map_err(|_| DataError::StorageUnavailable)?;
        let meeting_relative = database.paths().meeting_relative(meeting_id)?;
        let meeting_path = database.paths().resolve_relative(&meeting_relative)?;
        let trashed_meeting = trash.join("meeting");
        if meeting_path.exists() {
            if trashed_meeting.exists() {
                remove_path(&trashed_meeting).map_err(|_| DataError::StorageUnavailable)?;
            }
            std::fs::rename(&meeting_path, &trashed_meeting)
                .map_err(|_| DataError::StorageUnavailable)?;
        }
        let trashed_staging = trash.join("staging");
        let trashed_document_jobs = trash.join("document-deletions");
        for document in &documents {
            let source = database
                .paths()
                .resolve_relative(&document.storage_relative_path)?;
            if source.exists() && source.starts_with(database.paths().staging()) {
                std::fs::create_dir_all(&trashed_staging)
                    .map_err(|_| DataError::StorageUnavailable)?;
                let destination = trashed_staging.join(&document.id);
                std::fs::rename(source, destination).map_err(|_| DataError::StorageUnavailable)?;
            }
            if let Some(document_job) = jobs.iter().find(|job| {
                job.entity_kind == "document"
                    && job.document_id.as_deref() == Some(document.id.as_str())
            }) {
                let pending_trash_relative = database.paths().trash_relative(&document_job.id)?;
                let pending_trash = database.paths().resolve_relative(pending_trash_relative)?;
                if pending_trash.exists() {
                    std::fs::create_dir_all(&trashed_document_jobs)
                        .map_err(|_| DataError::StorageUnavailable)?;
                    std::fs::rename(pending_trash, trashed_document_jobs.join(&document.id))
                        .map_err(|_| DataError::StorageUnavailable)?;
                }
            }
        }
        remove_path(&trash).map_err(|_| DataError::StorageUnavailable)
    })();

    let finalized = if operation.is_ok() {
        (|| -> DataResult<()> {
            let mut connection = database.connect()?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(DataError::from)?;
            repository::update_meeting_trash_path(&transaction, meeting_id, &trash_relative)?;
            repository::finalize_delete(&transaction, meeting_id)?;
            transaction.commit().map_err(DataError::from)
        })()
    } else {
        Err(DataError::MeetingDeleteFailed)
    };
    if finalized.is_ok() {
        return Ok(());
    }

    let mut connection = database.connect()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(DataError::from)?;
    repository::mark_delete_failed(&transaction, meeting_id, "meeting_delete_failed")?;
    transaction.commit().map_err(DataError::from)?;
    Err(DataError::MeetingDeleteFailed)
}

fn validate_parser_output(
    parser_version: &str,
    page_count: Option<u64>,
    chunks: &[DocumentChunkInput],
) -> DocumentResult<()> {
    if parser_version.trim().is_empty()
        || parser_version.as_bytes().len() > 64
        || !parser_version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
        || chunks.is_empty()
        || chunks.len() > DOCUMENT_CHUNK_MAX_COUNT
        || page_count.is_some_and(|count| !(1..=300).contains(&count))
    {
        return Err(DocumentError::IntegrityFailed);
    }

    let mut total_bytes = 0usize;
    for chunk in chunks {
        let text_bytes = chunk.text.as_bytes();
        total_bytes = total_bytes
            .checked_add(text_bytes.len())
            .ok_or(DocumentError::IntegrityFailed)?;
        if chunk.text.trim().is_empty()
            || text_bytes.len() > DOCUMENT_CHUNK_MAX_BYTES
            || total_bytes > DOCUMENT_CHUNKS_MAX_TOTAL_BYTES
            || chunk.chunk_schema_version != DOCUMENT_CHUNK_SCHEMA_VERSION
            || sha256_bytes(text_bytes) != chunk.content_sha256
        {
            return Err(DocumentError::IntegrityFailed);
        }

        match (chunk.page_start, chunk.page_end, page_count) {
            (None, None, _) => {}
            (Some(start), Some(end), Some(total))
                if start >= 1 && start <= end && end <= total && total <= 300 => {}
            _ => return Err(DocumentError::IntegrityFailed),
        }

        let section_path = serde_json::from_str::<Vec<String>>(&chunk.section_path_json)
            .map_err(|_| DocumentError::IntegrityFailed)?;
        if chunk.section_path_json.as_bytes().len() > 4096
            || section_path.len() > 16
            || section_path.iter().any(|part| {
                part.trim().is_empty() || part.as_bytes().len() > 255 || part.contains('\0')
            })
            || serde_json::to_string(&section_path).map_err(|_| DocumentError::IntegrityFailed)?
                != chunk.section_path_json
        {
            return Err(DocumentError::IntegrityFailed);
        }

        match (chunk.kind, chunk.table_ordinal) {
            (DocumentChunkKind::Table, Some(ordinal))
                if ordinal
                    < u64::try_from(DOCUMENT_CHUNK_MAX_COUNT)
                        .map_err(|_| DocumentError::Internal)? => {}
            (DocumentChunkKind::Heading, None)
            | (DocumentChunkKind::ListItem, None)
            | (DocumentChunkKind::Paragraph, None) => {}
            _ => return Err(DocumentError::IntegrityFailed),
        }
    }
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_source_file(path: &Path) -> DocumentResult<std::fs::Metadata> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| DocumentError::StorageUnavailable)?;
    if !metadata.file_type().is_file() || is_redirected(&metadata) {
        return Err(DocumentError::InvalidType);
    }
    Ok(metadata)
}

fn open_source_file(path: &Path) -> DocumentResult<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        options.share_mode(FILE_SHARE_READ);
    }
    options
        .open(path)
        .map_err(|_| DocumentError::StorageUnavailable)
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

fn same_source_snapshot(first: &std::fs::Metadata, second: &std::fs::Metadata) -> bool {
    first.len() == second.len()
        && first.modified().ok() == second.modified().ok()
        && first.created().ok() == second.created().ok()
}

fn copy_and_hash(
    source: &Path,
    destination: &Path,
    before: std::fs::Metadata,
    max_bytes: u64,
    mut on_progress: impl FnMut(u64),
) -> DocumentResult<(u64, String)> {
    let source_file = open_source_file(source)?;
    let opened = source_file
        .metadata()
        .map_err(|_| DocumentError::StorageUnavailable)?;
    if !same_source_snapshot(&before, &opened) {
        return Err(DocumentError::IntegrityFailed);
    }
    let destination_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| DocumentError::StorageUnavailable)?;
    let mut reader = BufReader::with_capacity(COPY_BUFFER_BYTES, source_file);
    let mut writer = BufWriter::with_capacity(COPY_BUFFER_BYTES, destination_file);
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut last_reported = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DocumentError::ImportFailed)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > max_bytes {
            return Err(DocumentError::TooLarge);
        }
        digest.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .map_err(|_| DocumentError::StorageUnavailable)?;
        let bounded_total = total.min(before.len());
        if bounded_total > last_reported
            && (bounded_total.saturating_sub(last_reported) >= COPY_PROGRESS_INTERVAL_BYTES
                || bounded_total == before.len())
        {
            on_progress(bounded_total);
            last_reported = bounded_total;
        }
    }
    writer
        .flush()
        .map_err(|_| DocumentError::StorageUnavailable)?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| DocumentError::StorageUnavailable)?;

    let handle_after = reader
        .get_ref()
        .metadata()
        .map_err(|_| DocumentError::IntegrityFailed)?;
    let after = validate_source_file(source)?;
    let changed = !same_source_snapshot(&before, &handle_after)
        || !same_source_snapshot(&handle_after, &after)
        || total != after.len();
    if changed {
        return Err(DocumentError::IntegrityFailed);
    }
    Ok((total, encode_hex(&digest.finalize())))
}

#[allow(clippy::too_many_arguments)]
fn import_progress(
    file_index: usize,
    total_files: usize,
    original_filename: &str,
    phase: DocumentImportPhase,
    bytes_processed: u64,
    total_bytes: Option<u64>,
    document_id: Option<String>,
    error_code: Option<&'static str>,
) -> DocumentImportProgress {
    DocumentImportProgress {
        file_index,
        total_files,
        original_filename: original_filename.to_string(),
        phase,
        bytes_processed,
        total_bytes,
        document_id,
        error_code,
    }
}

fn progress_filename(path: &Path, max_bytes: usize) -> String {
    let Some(value) = path.file_name().and_then(|value| value.to_str()) else {
        return "Selected file".to_string();
    };
    let mut safe = String::new();
    for character in value.chars() {
        let character = if character.is_control() {
            '\u{fffd}'
        } else {
            character
        };
        if safe.len() + character.len_utf8() > max_bytes {
            break;
        }
        safe.push(character);
    }
    if safe.is_empty() {
        "Selected file".to_string()
    } else {
        safe
    }
}

fn hash_file(path: &Path, max_bytes: u64) -> DocumentResult<(u64, String)> {
    let metadata = validate_source_file(path)?;
    if metadata.len() == 0 {
        return Err(DocumentError::Empty);
    }
    if metadata.len() > max_bytes {
        return Err(DocumentError::TooLarge);
    }
    let mut reader = BufReader::new(open_source_file(path)?);
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| DocumentError::IntegrityFailed)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        digest.update(&buffer[..read]);
    }
    if total != metadata.len() {
        return Err(DocumentError::IntegrityFailed);
    }
    Ok((total, encode_hex(&digest.finalize())))
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

fn verify_committed_document(document: &Document, path: &Path) -> DocumentResult<()> {
    let expected_size = document.size_bytes.ok_or(DocumentError::IntegrityFailed)?;
    let expected_hash = document
        .sha256
        .as_deref()
        .ok_or(DocumentError::IntegrityFailed)?;
    let (actual_size, actual_hash) = hash_file(path, expected_size.max(1))?;
    if actual_size != expected_size || actual_hash != expected_hash {
        return Err(DocumentError::IntegrityFailed);
    }
    detect_media_type(path, &document.safe_extension)?;
    Ok(())
}

fn remove_path(path: &Path) -> DocumentResult<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|_| DocumentError::DeleteFailed)
    } else {
        std::fs::remove_file(path).map_err(|_| DocumentError::DeleteFailed)
    }
}

fn read_directory(path: &Path) -> DocumentResult<Vec<PathBuf>> {
    std::fs::read_dir(path)
        .map_err(|_| DocumentError::StorageUnavailable)?
        .map(|entry| {
            entry
                .map(|value| value.path())
                .map_err(|_| DocumentError::StorageUnavailable)
        })
        .collect()
}

fn collect_files(root: &Path) -> DocumentResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for path in read_directory(&directory)? {
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|_| DocumentError::StorageUnavailable)?;
            if metadata.is_dir() && !is_redirected(&metadata) {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn quarantine_path(database: &Database, source: &Path) -> DocumentResult<()> {
    let quarantine_relative = database
        .paths()
        .trash_relative(&Uuid::new_v4().to_string())
        .map_err(map_data_error)?;
    let quarantine = database
        .paths()
        .resolve_relative(quarantine_relative)
        .map_err(map_data_error)?;
    std::fs::rename(source, quarantine).map_err(|_| DocumentError::StorageUnavailable)
}

fn normalize_path(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_ascii_lowercase())
    } else {
        path.to_path_buf()
    }
}

fn validate_uuid(value: &str) -> DocumentResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| DocumentError::NotFound)?;
    if parsed.to_string() != value {
        return Err(DocumentError::NotFound);
    }
    Ok(())
}

fn map_data_error(error: DataError) -> DocumentError {
    match error {
        DataError::InvalidState
        | DataError::MeetingDeleteFailed
        | DataError::MeetingDeletePending => DocumentError::InvalidState,
        DataError::NotFound | DataError::InvalidId => DocumentError::NotFound,
        DataError::StorageUnavailable | DataError::UnsafePath => DocumentError::StorageUnavailable,
        DataError::Database | DataError::InvalidInput | DataError::MigrationConflict => {
            DocumentError::Internal
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::sync::{Arc, Barrier};

    use rusqlite::TransactionBehavior;

    use super::*;
    use crate::data::repository::NewMeeting;
    use crate::data::{CreateMeeting, MeetingService};

    struct TestWorkspace {
        root: PathBuf,
        app_data: PathBuf,
        sources: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("meetinglens-document-storage-{}", Uuid::new_v4()));
            let app_data = root.join("app-data");
            let sources = root.join("sources");
            std::fs::create_dir_all(&sources).expect("synthetic source directory should exist");
            Self {
                root,
                app_data,
                sources,
            }
        }

        fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.sources.join(name);
            std::fs::write(&path, bytes).expect("synthetic source should write");
            path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn create_meeting(service: &MeetingService, title: &str) -> String {
        service
            .create(CreateMeeting {
                title: title.to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("synthetic meeting should create")
            .id
    }

    fn insert_meeting(database: &Database) -> String {
        let id = Uuid::new_v4().to_string();
        let mut connection = database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        repository::insert(
            &transaction,
            NewMeeting {
                id: &id,
                title: "Synthetic document meeting",
                primary_language: "en-AU",
            },
        )
        .expect("meeting should insert");
        transaction.commit().expect("meeting should commit");
        id
    }

    fn ooxml_source(workspace: &TestWorkspace, name: &str, package_path: &str) -> PathBuf {
        let path = workspace.sources.join(name);
        let file = File::create(&path).expect("synthetic OOXML file should create");
        let mut writer = zip::ZipWriter::new(file);
        for (entry, bytes) in [
            ("[Content_Types].xml", b"synthetic types".as_slice()),
            (package_path, b"synthetic package".as_slice()),
        ] {
            writer
                .start_file(entry, zip::write::SimpleFileOptions::default())
                .expect("synthetic OOXML entry should start");
            writer
                .write_all(bytes)
                .expect("synthetic OOXML entry should write");
        }
        writer.finish().expect("synthetic OOXML should finish");
        path
    }

    #[test]
    fn imports_streams_hashes_and_commits_to_uuid_only_private_storage() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic import");
        let source = workspace.source("private-notes.txt", b"synthetic notes\n");

        let document = meetings
            .documents()
            .import_document(&meeting_id, &source)
            .expect("document should import");

        assert_eq!(document.status, DocumentStatus::Staged);
        assert_eq!(document.original_filename, "private-notes.txt");
        assert_eq!(document.safe_extension, "txt");
        assert_eq!(document.detected_media_type.as_deref(), Some("text/plain"));
        assert_eq!(document.size_bytes, Some(16));
        assert_eq!(document.sha256.as_deref().map(str::len), Some(64));
        assert!(!document.storage_relative_path.contains("private-notes"));
        assert!(document.storage_relative_path.contains(&meeting_id));
        assert!(document.storage_relative_path.contains(&document.id));
        let committed = meetings
            .storage_paths()
            .resolve_relative(&document.storage_relative_path)
            .expect("committed path should resolve");
        assert_eq!(
            std::fs::read(committed).expect("file should read"),
            b"synthetic notes\n"
        );

        let stored_path: String = rusqlite::Connection::open(meetings.storage_paths().database())
            .expect("database should connect")
            .query_row(
                "SELECT storage_relative_path FROM documents WHERE id = ?1",
                [&document.id],
                |row| row.get(0),
            )
            .expect("stored path should read");
        assert!(!stored_path.contains(&workspace.sources.to_string_lossy().to_string()));
        assert!(!stored_path.contains(':'));
    }

    #[test]
    fn deduplicates_within_a_meeting_but_not_across_meetings() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let first_meeting = create_meeting(&meetings, "Synthetic first meeting");
        let second_meeting = create_meeting(&meetings, "Synthetic second meeting");
        let first_source = workspace.source("first.txt", b"same synthetic content");
        let second_source = workspace.source("second.txt", b"same synthetic content");
        let documents = meetings.documents();

        let first = documents
            .import_document(&first_meeting, &first_source)
            .expect("first import should work");
        assert_eq!(
            documents.import_document(&first_meeting, &second_source),
            Err(DocumentError::AlreadyImported {
                document_id: first.id.clone()
            })
        );
        assert_eq!(
            documents
                .list(&first_meeting)
                .expect("documents should list")
                .len(),
            1
        );
        let cross_meeting = documents
            .import_document(&second_meeting, &second_source)
            .expect("same content in another meeting should import");
        assert_ne!(cross_meeting.id, first.id);
    }

    #[test]
    fn imports_every_allowlisted_format_with_bounded_type_detection() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic format matrix");
        let mut sources = vec![
            workspace.source("synthetic.pdf", b"%PDF-1.7\nsynthetic"),
            workspace.source("synthetic.png", b"\x89PNG\r\n\x1a\nsynthetic"),
            workspace.source("synthetic.jpg", b"\xff\xd8\xff\xe0synthetic"),
            workspace.source("synthetic.txt", b"synthetic text\n"),
            workspace.source("synthetic.md", b"# synthetic markdown\n"),
            workspace.source("synthetic.csv", b"name,value\nsynthetic,1\n"),
        ];
        sources.extend([
            ooxml_source(&workspace, "synthetic.docx", "word/document.xml"),
            ooxml_source(&workspace, "synthetic.pptx", "ppt/presentation.xml"),
            ooxml_source(&workspace, "synthetic.xlsx", "xl/workbook.xml"),
        ]);

        let imported = meetings
            .documents()
            .import_batch(&meeting_id, &sources)
            .expect("all allowlisted formats should import");
        assert_eq!(imported.len(), 9);
        assert!(imported.iter().all(|document| {
            document.status == DocumentStatus::Staged
                && document.detected_media_type.is_some()
                && document
                    .sha256
                    .as_deref()
                    .is_some_and(|hash| hash.len() == 64)
        }));
    }

    #[test]
    fn same_filename_with_different_content_is_allowed() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic same name");
        let source = workspace.source("same-name.txt", b"first content");
        let first = meetings
            .documents()
            .import_document(&meeting_id, &source)
            .expect("first content should import");
        std::fs::write(&source, b"second content").expect("source should change between imports");
        let second = meetings
            .documents()
            .import_document(&meeting_id, &source)
            .expect("different content with the same name should import");
        assert_ne!(first.sha256, second.sha256);
        assert_eq!(first.original_filename, second.original_filename);
    }

    #[test]
    fn enforces_file_batch_count_byte_and_filename_quotas() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let policy = DocumentImportPolicy {
            max_file_bytes: 8,
            max_batch_files: 1,
            max_meeting_files: 1,
            max_meeting_bytes: 8,
            max_filename_bytes: 12,
        };
        let documents = DocumentService::with_policy(database, policy);
        let first = workspace.source("first.txt", b"1234");
        let second = workspace.source("second.txt", b"5678");
        let large = workspace.source("large.txt", b"123456789");
        let long_name = workspace.source("filename-long.txt", b"x");

        assert_eq!(
            documents.import_batch(&meeting_id, &[first.clone(), second.clone()]),
            Err(DocumentError::QuotaExceeded)
        );
        assert_eq!(
            documents.import_document(&meeting_id, &large),
            Err(DocumentError::TooLarge)
        );
        assert_eq!(
            documents.import_document(&meeting_id, &long_name),
            Err(DocumentError::FilenameTooLong)
        );
        documents
            .import_document(&meeting_id, &first)
            .expect("first allowed file should import");
        assert_eq!(
            documents.import_document(&meeting_id, &second),
            Err(DocumentError::QuotaExceeded)
        );
    }

    #[test]
    fn rejects_empty_mismatched_invalid_text_and_disguised_sources() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic validation");
        let documents = meetings.documents();
        let empty = workspace.source("empty.txt", b"");
        let fake_pdf = workspace.source("fake.pdf", b"not-pdf");
        let invalid_text = workspace.source("invalid.txt", b"hello\0world");
        let disguised = workspace.source("payload.exe.pdf", b"%PDF-1.7\n");
        let unsupported = workspace.source("archive.zip", b"not allowed");
        let directory = workspace.sources.join("folder.txt");
        std::fs::create_dir(&directory).expect("synthetic directory should create");

        assert_eq!(
            documents.import_document(&meeting_id, empty),
            Err(DocumentError::Empty)
        );
        assert_eq!(
            documents.import_document(&meeting_id, fake_pdf),
            Err(DocumentError::TypeMismatch)
        );
        assert_eq!(
            documents.import_document(&meeting_id, invalid_text),
            Err(DocumentError::TypeMismatch)
        );
        assert_eq!(
            documents.import_document(&meeting_id, disguised),
            Err(DocumentError::InvalidType)
        );
        assert_eq!(
            documents.import_document(&meeting_id, unsupported),
            Err(DocumentError::InvalidType)
        );
        assert_eq!(
            documents.import_document(&meeting_id, directory),
            Err(DocumentError::InvalidType)
        );
        let failures = documents
            .list(&meeting_id)
            .expect("failed imports should be inspectable");
        assert_eq!(failures.len(), 2);
        assert_ne!(
            failures[0].storage_relative_path,
            failures[1].storage_relative_path
        );
        for failure in failures {
            assert_eq!(failure.status, DocumentStatus::ImportFailed);
            let temporary_name = Path::new(&failure.storage_relative_path)
                .file_stem()
                .and_then(|value| value.to_str())
                .expect("staging filename should be UTF-8");
            assert!(Uuid::parse_str(temporary_name).is_ok());
            let temporary = meetings
                .storage_paths()
                .resolve_relative(&failure.storage_relative_path)
                .expect("temporary path should resolve");
            assert!(!temporary.exists());
        }
    }

    #[test]
    fn serializes_concurrent_imports_and_fails_closed_at_the_quota() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let documents = DocumentService::with_policy(
            database,
            DocumentImportPolicy {
                max_meeting_files: 1,
                ..DocumentImportPolicy::default()
            },
        );
        let first = workspace.source("concurrent-one.txt", b"one");
        let second = workspace.source("concurrent-two.txt", b"two");
        let barrier = Arc::new(Barrier::new(3));
        let handles = [first, second].map(|source| {
            let documents = documents.clone();
            let meeting_id = meeting_id.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                documents.import_document(&meeting_id, source)
            })
        });
        barrier.wait();
        let results = handles.map(|handle| handle.join().expect("import thread should finish"));

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(DocumentError::QuotaExceeded)))
                .count(),
            1
        );
    }

    #[test]
    fn concurrent_deduplication_and_capacity_checks_remain_race_safe() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let duplicate_meeting = insert_meeting(&database);
        let duplicate_service = DocumentService::new(database.clone());
        let duplicate_sources = [
            workspace.source("duplicate-one.txt", b"identical"),
            workspace.source("duplicate-two.txt", b"identical"),
        ];
        let barrier = Arc::new(Barrier::new(3));
        let handles = duplicate_sources.map(|source| {
            let service = duplicate_service.clone();
            let meeting_id = duplicate_meeting.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                service.import_document(&meeting_id, source)
            })
        });
        barrier.wait();
        let results = handles.map(|handle| handle.join().expect("import thread should finish"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(DocumentError::AlreadyImported { .. })))
                .count(),
            1
        );
        assert_eq!(
            duplicate_service
                .list(&duplicate_meeting)
                .expect("documents should list")
                .len(),
            1
        );
        assert_eq!(
            collect_files(database.paths().meetings())
                .expect("committed files should enumerate")
                .len(),
            1
        );

        let capacity_meeting = insert_meeting(&database);
        let capacity_service = DocumentService::with_policy(
            database,
            DocumentImportPolicy {
                max_meeting_bytes: 5,
                ..DocumentImportPolicy::default()
            },
        );
        let capacity_sources = [
            workspace.source("capacity-one.txt", b"one"),
            workspace.source("capacity-two.txt", b"two"),
        ];
        let barrier = Arc::new(Barrier::new(3));
        let handles = capacity_sources.map(|source| {
            let service = capacity_service.clone();
            let meeting_id = capacity_meeting.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                service.import_document(&meeting_id, source)
            })
        });
        barrier.wait();
        let results = handles.map(|handle| handle.join().expect("import thread should finish"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(DocumentError::QuotaExceeded)))
                .count(),
            1
        );
    }

    #[test]
    fn streaming_copy_enforces_live_limits_hashes_and_source_stability() {
        let workspace = TestWorkspace::new();
        let source = workspace.source("stream.txt", b"abc");
        let before = std::fs::metadata(&source).expect("source metadata should read");
        let destination = workspace.root.join("first-copy.tmp");
        let (size, hash) = copy_and_hash(&source, &destination, before, 3, |_| {})
            .expect("bounded copy should succeed");
        assert_eq!(size, 3);
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let before_change = std::fs::metadata(&source).expect("source metadata should read");
        std::fs::write(&source, b"source changed").expect("source should change");
        assert_eq!(
            copy_and_hash(
                &source,
                &workspace.root.join("changed-copy.tmp"),
                before_change,
                100,
                |_| {},
            ),
            Err(DocumentError::IntegrityFailed)
        );

        let before_limit = std::fs::metadata(&source).expect("source metadata should read");
        assert_eq!(
            copy_and_hash(
                &source,
                &workspace.root.join("limited-copy.tmp"),
                before_limit,
                4,
                |_| {},
            ),
            Err(DocumentError::TooLarge)
        );
    }

    #[test]
    fn document_delete_failure_is_recoverable_and_bounded_by_a_job() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic delete retry");
        let source = workspace.source("delete-me.txt", b"synthetic deletion");
        let documents = meetings.documents();
        let document = documents
            .import_document(&meeting_id, source)
            .expect("document should import");
        let committed = meetings
            .storage_paths()
            .resolve_relative(&document.storage_relative_path)
            .expect("committed path should resolve");

        documents.inject_delete_failure_after_move();
        assert_eq!(
            documents.delete(&document.id),
            Err(DocumentError::DeleteFailed)
        );
        assert!(!committed.exists());
        assert_eq!(
            documents
                .get(&document.id)
                .expect("failed row should remain")
                .status,
            DocumentStatus::DeleteFailed
        );
        let job = document_repository::deletion_job_for_document(
            &documents
                .database
                .connect()
                .expect("database should connect"),
            &document.id,
        )
        .expect("job should read")
        .expect("job should remain");
        assert_eq!(job.attempt_count, 1);

        documents
            .delete(&document.id)
            .expect("explicit retry should complete");
        assert_eq!(documents.get(&document.id), Err(DocumentError::NotFound));
    }

    #[test]
    fn deletion_state_machine_handles_import_failures_pending_and_missing_documents() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let documents = DocumentService::new(database.clone());
        let invalid = workspace.source("invalid.pdf", b"not a PDF");
        assert_eq!(
            documents.import_document(&meeting_id, invalid),
            Err(DocumentError::TypeMismatch)
        );
        let failed = documents
            .list(&meeting_id)
            .expect("failed import should list")
            .pop()
            .expect("failed import row should exist");
        documents
            .delete(&failed.id)
            .expect("import_failed document should delete");

        let source = workspace.source("pending.txt", b"pending synthetic document");
        let staged = documents
            .import_document(&meeting_id, source)
            .expect("staged document should import");
        let mut connection = database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        document_repository::begin_delete(&transaction, &staged, &Uuid::new_v4().to_string())
            .expect("deletion should begin");
        transaction.commit().expect("deletion state should commit");
        assert_eq!(
            documents.delete(&staged.id),
            Err(DocumentError::DeletePending)
        );
        assert_eq!(
            documents.delete(&Uuid::new_v4().to_string()),
            Err(DocumentError::NotFound)
        );
    }

    #[test]
    fn meeting_tombstone_blocks_new_import_and_individual_document_changes() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let documents = DocumentService::new(database.clone());
        let source = workspace.source("before-delete.txt", b"synthetic existing document");
        let document = documents
            .import_document(&meeting_id, source)
            .expect("document should import before tombstone");
        let mut connection = database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        repository::begin_delete(&transaction, &meeting_id, &Uuid::new_v4().to_string())
            .expect("meeting deletion should begin");
        transaction.commit().expect("tombstone should commit");
        let new_source = workspace.source("after-delete.txt", b"synthetic blocked document");

        assert_eq!(
            documents.import_document(&meeting_id, new_source),
            Err(DocumentError::InvalidState)
        );
        assert_eq!(
            documents.delete(&document.id),
            Err(DocumentError::InvalidState)
        );
    }

    #[test]
    fn meeting_delete_removes_committed_and_staging_documents_atomically_enough_to_retry() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic meeting delete");
        let source = workspace.source("attached.txt", b"synthetic attachment");
        let document = meetings
            .documents()
            .import_document(&meeting_id, source)
            .expect("document should import");
        let committed = meetings
            .storage_paths()
            .resolve_relative(&document.storage_relative_path)
            .expect("committed path should resolve");

        meetings.delete(&meeting_id).expect("meeting should delete");
        assert!(!committed.exists());
        assert_eq!(meetings.get(&meeting_id), Err(DataError::NotFound));
        assert_eq!(
            meetings.documents().get(&document.id),
            Err(DocumentError::NotFound)
        );
    }

    #[test]
    fn reconciliation_completes_rename_before_commit_and_fails_staging_only_closed() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let documents = DocumentService::new(database.clone());
        let recovered_id = Uuid::new_v4().to_string();
        let failed_id = Uuid::new_v4().to_string();
        let recovered_staging = database
            .paths()
            .staging_relative(&recovered_id)
            .expect("staging path should build");
        let failed_staging = database
            .paths()
            .staging_relative(&failed_id)
            .expect("staging path should build");
        let mut connection = database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        for (id, path) in [
            (&recovered_id, &recovered_staging),
            (&failed_id, &failed_staging),
        ] {
            document_repository::insert_importing(
                &transaction,
                NewImport {
                    id,
                    meeting_id: &meeting_id,
                    original_filename: "synthetic.txt",
                    safe_extension: "txt",
                    storage_relative_path: path,
                },
            )
            .expect("import row should insert");
        }
        transaction.commit().expect("rows should commit");
        let recovered_final = database
            .paths()
            .document_relative(&meeting_id, &recovered_id, "txt")
            .expect("final relative path should build");
        let recovered_final = database
            .paths()
            .resolve_relative(recovered_final)
            .expect("final path should resolve");
        std::fs::create_dir_all(
            recovered_final
                .parent()
                .expect("final path should have parent"),
        )
        .expect("final parent should create");
        std::fs::write(&recovered_final, b"recovered synthetic file")
            .expect("recovered file should write");
        let failed_staging_path = database
            .paths()
            .resolve_relative(failed_staging)
            .expect("failed staging should resolve");
        std::fs::write(failed_staging_path, b"incomplete synthetic file")
            .expect("staging file should write");

        let report = documents
            .reconcile()
            .expect("reconciliation should succeed");
        assert_eq!(report.imports_completed, 1);
        assert_eq!(report.imports_failed_closed, 1);
        assert_eq!(
            documents
                .get(&recovered_id)
                .expect("recovered document should exist")
                .status,
            DocumentStatus::Staged
        );
        assert_eq!(
            documents
                .get(&failed_id)
                .expect("failed document should remain")
                .status,
            DocumentStatus::ImportFailed
        );
    }

    #[test]
    fn reconciliation_marks_tampering_and_quarantines_unknown_files_without_deleting_them() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic reconciliation");
        let source = workspace.source("integrity.txt", b"original synthetic bytes");
        let documents = meetings.documents();
        let document = documents
            .import_document(&meeting_id, source)
            .expect("document should import");
        let committed = meetings
            .storage_paths()
            .resolve_relative(&document.storage_relative_path)
            .expect("committed path should resolve");
        std::fs::write(&committed, b"tampered bytes").expect("test should alter committed file");
        let unknown = meetings
            .storage_paths()
            .staging()
            .join("unknown-private.tmp");
        std::fs::write(&unknown, b"unknown synthetic bytes").expect("unknown file should write");

        let report = documents
            .reconcile()
            .expect("reconciliation should succeed");
        assert_eq!(report.missing_committed_files, 1);
        assert_eq!(report.quarantined_files, 1);
        assert_eq!(
            documents
                .get(&document.id)
                .expect("document should remain")
                .status,
            DocumentStatus::ImportFailed
        );
        assert!(!unknown.exists());
        assert!(
            std::fs::read_dir(meetings.storage_paths().deletion_trash())
                .expect("quarantine should read")
                .next()
                .is_some()
        );
    }

    #[test]
    fn startup_recovery_retries_document_and_meeting_deletions_then_is_idempotent() {
        let workspace = TestWorkspace::new();
        let (document_id, document_source, meeting_id) = {
            let meetings =
                MeetingService::initialize(&workspace.app_data).expect("service should initialize");
            let document_meeting = create_meeting(&meetings, "Synthetic document recovery");
            let document_source = workspace.source("recovery-source.txt", b"source is retained");
            let documents = meetings.documents();
            let document = documents
                .import_document(&document_meeting, &document_source)
                .expect("document should import");
            documents.inject_delete_failure_after_move();
            assert_eq!(
                documents.delete(&document.id),
                Err(DocumentError::DeleteFailed)
            );

            let meeting_id = create_meeting(&meetings, "Synthetic meeting recovery");
            let attached = workspace.source("meeting-recovery.txt", b"meeting attachment");
            meetings
                .documents()
                .import_document(&meeting_id, attached)
                .expect("meeting document should import");
            let connection = documents
                .database
                .connect()
                .expect("database should connect");
            connection
                .execute_batch(
                    "
                    CREATE TRIGGER synthetic_meeting_recovery_failure
                    BEFORE DELETE ON meetings
                    BEGIN
                        SELECT RAISE(ABORT, 'synthetic failure');
                    END;
                    ",
                )
                .expect("failure trigger should install");
            drop(connection);
            assert_eq!(
                meetings.delete(&meeting_id),
                Err(DataError::MeetingDeleteFailed)
            );
            let connection = documents
                .database
                .connect()
                .expect("database should connect");
            connection
                .execute_batch("DROP TRIGGER synthetic_meeting_recovery_failure;")
                .expect("failure trigger should remove");
            (document.id, document_source, meeting_id)
        };

        let reopened =
            MeetingService::initialize(&workspace.app_data).expect("startup recovery should run");
        assert_eq!(
            reopened.documents().get(&document_id),
            Err(DocumentError::NotFound)
        );
        assert!(document_source.exists());
        assert_eq!(reopened.get(&meeting_id), Err(DataError::NotFound));
        let report = reopened
            .documents()
            .reconcile()
            .expect("second reconciliation should be idempotent");
        assert_eq!(report, ReconcileReport::default());
        let integrity: String = rusqlite::Connection::open(reopened.storage_paths().database())
            .expect("database should connect")
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check should run");
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn staged_missing_file_fails_closed_without_preventing_startup() {
        let workspace = TestWorkspace::new();
        let document_id = {
            let meetings =
                MeetingService::initialize(&workspace.app_data).expect("service should initialize");
            let meeting_id = create_meeting(&meetings, "Synthetic missing file");
            let source = workspace.source("will-be-missing.txt", b"synthetic committed file");
            let document = meetings
                .documents()
                .import_document(&meeting_id, source)
                .expect("document should import");
            let committed = meetings
                .storage_paths()
                .resolve_relative(&document.storage_relative_path)
                .expect("committed path should resolve");
            std::fs::remove_file(committed).expect("test should remove committed file");
            document.id
        };

        let reopened = MeetingService::initialize(&workspace.app_data)
            .expect("a missing document must not prevent startup");
        assert_eq!(
            reopened
                .documents()
                .get(&document_id)
                .expect("failed document should remain")
                .status,
            DocumentStatus::ImportFailed
        );
    }

    #[test]
    fn reconciliation_repairs_missing_jobs_and_exhausts_inconsistent_jobs() {
        let workspace = TestWorkspace::new();
        let database =
            Database::initialize(&workspace.app_data).expect("database should initialize");
        let meeting_id = insert_meeting(&database);
        let documents = DocumentService::new(database.clone());
        let keep = documents
            .import_document(
                &meeting_id,
                workspace.source("keep.txt", b"synthetic keep document"),
            )
            .expect("first document should import");
        let recover = documents
            .import_document(
                &meeting_id,
                workspace.source("recover.txt", b"synthetic recover document"),
            )
            .expect("second document should import");
        let document_job_id = Uuid::new_v4().to_string();
        let meeting_job_id = Uuid::new_v4().to_string();
        let mut connection = database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        document_repository::ensure_delete_job(&transaction, &keep, &document_job_id)
            .expect("inconsistent document job should insert");
        transaction
            .execute(
                "UPDATE documents SET status = 'deleting' WHERE id = ?1",
                [&recover.id],
            )
            .expect("test should create a missing-job state");
        transaction
            .execute(
                "
                INSERT INTO deletion_jobs(
                    id, entity_kind, meeting_id, document_id, status,
                    attempt_count, created_at, updated_at
                ) VALUES (
                    ?1, 'meeting', ?2, NULL, 'pending', 0,
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                )
                ",
                rusqlite::params![meeting_job_id, meeting_id],
            )
            .expect("inconsistent meeting job should insert");
        transaction
            .commit()
            .expect("inconsistent states should commit");

        let report = documents
            .reconcile()
            .expect("reconciliation should complete");
        assert_eq!(report.deletions_completed, 1);
        assert_eq!(report.deletions_failed, 1);
        assert_eq!(documents.get(&recover.id), Err(DocumentError::NotFound));
        assert_eq!(
            documents
                .get(&keep.id)
                .expect("safe document should remain")
                .status,
            DocumentStatus::Staged
        );
        let connection = database.connect().expect("database should connect");
        for job_id in [document_job_id, meeting_job_id] {
            let (status, attempts): (String, i64) = connection
                .query_row(
                    "SELECT status, attempt_count FROM deletion_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("inconsistent job should remain safely exhausted");
            assert_eq!(status, "failed");
            assert_eq!(attempts, 3);
        }
    }

    fn synthetic_chunk(
        kind: DocumentChunkKind,
        text: &str,
        page_start: Option<u64>,
        page_end: Option<u64>,
        section_path_json: &str,
        table_ordinal: Option<u64>,
    ) -> DocumentChunkInput {
        DocumentChunkInput {
            kind,
            text: text.to_string(),
            content_sha256: sha256_bytes(text.as_bytes()),
            page_start,
            page_end,
            section_path_json: section_path_json.to_string(),
            table_ordinal,
            chunk_schema_version: DOCUMENT_CHUNK_SCHEMA_VERSION,
        }
    }

    #[test]
    fn parse_state_machine_commits_bounded_chunks_and_delete_cascades() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic parsing");
        let documents = meetings.documents();
        let document = documents
            .import_document(
                &meeting_id,
                workspace.source("parse.txt", b"synthetic parser source"),
            )
            .expect("document should import");

        let parsing = documents
            .begin_parse(&document.id)
            .expect("staged document should begin parsing");
        assert_eq!(parsing.status, DocumentStatus::Parsing);
        let parsed = documents
            .commit_parse(
                &document.id,
                "2.117.0",
                Some(2),
                vec![
                    synthetic_chunk(
                        DocumentChunkKind::Heading,
                        "Synthetic heading",
                        Some(1),
                        Some(1),
                        "[]",
                        None,
                    ),
                    synthetic_chunk(
                        DocumentChunkKind::Table,
                        "Synthetic table part one",
                        Some(2),
                        Some(2),
                        "[\"Synthetic heading\"]",
                        Some(0),
                    ),
                    synthetic_chunk(
                        DocumentChunkKind::Table,
                        "Synthetic table part two",
                        Some(2),
                        Some(2),
                        "[\"Synthetic heading\"]",
                        Some(0),
                    ),
                ],
            )
            .expect("validated parser output should commit atomically");
        assert_eq!(parsed.status, DocumentStatus::Parsed);
        assert_eq!(parsed.parser_version_optional.as_deref(), Some("2.117.0"));
        assert_eq!(parsed.page_count_optional, Some(2));
        let chunks = documents
            .chunks(&document.id)
            .expect("parsed chunks should list");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].ordinal, 0);
        assert_eq!(chunks[1].table_ordinal, Some(0));
        assert_eq!(chunks[2].table_ordinal, Some(0));
        assert_eq!(
            documents.chunk(&document.id, &chunks[1].id),
            Ok(chunks[1].clone())
        );
        assert_eq!(
            documents.begin_parse(&document.id),
            Err(DocumentError::InvalidState)
        );

        documents
            .delete(&document.id)
            .expect("parsed document should delete");
        assert_eq!(documents.get(&document.id), Err(DocumentError::NotFound));
        let chunk_count: i64 = documents
            .database
            .connect()
            .expect("database should connect")
            .query_row(
                "SELECT COUNT(*) FROM document_chunks WHERE document_id = ?1",
                [&document.id],
                |row| row.get(0),
            )
            .expect("chunk count should read");
        assert_eq!(chunk_count, 0);
    }

    #[test]
    fn invalid_parse_output_rolls_back_then_failure_and_cancel_use_strict_cas() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic parse failure");
        let documents = meetings.documents();
        let first = documents
            .import_document(
                &meeting_id,
                workspace.source("invalid.txt", b"synthetic invalid parser source"),
            )
            .expect("first document should import");
        documents
            .begin_parse(&first.id)
            .expect("first parse should begin");
        let mut invalid = synthetic_chunk(
            DocumentChunkKind::Paragraph,
            "Synthetic invalid chunk",
            None,
            None,
            "[]",
            None,
        );
        invalid.content_sha256 = "0".repeat(64);
        assert_eq!(
            documents.commit_parse(&first.id, "2.117.0", None, vec![invalid]),
            Err(DocumentError::IntegrityFailed)
        );
        assert_eq!(
            documents
                .get(&first.id)
                .expect("first document should remain")
                .status,
            DocumentStatus::Parsing
        );
        let failed = documents
            .fail_parse(&first.id, DocumentError::IntegrityFailed)
            .expect("running parse should fail closed");
        assert_eq!(failed.status, DocumentStatus::ParseFailed);
        assert_eq!(
            failed.failure_code.as_deref(),
            Some("document_integrity_failed")
        );
        assert_eq!(
            documents.cancel_parse(&first.id),
            Err(DocumentError::InvalidState)
        );

        let second = documents
            .import_document(
                &meeting_id,
                workspace.source("cancel.txt", b"synthetic cancelled parser source"),
            )
            .expect("second document should import");
        documents
            .begin_parse(&second.id)
            .expect("second parse should begin");
        let staged = documents
            .cancel_parse(&second.id)
            .expect("running parse should cancel to staged");
        assert_eq!(staged.status, DocumentStatus::Staged);
        assert!(staged.failure_code.is_none());
        assert_eq!(
            documents.chunks(&second.id),
            Err(DocumentError::InvalidState)
        );
    }

    #[test]
    fn parser_commit_database_failure_is_atomic_and_startup_fails_running_parse_closed() {
        let workspace = TestWorkspace::new();
        let (document_id, database_path) = {
            let meetings =
                MeetingService::initialize(&workspace.app_data).expect("service should initialize");
            let meeting_id = create_meeting(&meetings, "Synthetic atomic parse");
            let documents = meetings.documents();
            let document = documents
                .import_document(
                    &meeting_id,
                    workspace.source("atomic.txt", b"synthetic atomic parser source"),
                )
                .expect("document should import");
            documents
                .begin_parse(&document.id)
                .expect("parse should begin");
            let connection = documents
                .database
                .connect()
                .expect("database should connect");
            connection
                .execute_batch(
                    "
                    CREATE TRIGGER synthetic_chunk_commit_failure
                    BEFORE INSERT ON document_chunks
                    BEGIN
                        SELECT RAISE(ABORT, 'synthetic chunk failure');
                    END;
                    ",
                )
                .expect("failure trigger should install");
            assert_eq!(
                documents.commit_parse(
                    &document.id,
                    "2.117.0",
                    None,
                    vec![synthetic_chunk(
                        DocumentChunkKind::Paragraph,
                        "Synthetic atomic chunk",
                        None,
                        None,
                        "[]",
                        None,
                    )],
                ),
                Err(DocumentError::IntegrityFailed)
            );
            let still_parsing = documents
                .get(&document.id)
                .expect("document should remain after rollback");
            assert_eq!(still_parsing.status, DocumentStatus::Parsing);
            assert!(
                document_repository::list_chunks(&connection, &document.id)
                    .expect("rolled-back chunks should list")
                    .is_empty()
            );
            connection
                .execute_batch("DROP TRIGGER synthetic_chunk_commit_failure;")
                .expect("failure trigger should remove");
            (
                document.id,
                meetings.storage_paths().database().to_path_buf(),
            )
        };

        let reopened =
            MeetingService::initialize(&workspace.app_data).expect("startup should reconcile");
        let interrupted = reopened
            .documents()
            .get(&document_id)
            .expect("interrupted document should remain");
        assert_eq!(interrupted.status, DocumentStatus::ParseFailed);
        assert_eq!(
            interrupted.failure_code.as_deref(),
            Some("document_parse_interrupted")
        );
        let connection =
            rusqlite::Connection::open(database_path).expect("reconciled database should connect");
        let chunk_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM document_chunks WHERE document_id = ?1",
                [&document_id],
                |row| row.get(0),
            )
            .expect("chunk count should read");
        assert_eq!(chunk_count, 0);
    }

    #[test]
    fn reconciliation_fails_parsed_documents_without_live_chunks_closed() {
        let workspace = TestWorkspace::new();
        let meetings =
            MeetingService::initialize(&workspace.app_data).expect("service should initialize");
        let meeting_id = create_meeting(&meetings, "Synthetic parsed integrity");
        let documents = meetings.documents();
        let document = documents
            .import_document(
                &meeting_id,
                workspace.source("missing-chunks.txt", b"synthetic parsed source"),
            )
            .expect("document should import");
        let connection = documents
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "UPDATE documents SET status = 'parsed', parser_version_optional = '2.117.0' \
                 WHERE id = ?1",
                [&document.id],
            )
            .expect("test should create inconsistent parsed state");
        drop(connection);

        let report = documents
            .reconcile()
            .expect("reconciliation should fail parsed document closed");
        assert_eq!(report.parsed_integrity_failures, 1);
        let failed = documents
            .get(&document.id)
            .expect("failed parsed document should remain");
        assert_eq!(failed.status, DocumentStatus::ParseFailed);
        assert_eq!(
            failed.failure_code.as_deref(),
            Some("document_integrity_failed")
        );
    }
}
