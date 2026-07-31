use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::error::{DataError, DataResult};
use super::model::{Meeting, MeetingStatus};

const MEETING_COLUMNS: &str = "
    id,
    title,
    status,
    primary_language,
    selected_window_json,
    started_at,
    ended_at,
    created_at,
    updated_at,
    deletion_error_code
";

pub(crate) struct NewMeeting<'a> {
    pub(crate) id: &'a str,
    pub(crate) title: &'a str,
    pub(crate) primary_language: &'a str,
}

pub(crate) fn insert(transaction: &Transaction<'_>, meeting: NewMeeting<'_>) -> DataResult<()> {
    transaction
        .execute(
            "
            INSERT INTO meetings(
                id, title, status, primary_language, created_at, updated_at
            )
            VALUES (
                ?1, ?2, 'draft', ?3,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            params![meeting.id, meeting.title, meeting.primary_language],
        )
        .map_err(DataError::from)?;
    Ok(())
}

pub(crate) fn get(connection: &Connection, id: &str) -> DataResult<Meeting> {
    let sql =
        format!("SELECT {MEETING_COLUMNS} FROM meetings WHERE id = ?1 AND deleted_at IS NULL");
    connection
        .query_row(&sql, [id], meeting_from_row)
        .optional()
        .map_err(DataError::from)?
        .ok_or(DataError::NotFound)
}

pub(crate) fn get_including_deleted(connection: &Connection, id: &str) -> DataResult<Meeting> {
    let sql = format!("SELECT {MEETING_COLUMNS} FROM meetings WHERE id = ?1");
    connection
        .query_row(&sql, [id], meeting_from_row)
        .optional()
        .map_err(DataError::from)?
        .ok_or(DataError::NotFound)
}

pub(crate) fn list(connection: &Connection) -> DataResult<Vec<Meeting>> {
    let sql = format!(
        "SELECT {MEETING_COLUMNS} FROM meetings WHERE deleted_at IS NULL ORDER BY updated_at DESC, id"
    );
    list_with_sql(connection, &sql)
}

pub(crate) fn list_including_deleted(connection: &Connection) -> DataResult<Vec<Meeting>> {
    let sql = format!("SELECT {MEETING_COLUMNS} FROM meetings ORDER BY updated_at DESC, id");
    list_with_sql(connection, &sql)
}

fn list_with_sql(connection: &Connection, sql: &str) -> DataResult<Vec<Meeting>> {
    let mut statement = connection.prepare(sql).map_err(DataError::from)?;
    let meetings = statement
        .query_map([], meeting_from_row)
        .map_err(DataError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(DataError::from)?;
    Ok(meetings)
}

pub(crate) fn update(
    transaction: &Transaction<'_>,
    id: &str,
    title: &str,
    primary_language: &str,
) -> DataResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE meetings
            SET title = ?2,
                primary_language = ?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND deleted_at IS NULL
            ",
            params![id, title, primary_language],
        )
        .map_err(DataError::from)?;

    if changed == 0 {
        return Err(DataError::NotFound);
    }
    Ok(())
}

pub(crate) fn start(transaction: &Transaction<'_>, id: &str) -> DataResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE meetings
            SET status = 'recording',
                started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
              AND deleted_at IS NULL
              AND status = 'draft'
              AND started_at IS NULL
              AND ended_at IS NULL
            ",
            [id],
        )
        .map_err(DataError::from)?;

    if changed == 0 {
        return Err(DataError::InvalidState);
    }
    Ok(())
}

pub(crate) fn end(transaction: &Transaction<'_>, id: &str) -> DataResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE meetings
            SET status = 'ready',
                ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
              AND deleted_at IS NULL
              AND status = 'recording'
              AND started_at IS NOT NULL
              AND ended_at IS NULL
            ",
            [id],
        )
        .map_err(DataError::from)?;

    if changed == 0 {
        return Err(DataError::InvalidState);
    }
    Ok(())
}

pub(crate) fn begin_delete(
    transaction: &Transaction<'_>,
    id: &str,
    job_id: &str,
) -> DataResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE meetings
            SET status = 'deleting',
                deleted_at = COALESCE(deleted_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                deletion_error_code = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status IN ('draft', 'ready', 'delete_failed')
            ",
            [id],
        )
        .map_err(DataError::from)?;

    if changed == 0 {
        return Err(DataError::InvalidState);
    }

    transaction
        .execute(
            "
            INSERT INTO deletion_jobs(
                id, entity_kind, meeting_id, document_id, status,
                attempt_count, created_at, updated_at
            )
            VALUES (
                ?2, 'meeting', ?1, NULL, 'pending', 0,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ON CONFLICT(meeting_id) WHERE entity_kind = 'meeting'
            DO UPDATE SET
                status = 'pending',
                error_code = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![id, job_id],
        )
        .map_err(DataError::from)?;
    Ok(())
}

pub(crate) fn all_ids_including_deleted(connection: &Connection) -> DataResult<Vec<String>> {
    let mut statement = connection
        .prepare("SELECT id FROM meetings ORDER BY id")
        .map_err(DataError::from)?;
    statement
        .query_map([], |row| row.get(0))
        .map_err(DataError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(DataError::from)
}

pub(crate) fn mark_delete_failed(
    transaction: &Transaction<'_>,
    id: &str,
    error_code: &str,
) -> DataResult<()> {
    transaction
        .execute(
            "
            UPDATE meetings
            SET status = 'delete_failed',
                deletion_error_code = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1 AND status = 'deleting'
            ",
            params![id, error_code],
        )
        .map_err(DataError::from)?;
    transaction
        .execute(
            "
            UPDATE deletion_jobs
            SET status = 'failed',
                attempt_count = MIN(attempt_count + 1, 3),
                error_code = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE entity_kind = 'meeting' AND meeting_id = ?1
            ",
            params![id, error_code],
        )
        .map_err(DataError::from)?;
    Ok(())
}

pub(crate) fn update_meeting_trash_path(
    transaction: &Transaction<'_>,
    id: &str,
    relative_path: &str,
) -> DataResult<()> {
    transaction
        .execute(
            "
            UPDATE deletion_jobs
            SET trash_relative_path = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE entity_kind = 'meeting' AND meeting_id = ?1
            ",
            params![id, relative_path],
        )
        .map_err(DataError::from)?;
    Ok(())
}

pub(crate) fn finalize_delete(transaction: &Transaction<'_>, id: &str) -> DataResult<()> {
    let changed = transaction
        .execute(
            "DELETE FROM meetings WHERE id = ?1 AND status = 'deleting'",
            [id],
        )
        .map_err(DataError::from)?;
    if changed == 0 {
        return Err(DataError::InvalidState);
    }
    Ok(())
}

fn meeting_from_row(row: &Row<'_>) -> rusqlite::Result<Meeting> {
    let status = row.get::<_, String>(2)?;
    let status = MeetingStatus::from_database(&status).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(DataError::Database),
        )
    })?;

    Ok(Meeting {
        id: row.get(0)?,
        title: row.get(1)?,
        status,
        primary_language: row.get(3)?,
        selected_window_json: row.get(4)?,
        started_at: row.get(5)?,
        ended_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deletion_error_code: row.get(9)?,
    })
}
