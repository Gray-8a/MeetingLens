use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::document_error::{DocumentError, DocumentResult};
use super::document_model::{DeletionJob, Document, DocumentStatus};

const DOCUMENT_COLUMNS: &str = "
    id, meeting_id, original_filename, safe_extension, detected_media_type,
    size_bytes, sha256, status, storage_relative_path, imported_at,
    updated_at, failure_code, failure_message_safe, parser_version_optional,
    page_count_optional, created_at
";

pub(crate) struct NewImport<'a> {
    pub id: &'a str,
    pub meeting_id: &'a str,
    pub original_filename: &'a str,
    pub safe_extension: &'a str,
    pub storage_relative_path: &'a str,
}

pub(crate) fn insert_importing(
    transaction: &Transaction<'_>,
    input: NewImport<'_>,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            INSERT INTO documents(
                id, meeting_id, original_filename, safe_extension, status,
                storage_relative_path, created_at, updated_at
            )
            VALUES (
                ?1, ?2, ?3, ?4, 'importing', ?5,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            params![
                input.id,
                input.meeting_id,
                input.original_filename,
                input.safe_extension,
                input.storage_relative_path
            ],
        )
        .map_err(|_| DocumentError::ImportFailed)?;
    Ok(())
}

pub(crate) fn get(connection: &Connection, id: &str) -> DocumentResult<Document> {
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents WHERE id = ?1");
    connection
        .query_row(&sql, [id], document_from_row)
        .optional()
        .map_err(|_| DocumentError::Internal)?
        .ok_or(DocumentError::NotFound)
}

pub(crate) fn list(connection: &Connection, meeting_id: &str) -> DocumentResult<Vec<Document>> {
    let sql = format!(
        "SELECT {DOCUMENT_COLUMNS} FROM documents WHERE meeting_id = ?1 ORDER BY created_at, id"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| DocumentError::Internal)?;
    statement
        .query_map([meeting_id], document_from_row)
        .map_err(|_| DocumentError::Internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DocumentError::Internal)
}

pub(crate) fn all(connection: &Connection) -> DocumentResult<Vec<Document>> {
    let sql = format!("SELECT {DOCUMENT_COLUMNS} FROM documents ORDER BY created_at, id");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| DocumentError::Internal)?;
    statement
        .query_map([], document_from_row)
        .map_err(|_| DocumentError::Internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DocumentError::Internal)
}

