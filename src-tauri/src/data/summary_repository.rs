use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    SummaryContent, SummaryEvidence, SummaryEvidencePage, SummaryOutputLanguage, SummaryRun,
    SummaryRunPage, SummarySnapshotSegment, SummaryStatus, SummaryTemplate,
};

const SUMMARY_COLUMNS: &str = "
    id, meeting_id, series_id, previous_summary_id, revision, run_purpose, trigger_kind,
    template, output_language, status, provider, provider_mode, model, prompt_version, content_json,
    error_code, source_count, input_segment_count, input_bytes, is_stale, stale_reason,
    stale_at, is_user_edited, snapshot_created_at, created_at, started_at, finished_at,
    cancel_requested_at, updated_at
";

const SNAPSHOT_COLUMNS: &str = "
    id, summary_id, meeting_id, source_id_snapshot, segment_id_snapshot,
    segment_revision, source_ordinal, segment_ordinal, global_ordinal,
    source_type_snapshot, source_display_name_snapshot, source_language_snapshot,
    source_base_started_at_snapshot, speaker_label_snapshot, speaker_name_snapshot,
    start_offset_ms_snapshot, end_offset_ms_snapshot, original_text_snapshot,
    language_snapshot, confidence_snapshot, is_final_snapshot, snapshot_at
";

const EVIDENCE_COLUMNS: &str = "
    id, summary_id, meeting_id, kind, source_id_snapshot,
    transcript_segment_id_snapshot, segment_revision, label, speaker_snapshot,
    start_offset_ms, end_offset_ms, ordinal, global_ordinal, excerpt,
    source_deleted, created_at
";

pub(crate) struct NewSummary<'a> {
    pub(crate) id: &'a str,
    pub(crate) meeting_id: &'a str,
    pub(crate) series_id: &'a str,
    pub(crate) previous_summary_id: Option<&'a str>,
    pub(crate) revision: u64,
    pub(crate) meeting_title_snapshot: &'a str,
    pub(crate) meeting_primary_language_snapshot: &'a str,
    pub(crate) template: SummaryTemplate,
    pub(crate) output_language: SummaryOutputLanguage,
    pub(crate) provider: &'a str,
    pub(crate) provider_mode: &'a str,
    pub(crate) model: &'a str,
    pub(crate) prompt_version: &'a str,
    pub(crate) source_count: usize,
    pub(crate) input_segment_count: usize,
    pub(crate) input_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SummaryMarkdownMetadata {
    pub(crate) meeting_title: String,
}

pub(crate) fn insert_summary(
    transaction: &Transaction<'_>,
    summary: NewSummary<'_>,
) -> SummaryResult<()> {
    transaction
        .execute(
            "
            INSERT INTO summaries(
                id, meeting_id, series_id, previous_summary_id, revision,
                run_purpose, trigger_kind, meeting_title_snapshot,
                meeting_primary_language_snapshot, template, output_language, status,
                provider, provider_mode, model, prompt_version, source_count,
                input_segment_count, input_bytes, snapshot_created_at,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, 'final', 'manual', ?6, ?7, ?8, ?9, 'queued',
                ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            params![
                summary.id,
                summary.meeting_id,
                summary.series_id,
                summary.previous_summary_id,
                sqlite_integer(summary.revision)?,
                summary.meeting_title_snapshot,
                summary.meeting_primary_language_snapshot,
                summary.template.as_str(),
                summary.output_language.as_str(),
                summary.provider,
                summary.provider_mode,
                summary.model,
                summary.prompt_version,
                sqlite_usize(summary.source_count)?,
                sqlite_usize(summary.input_segment_count)?,
                sqlite_usize(summary.input_bytes)?,
            ],
        )
        .map_err(SummaryError::from)?;
    Ok(())
}

pub(crate) fn insert_snapshot(
    transaction: &Transaction<'_>,
    segment: &SummarySnapshotSegment,
) -> SummaryResult<()> {
    transaction
        .execute(
            "
            INSERT INTO summary_sources(
                id, summary_id, meeting_id, source_id_snapshot, segment_id_snapshot,
                segment_revision, source_ordinal, segment_ordinal, global_ordinal,
                source_type_snapshot, source_display_name_snapshot, source_language_snapshot,
                source_base_started_at_snapshot, speaker_label_snapshot, speaker_name_snapshot,
                start_offset_ms_snapshot, end_offset_ms_snapshot, original_text_snapshot,
                language_snapshot, confidence_snapshot, is_final_snapshot, snapshot_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )
            ",
            params![
                &segment.evidence_id,
                &segment.summary_id,
                &segment.meeting_id,
                &segment.source_id,
                &segment.segment_id,
                sqlite_integer(segment.segment_revision)?,
                sqlite_integer(segment.source_ordinal)?,
                sqlite_integer(segment.segment_ordinal)?,
                sqlite_integer(segment.global_ordinal)?,
                &segment.source_type,
                &segment.source_display_name,
                &segment.source_language,
                &segment.source_base_started_at,
                &segment.speaker_label,
                &segment.speaker_name,
                optional_sqlite_integer(segment.start_offset_ms)?,
                optional_sqlite_integer(segment.end_offset_ms)?,
                &segment.original_text,
                &segment.language,
                segment.confidence,
                i64::from(segment.is_final),
                &segment.snapshot_at,
            ],
        )
        .map_err(SummaryError::from)?;
    Ok(())
}

pub(crate) fn get_summary(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<SummaryRun> {
    let sql = format!("SELECT {SUMMARY_COLUMNS} FROM summaries WHERE meeting_id = ?1 AND id = ?2");
    connection
        .query_row(&sql, params![meeting_id, summary_id], summary_from_row)
        .optional()
        .map_err(SummaryError::from)?
        .ok_or(SummaryError::RunNotFound)
}

pub(crate) fn get_summary_by_id(
    connection: &Connection,
    summary_id: &str,
) -> SummaryResult<SummaryRun> {
    let sql = format!("SELECT {SUMMARY_COLUMNS} FROM summaries WHERE id = ?1");
    connection
        .query_row(&sql, [summary_id], summary_from_row)
        .optional()
        .map_err(SummaryError::from)?
        .ok_or(SummaryError::RunNotFound)
}

pub(crate) fn next_revision(connection: &Connection, series_id: &str) -> SummaryResult<u64> {
    let current: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(revision), 0) FROM summaries WHERE series_id = ?1",
            [series_id],
            |row| row.get(0),
        )
        .map_err(SummaryError::from)?;
    u64::try_from(current)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(SummaryError::PersistenceFailed)
}