pub(crate) fn usage(
    connection: &Connection,
    meeting_id: &str,
    excluding_id: Option<&str>,
) -> DocumentResult<(u64, u64)> {
    let (count, bytes): (i64, i64) = connection
        .query_row(
            "
            SELECT COUNT(*), COALESCE(SUM(size_bytes), 0)
            FROM documents
            WHERE meeting_id = ?1 AND (?2 IS NULL OR id <> ?2)
            ",
            params![meeting_id, excluding_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok((count as u64, bytes as u64))
}

pub(crate) fn find_by_hash(
    connection: &Connection,
    meeting_id: &str,
    sha256: &str,
    excluding_id: &str,
) -> DocumentResult<Option<Document>> {
    let sql = format!(
        "SELECT {DOCUMENT_COLUMNS} FROM documents WHERE meeting_id = ?1 AND sha256 = ?2 AND id <> ?3"
    );
    connection
        .query_row(
            &sql,
            params![meeting_id, sha256, excluding_id],
            document_from_row,
        )
        .optional()
        .map_err(|_| DocumentError::Internal)
}

pub(crate) fn mark_staged(
    transaction: &Transaction<'_>,
    id: &str,
    media_type: &str,
    size_bytes: u64,
    sha256: &str,
    relative_path: &str,
) -> DocumentResult<()> {
    let size_bytes = i64::try_from(size_bytes).map_err(|_| DocumentError::TooLarge)?;
    let changed = transaction
        .execute(
            "
            UPDATE documents
            SET detected_media_type = ?2,
                size_bytes = ?3,
                sha256 = ?4,
                status = 'staged',
                storage_relative_path = ?5,
                imported_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                failure_code = NULL,
                failure_message_safe = NULL
            WHERE id = ?1 AND status = 'importing'
            ",
            params![id, media_type, size_bytes, sha256, relative_path],
        )
        .map_err(|_| DocumentError::ImportFailed)?;
    if changed == 0 {
        return Err(DocumentError::InvalidState);
    }
    Ok(())
}

pub(crate) fn mark_import_failed(
    transaction: &Transaction<'_>,
    id: &str,
    error: &DocumentError,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            UPDATE documents
            SET status = 'import_failed',
                failure_code = ?2,
                failure_message_safe = ?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status = 'importing'
            ",
            params![id, error.code(), error.safe_message()],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn mark_integrity_failed(transaction: &Transaction<'_>, id: &str) -> DocumentResult<()> {
    transaction
        .execute(
            "
            UPDATE documents
            SET status = 'import_failed',
                failure_code = 'document_integrity_failed',
                failure_message_safe = 'The document could not be verified safely.',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status = 'staged'
            ",
            [id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn remove_importing(transaction: &Transaction<'_>, id: &str) -> DocumentResult<()> {
    transaction
        .execute(
            "DELETE FROM documents WHERE id = ?1 AND status = 'importing'",
            [id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn begin_delete(
    transaction: &Transaction<'_>,
    document: &Document,
    job_id: &str,
) -> DocumentResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE documents
            SET status = 'deleting',
                failure_code = NULL,
                failure_message_safe = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status IN ('staged', 'import_failed', 'delete_failed')
            ",
            [&document.id],
        )
        .map_err(|_| DocumentError::Internal)?;
    if changed == 0 {
        return Err(DocumentError::InvalidState);
    }
    transaction
        .execute(
            "
            INSERT INTO deletion_jobs(
                id, entity_kind, meeting_id, document_id, status,
                attempt_count, created_at, updated_at
            )
            VALUES (
                ?1, 'document', ?2, ?3, 'pending', 0,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ON CONFLICT(document_id) WHERE entity_kind = 'document'
            DO UPDATE SET
                status = 'pending',
                error_code = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![job_id, document.meeting_id, document.id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn ensure_delete_job(
    transaction: &Transaction<'_>,
    document: &Document,
    job_id: &str,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            INSERT INTO deletion_jobs(
                id, entity_kind, meeting_id, document_id, status,
                attempt_count, created_at, updated_at
            )
            VALUES (
                ?1, 'document', ?2, ?3, 'pending', 0,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ON CONFLICT(document_id) WHERE entity_kind = 'document'
            DO NOTHING
            ",
            params![job_id, document.meeting_id, document.id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn exhaust_inconsistent_job(
    transaction: &Transaction<'_>,
    job_id: &str,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            UPDATE deletion_jobs
            SET status = 'failed',
                attempt_count = 3,
                error_code = 'deletion_job_inconsistent',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            [job_id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn update_trash_path(
    transaction: &Transaction<'_>,
    document_id: &str,
    relative_path: &str,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            UPDATE deletion_jobs
            SET trash_relative_path = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE entity_kind = 'document' AND document_id = ?1
            ",
            params![document_id, relative_path],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn mark_delete_failed(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> DocumentResult<()> {
    transaction
        .execute(
            "
            UPDATE documents
            SET status = 'delete_failed',
                failure_code = 'document_delete_failed',
                failure_message_safe = 'The document could not be deleted safely.',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status = 'deleting'
            ",
            [document_id],
        )
        .map_err(|_| DocumentError::Internal)?;
    transaction
        .execute(
            "
            UPDATE deletion_jobs
            SET status = 'failed',
                attempt_count = MIN(attempt_count + 1, 3),
                error_code = 'document_delete_failed',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE entity_kind = 'document' AND document_id = ?1
            ",
            [document_id],
        )
        .map_err(|_| DocumentError::Internal)?;
    Ok(())
}

pub(crate) fn finalize_delete(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> DocumentResult<()> {
    let changed = transaction
        .execute(
            "DELETE FROM documents WHERE id = ?1 AND status = 'deleting'",
            [document_id],
        )
        .map_err(|_| DocumentError::Internal)?;
    if changed == 0 {
        return Err(DocumentError::InvalidState);
    }
    Ok(())
}

pub(crate) fn deletion_jobs(connection: &Connection) -> DocumentResult<Vec<DeletionJob>> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, entity_kind, meeting_id, document_id, status,
                   trash_relative_path, attempt_count
            FROM deletion_jobs
            ORDER BY created_at, id
            ",
        )
        .map_err(|_| DocumentError::Internal)?;
    statement
        .query_map([], |row| {
            Ok(DeletionJob {
                id: row.get(0)?,
                entity_kind: row.get(1)?,
                meeting_id: row.get(2)?,
                document_id: row.get(3)?,
                status: row.get(4)?,
                trash_relative_path: row.get(5)?,
                attempt_count: row.get::<_, i64>(6)? as u64,
            })
        })
        .map_err(|_| DocumentError::Internal)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DocumentError::Internal)
}

pub(crate) fn deletion_job_for_document(
    connection: &Connection,
    document_id: &str,
) -> DocumentResult<Option<DeletionJob>> {
    connection
        .query_row(
            "
            SELECT id, entity_kind, meeting_id, document_id, status,
                   trash_relative_path, attempt_count
            FROM deletion_jobs
            WHERE entity_kind = 'document' AND document_id = ?1
            ",
            [document_id],
            |row| {
                Ok(DeletionJob {
                    id: row.get(0)?,
                    entity_kind: row.get(1)?,
                    meeting_id: row.get(2)?,
                    document_id: row.get(3)?,
                    status: row.get(4)?,
                    trash_relative_path: row.get(5)?,
                    attempt_count: row.get::<_, i64>(6)? as u64,
                })
            },
        )
        .optional()
        .map_err(|_| DocumentError::Internal)
}

fn document_from_row(row: &Row<'_>) -> rusqlite::Result<Document> {
    let status = row.get::<_, String>(7)?;
    let status = DocumentStatus::from_database(&status).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            Box::new(DocumentError::Internal),
        )
    })?;
    Ok(Document {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        original_filename: row.get(2)?,
        safe_extension: row.get(3)?,
        detected_media_type: row.get(4)?,
        size_bytes: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
        sha256: row.get(6)?,
        status,
        storage_relative_path: row.get(8)?,
        imported_at: row.get(9)?,
        updated_at: row.get(10)?,
        failure_code: row.get(11)?,
        failure_message_safe: row.get(12)?,
        parser_version_optional: row.get(13)?,
        page_count_optional: row.get::<_, Option<i64>>(14)?.map(|value| value as u64),
        created_at: row.get(15)?,
    })
}