pub(crate) fn is_latest_summary(
    connection: &Connection,
    series_id: &str,
    summary_id: &str,
) -> SummaryResult<bool> {
    connection
        .query_row(
            "SELECT NOT EXISTS(\
                SELECT 1 FROM summaries \
                WHERE series_id = ?1 AND id <> ?2 \
                  AND revision > (SELECT revision FROM summaries WHERE id = ?2)\
            )",
            params![series_id, summary_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value == 1)
        .map_err(SummaryError::from)
}

pub(crate) fn content_json(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<String> {
    connection
        .query_row(
            "SELECT content_json FROM summaries WHERE meeting_id = ?1 AND id = ?2",
            params![meeting_id, summary_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(SummaryError::from)?
        .flatten()
        .ok_or(SummaryError::InvalidState)
}

pub(crate) fn update_content_cas(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    summary_id: &str,
    previous_content_json: &str,
    content: &SummaryContent,
) -> SummaryResult<bool> {
    let content_json =
        serde_json::to_string(content).map_err(|_| SummaryError::PersistenceFailed)?;
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET content_json = ?4, is_user_edited = 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND status = 'completed'
              AND content_json = ?3
            ",
            params![meeting_id, summary_id, previous_content_json, content_json],
        )
        .map_err(SummaryError::from)?;
    Ok(changed == 1)
}

pub(crate) fn markdown_metadata(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<SummaryMarkdownMetadata> {
    connection
        .query_row(
            "SELECT meeting_title_snapshot FROM summaries WHERE meeting_id = ?1 AND id = ?2",
            params![meeting_id, summary_id],
            |row| {
                Ok(SummaryMarkdownMetadata {
                    meeting_title: row.get(0)?,
                })
            },
        )
        .optional()
        .map_err(SummaryError::from)?
        .ok_or(SummaryError::RunNotFound)
}

pub(crate) fn source_ids_for_summary(
    connection: &Connection,
    summary_id: &str,
) -> SummaryResult<Vec<String>> {
    let mut statement = connection
        .prepare(
            "
            SELECT source_id_snapshot
            FROM summary_sources
            WHERE summary_id = ?1
            GROUP BY source_ordinal, source_id_snapshot
            ORDER BY source_ordinal
            ",
        )
        .map_err(SummaryError::from)?;
    let source_ids = statement
        .query_map([summary_id], |row| row.get(0))
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SummaryError::from)?;
    if source_ids.is_empty() {
        return Err(SummaryError::SnapshotMissing);
    }
    Ok(source_ids)
}

pub(crate) fn source_input_totals(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
) -> SummaryResult<(usize, usize)> {
    let (count, bytes): (i64, i64) = connection
        .query_row(
            "
            SELECT COUNT(*), COALESCE(SUM(length(CAST(original_text AS BLOB))), 0)
            FROM transcript_segments
            WHERE meeting_id = ?1 AND source_id = ?2 AND is_final = 1
            ",
            params![meeting_id, source_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(SummaryError::from)?;
    Ok((
        usize::try_from(count).map_err(|_| SummaryError::PersistenceFailed)?,
        usize::try_from(bytes).map_err(|_| SummaryError::PersistenceFailed)?,
    ))
}

pub(crate) fn load_snapshot(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<Vec<SummarySnapshotSegment>> {
    let sql = format!(
        "SELECT {SNAPSHOT_COLUMNS} FROM summary_sources \
         WHERE meeting_id = ?1 AND summary_id = ?2 AND source_deleted = 0 \
           AND original_text_snapshot IS NOT NULL \
         ORDER BY global_ordinal"
    );
    let mut statement = connection.prepare(&sql).map_err(SummaryError::from)?;
    let segments = statement
        .query_map(params![meeting_id, summary_id], snapshot_from_row)
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SummaryError::from)?;
    if segments.is_empty() {
        return Err(SummaryError::SnapshotMissing);
    }
    Ok(segments)
}

pub(crate) fn transition_to_running(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'running',
                started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND status = 'queued'
            ",
            params![meeting_id, summary_id],
        )
        .map_err(SummaryError::from)?;
    if changed == 0 {
        get_summary(transaction, meeting_id, summary_id)?;
        return Err(SummaryError::InvalidState);
    }
    Ok(())
}

pub(crate) fn complete(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    summary_id: &str,
    content: &SummaryContent,
    evidence: &[SummaryEvidence],
) -> SummaryResult<bool> {
    for item in evidence {
        insert_evidence(transaction, item)?;
    }
    let content_json =
        serde_json::to_string(content).map_err(|_| SummaryError::ProviderOutputInvalid)?;
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'completed', content_json = ?3, error_code = NULL,
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND status = 'running'
            ",
            params![meeting_id, summary_id, content_json],
        )
        .map_err(SummaryError::from)?;
    if changed == 0 {
        return Ok(false);
    }
    Ok(true)
}

fn insert_evidence(transaction: &Transaction<'_>, evidence: &SummaryEvidence) -> SummaryResult<()> {
    transaction
        .execute(
            "
            INSERT INTO evidence_items(
                id, summary_id, meeting_id, summary_source_id, kind,
                source_id_snapshot, transcript_segment_id_snapshot, segment_revision,
                label, speaker_snapshot, start_offset_ms, end_offset_ms, ordinal,
                global_ordinal, excerpt, source_deleted, created_at
            ) VALUES (
                ?1, ?2, ?3, ?1, 'transcript', ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13, ?14,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
            params![
                &evidence.id,
                &evidence.summary_id,
                &evidence.meeting_id,
                &evidence.source_id,
                &evidence.transcript_segment_id,
                sqlite_integer(evidence.segment_revision)?,
                &evidence.label,
                &evidence.speaker_snapshot,
                optional_sqlite_integer(evidence.start_offset_ms)?,
                optional_sqlite_integer(evidence.end_offset_ms)?,
                sqlite_integer(evidence.ordinal)?,
                sqlite_integer(evidence.global_ordinal)?,
                &evidence.excerpt,
                i64::from(evidence.source_deleted),
            ],
        )
        .map_err(SummaryError::from)?;
    Ok(())
}

pub(crate) fn fail_running(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    summary_id: &str,
    error_code: &str,
) -> SummaryResult<bool> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'failed', content_json = NULL, error_code = ?3,
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND status = 'running'
            ",
            params![meeting_id, summary_id, error_code],
        )
        .map_err(SummaryError::from)?;
    Ok(changed > 0)
}

pub(crate) fn cancel(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<()> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'cancelled', content_json = NULL, error_code = NULL,
                cancel_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND status IN ('queued', 'running')
            ",
            params![meeting_id, summary_id],
        )
        .map_err(SummaryError::from)?;
    if changed == 0 {
        let current = get_summary(transaction, meeting_id, summary_id)?;
        if current.status != SummaryStatus::Cancelled {
            return Err(SummaryError::InvalidState);
        }
    }
    Ok(())
}

pub(crate) fn recover_interrupted(transaction: &Transaction<'_>) -> SummaryResult<u64> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'failed', content_json = NULL,
                error_code = 'summary_interrupted',
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE status = 'running'
            ",
            [],
        )
        .map_err(SummaryError::from)?;
    u64::try_from(changed).map_err(|_| SummaryError::PersistenceFailed)
}

pub(crate) fn list_summaries(
    connection: &Connection,
    meeting_id: &str,
    cursor: Option<&str>,
    limit: u64,
) -> SummaryResult<SummaryRunPage> {
    let total: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summaries WHERE meeting_id = ?1",
            [meeting_id],
            |row| row.get(0),
        )
        .map_err(SummaryError::from)?;
    let anchor_created_at = match cursor {
        Some(cursor) => Some(
            connection
                .query_row(
                    "SELECT created_at FROM summaries WHERE meeting_id = ?1 AND id = ?2",
                    params![meeting_id, cursor],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(SummaryError::from)?
                .ok_or(SummaryError::InvalidInput)?,
        ),
        None => None,
    };
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS} FROM summaries WHERE meeting_id = ?1 \
         AND (?2 IS NULL OR created_at < ?3 OR (created_at = ?3 AND id > ?2)) \
         ORDER BY created_at DESC, id LIMIT ?4"
    );
    let fetch_limit = limit.checked_add(1).ok_or(SummaryError::InvalidInput)?;
    let mut statement = connection.prepare(&sql).map_err(SummaryError::from)?;
    let mut items = statement
        .query_map(
            params![
                meeting_id,
                cursor,
                anchor_created_at.as_deref(),
                sqlite_integer(fetch_limit)?
            ],
            summary_from_row,
        )
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SummaryError::from)?;
    let has_more = items.len() > usize::try_from(limit).map_err(|_| SummaryError::InvalidInput)?;
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items.last().map(|item| item.id.clone())
    } else {
        None
    };
    Ok(SummaryRunPage {
        items,
        next_cursor,
        has_more,
        total_count: u64::try_from(total).map_err(|_| SummaryError::PersistenceFailed)?,
    })
}

pub(crate) fn list_evidence(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
    cursor: Option<u64>,
    limit: u64,
) -> SummaryResult<SummaryEvidencePage> {
    get_summary(connection, meeting_id, summary_id)?;
    let total: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM evidence_items WHERE meeting_id = ?1 AND summary_id = ?2",
            params![meeting_id, summary_id],
            |row| row.get(0),
        )
        .map_err(SummaryError::from)?;
    let sql = format!(
        "SELECT {EVIDENCE_COLUMNS} FROM evidence_items \
         WHERE meeting_id = ?1 AND summary_id = ?2 AND global_ordinal > ?3 \
         ORDER BY global_ordinal, id LIMIT ?4"
    );
    let fetch_limit = limit.checked_add(1).ok_or(SummaryError::InvalidInput)?;
    let cursor_value = cursor.map(sqlite_integer).transpose()?.unwrap_or(-1);
    let mut statement = connection.prepare(&sql).map_err(SummaryError::from)?;
    let mut items = statement
        .query_map(
            params![
                meeting_id,
                summary_id,
                cursor_value,
                sqlite_integer(fetch_limit)?
            ],
            evidence_from_row,
        )
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SummaryError::from)?;
    let has_more = items.len() > usize::try_from(limit).map_err(|_| SummaryError::InvalidInput)?;
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items.last().map(|item| item.global_ordinal)
    } else {
        None
    };
    Ok(SummaryEvidencePage {
        items,
        next_cursor,
        has_more,
        total_count: u64::try_from(total).map_err(|_| SummaryError::PersistenceFailed)?,
    })
}

pub(crate) fn get_evidence(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
    evidence_id: &str,
) -> SummaryResult<SummaryEvidence> {
    let sql = format!(
        "SELECT {EVIDENCE_COLUMNS} FROM evidence_items \
         WHERE meeting_id = ?1 AND summary_id = ?2 AND id = ?3"
    );
    connection
        .query_row(
            &sql,
            params![meeting_id, summary_id, evidence_id],
            evidence_from_row,
        )
        .optional()
        .map_err(SummaryError::from)?
        .ok_or(SummaryError::EvidenceInvalid)
}

pub(crate) fn all_evidence(
    connection: &Connection,
    meeting_id: &str,
    summary_id: &str,
) -> SummaryResult<Vec<SummaryEvidence>> {
    let sql = format!(
        "SELECT {EVIDENCE_COLUMNS} FROM evidence_items \
         WHERE meeting_id = ?1 AND summary_id = ?2 ORDER BY global_ordinal, id"
    );
    let mut statement = connection.prepare(&sql).map_err(SummaryError::from)?;
    statement
        .query_map(params![meeting_id, summary_id], evidence_from_row)
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(SummaryError::from)
}

pub(crate) fn mark_stale_for_segment(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    segment_id: &str,
    current_revision: u64,
) -> SummaryResult<u64> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET is_stale = 1,
                stale_reason = CASE
                    WHEN stale_reason = 'source_deleted' THEN stale_reason
                    ELSE 'segment_revised'
                END,
                stale_at = COALESCE(stale_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id IN (
                SELECT summary_id FROM summary_sources
                WHERE meeting_id = ?1 AND segment_id_snapshot = ?2
                  AND segment_revision <> ?3 AND source_deleted = 0
            )
            ",
            params![meeting_id, segment_id, sqlite_integer(current_revision)?],
        )
        .map_err(SummaryError::from)?;
    u64::try_from(changed).map_err(|_| SummaryError::PersistenceFailed)
}

pub(crate) fn mark_stale_for_changed_source_segments(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    source_id: &str,
) -> SummaryResult<u64> {
    let changed = transaction
        .execute(
            "
            UPDATE summaries
            SET is_stale = 1,
                stale_reason = CASE
                    WHEN stale_reason = 'source_deleted' THEN stale_reason
                    ELSE 'segment_revised'
                END,
                stale_at = COALESCE(stale_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id IN (
                SELECT snapshot.summary_id
                FROM summary_sources AS snapshot
                JOIN transcript_segments AS live
                  ON live.meeting_id = snapshot.meeting_id
                 AND live.id = snapshot.segment_id_snapshot
                WHERE snapshot.meeting_id = ?1
                  AND snapshot.source_id_snapshot = ?2
                  AND snapshot.source_deleted = 0
                  AND snapshot.segment_revision <> live.revision
            )
            ",
            params![meeting_id, source_id],
        )
        .map_err(SummaryError::from)?;
    u64::try_from(changed).map_err(|_| SummaryError::PersistenceFailed)
}

pub(crate) fn source_has_active_summary(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
) -> SummaryResult<bool> {
    connection
        .query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM summary_sources AS snapshot
                JOIN summaries AS summary ON summary.id = snapshot.summary_id
                WHERE snapshot.meeting_id = ?1
                  AND snapshot.source_id_snapshot = ?2
                  AND summary.status IN ('queued', 'running')
            )
            ",
            params![meeting_id, source_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value == 1)
        .map_err(SummaryError::from)
}

pub(crate) fn redact_terminal_source(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    source_id: &str,
) -> SummaryResult<()> {
    transaction
        .execute(
            "
            UPDATE summaries
            SET is_stale = 1,
                stale_reason = 'source_deleted',
                stale_at = COALESCE(stale_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id IN (
                SELECT summary_id FROM summary_sources
                WHERE meeting_id = ?1 AND source_id_snapshot = ?2
            )
            ",
            params![meeting_id, source_id],
        )
        .map_err(SummaryError::from)?;
    transaction
        .execute(
            "
            UPDATE evidence_items
            SET source_deleted = 1
            WHERE meeting_id = ?1 AND source_id_snapshot = ?2
            ",
            params![meeting_id, source_id],
        )
        .map_err(SummaryError::from)?;
    transaction
        .execute(
            "
            UPDATE summary_sources
            SET source_display_name_snapshot = NULL,
                source_language_snapshot = NULL,
                source_base_started_at_snapshot = NULL,
                speaker_label_snapshot = NULL,
                speaker_name_snapshot = NULL,
                start_offset_ms_snapshot = NULL,
                end_offset_ms_snapshot = NULL,
                original_text_snapshot = NULL,
                language_snapshot = NULL,
                confidence_snapshot = NULL,
                source_deleted = 1,
                redacted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND source_id_snapshot = ?2
              AND summary_id IN (
                  SELECT id FROM summaries
                  WHERE meeting_id = ?1 AND status IN ('completed', 'failed', 'cancelled')
              )
            ",
            params![meeting_id, source_id],
        )
        .map_err(SummaryError::from)?;
    Ok(())
}

pub(crate) fn cancel_active_for_meeting(
    transaction: &Transaction<'_>,
    meeting_id: &str,
) -> SummaryResult<Vec<String>> {
    let mut statement = transaction
        .prepare(
            "SELECT id FROM summaries WHERE meeting_id = ?1 AND status = 'running' ORDER BY id",
        )
        .map_err(SummaryError::from)?;
    let running_ids = statement
        .query_map([meeting_id], |row| row.get(0))
        .map_err(SummaryError::from)?
        .collect::<Result<Vec<String>, _>>()
        .map_err(SummaryError::from)?;
    drop(statement);
    transaction
        .execute(
            "
            UPDATE summaries
            SET status = 'cancelled', content_json = NULL, error_code = NULL,
                cancel_requested_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND status IN ('queued', 'running')
            ",
            [meeting_id],
        )
        .map_err(SummaryError::from)?;
    Ok(running_ids)
}

pub(crate) fn referenced_evidence_ids(content: &SummaryContent) -> Vec<String> {
    let mut ids = Vec::new();
    ids.extend(content.overview.evidence_ids.iter().cloned());
    for decision in &content.decisions {
        ids.extend(decision.evidence_ids.iter().cloned());
    }
    for item in &content.action_items {
        ids.extend(item.evidence_ids.iter().cloned());
    }
    for item in &content.risks {
        ids.extend(item.evidence_ids.iter().cloned());
    }
    for item in &content.open_questions {
        ids.extend(item.evidence_ids.iter().cloned());
    }
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    ids
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<SummaryRun> {
    let template_value: String = row.get(7)?;
    let output_language_value: String = row.get(8)?;
    let status_value: String = row.get(9)?;
    let content_json: Option<String> = row.get(14)?;
    let content = content_json
        .map(|value| serde_json::from_str::<SummaryContent>(&value))
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                14,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(SummaryRun {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        series_id: row.get(2)?,
        previous_summary_id: row.get(3)?,
        revision: nonnegative_integer(row, 4)?,
        run_purpose: row.get(5)?,
        trigger_kind: row.get(6)?,
        template: SummaryTemplate::from_database(&template_value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        output_language: SummaryOutputLanguage::from_database(&output_language_value).map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            },
        )?,
        status: SummaryStatus::from_database(&status_value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                9,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        provider: row.get(10)?,
        provider_mode: row.get(11)?,
        model: row.get(12)?,
        prompt_version: row.get(13)?,
        content,
        error_code: row.get(15)?,
        source_count: nonnegative_integer(row, 16)?,
        input_segment_count: nonnegative_integer(row, 17)?,
        input_bytes: nonnegative_integer(row, 18)?,
        is_stale: row.get::<_, i64>(19)? == 1,
        stale_reason: row.get(20)?,
        stale_at: row.get(21)?,
        is_user_edited: row.get::<_, i64>(22)? == 1,
        snapshot_created_at: row.get(23)?,
        created_at: row.get(24)?,
        started_at: row.get(25)?,
        finished_at: row.get(26)?,
        cancel_requested_at: row.get(27)?,
        updated_at: row.get(28)?,
    })
}

fn snapshot_from_row(row: &Row<'_>) -> rusqlite::Result<SummarySnapshotSegment> {
    let original_text = row.get::<_, Option<String>>(17)?.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            17,
            rusqlite::types::Type::Null,
            Box::new(SummaryError::SnapshotMissing),
        )
    })?;
    Ok(SummarySnapshotSegment {
        evidence_id: row.get(0)?,
        summary_id: row.get(1)?,
        meeting_id: row.get(2)?,
        source_id: row.get(3)?,
        segment_id: row.get(4)?,
        segment_revision: nonnegative_integer(row, 5)?,
        source_ordinal: nonnegative_integer(row, 6)?,
        segment_ordinal: nonnegative_integer(row, 7)?,
        global_ordinal: nonnegative_integer(row, 8)?,
        source_type: row.get(9)?,
        source_display_name: row.get(10)?,
        source_language: row.get(11)?,
        source_base_started_at: row.get(12)?,
        speaker_label: row.get(13)?,
        speaker_name: row.get(14)?,
        start_offset_ms: optional_nonnegative_integer(row, 15)?,
        end_offset_ms: optional_nonnegative_integer(row, 16)?,
        original_text,
        language: row.get(18)?,
        confidence: row.get(19)?,
        is_final: row.get::<_, i64>(20)? == 1,
        snapshot_at: row.get(21)?,
    })
}

fn evidence_from_row(row: &Row<'_>) -> rusqlite::Result<SummaryEvidence> {
    Ok(SummaryEvidence {
        id: row.get(0)?,
        summary_id: row.get(1)?,
        meeting_id: row.get(2)?,
        kind: row.get(3)?,
        source_id: row.get(4)?,
        transcript_segment_id: row.get(5)?,
        segment_revision: nonnegative_integer(row, 6)?,
        label: row.get(7)?,
        speaker_snapshot: row.get(8)?,
        start_offset_ms: optional_nonnegative_integer(row, 9)?,
        end_offset_ms: optional_nonnegative_integer(row, 10)?,
        ordinal: nonnegative_integer(row, 11)?,
        global_ordinal: nonnegative_integer(row, 12)?,
        excerpt: row.get(13)?,
        source_deleted: row.get::<_, i64>(14)? == 1,
        created_at: row.get(15)?,
    })
}

fn sqlite_integer(value: u64) -> SummaryResult<i64> {
    i64::try_from(value).map_err(|_| SummaryError::InvalidInput)
}

fn sqlite_usize(value: usize) -> SummaryResult<i64> {
    i64::try_from(value).map_err(|_| SummaryError::InvalidInput)
}

fn optional_sqlite_integer(value: Option<u64>) -> SummaryResult<Option<i64>> {
    value.map(sqlite_integer).transpose()
}

fn nonnegative_integer(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn optional_nonnegative_integer(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| {
            u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
        })
        .transpose()
}
