use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::transcript_error::{TranscriptError, TranscriptResult};
use super::transcript_model::{
    NewTranscriptSegment, NewTranscriptSource, SummaryInputSegment, TranscriptSegment,
    TranscriptSegmentPage, TranscriptSource, TranscriptSourceOverview, TranscriptSourceType,
    TranscriptSpeakerGroup, TranscriptSpeakerGroupPage,
};

const SOURCE_COLUMNS: &str = "
    id, meeting_id, source_type, display_name, original_filename,
    content_sha256, language, base_started_at, parser_name, parser_version,
    imported_at, created_at, updated_at
";

const SEGMENT_COLUMNS: &str = "
    id, meeting_id, source_id, ordinal, speaker_label, speaker_name,
    start_offset_ms, end_offset_ms, original_text, translated_text,
    language, confidence, is_final, revision, created_at, corrected_at
";

const SOURCE_OVERVIEW_COLUMNS: &str = "
    source.id, source.meeting_id, source.source_type, source.display_name,
    source.original_filename, source.content_sha256, source.language,
    source.base_started_at, source.parser_name, source.parser_version,
    source.imported_at, source.created_at, source.updated_at,
    (
        SELECT COUNT(*)
        FROM transcript_segments AS segment
        WHERE segment.meeting_id = source.meeting_id
          AND segment.source_id = source.id
    ) AS segment_count,
    (
        SELECT MIN(segment.start_offset_ms)
        FROM transcript_segments AS segment
        WHERE segment.meeting_id = source.meeting_id
          AND segment.source_id = source.id
    ) AS start_offset_ms,
    (
        SELECT MAX(segment.end_offset_ms)
        FROM transcript_segments AS segment
        WHERE segment.meeting_id = source.meeting_id
          AND segment.source_id = source.id
    ) AS end_offset_ms,
    (
        SELECT COUNT(DISTINCT segment.speaker_label)
        FROM transcript_segments AS segment
        WHERE segment.meeting_id = source.meeting_id
          AND segment.source_id = source.id
          AND segment.speaker_label IS NOT NULL
    ) AS speaker_count
";

pub(crate) fn find_source_by_hash(
    connection: &Connection,
    meeting_id: &str,
    content_sha256: &str,
) -> TranscriptResult<Option<TranscriptSource>> {
    let sql = format!(
        "SELECT {SOURCE_COLUMNS} FROM transcript_sources WHERE meeting_id = ?1 AND content_sha256 = ?2"
    );
    connection
        .query_row(&sql, params![meeting_id, content_sha256], source_from_row)
        .optional()
        .map_err(TranscriptError::from)
}

pub(crate) fn insert_source(
    transaction: &Transaction<'_>,
    source: &NewTranscriptSource,
) -> TranscriptResult<()> {
    let inserted = transaction.execute(
        "
            INSERT INTO transcript_sources(
                id, meeting_id, source_type, display_name, original_filename,
                content_sha256, language, base_started_at, parser_name, parser_version,
                imported_at, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
        params![
            source.id,
            source.meeting_id,
            source.source_type.as_str(),
            source.display_name,
            source.original_filename,
            source.content_sha256,
            source.language,
            source.base_started_at,
            source.parser_name,
            source.parser_version,
        ],
    );
    if let Err(error) = inserted {
        if let Some(existing) =
            find_source_by_hash(transaction, &source.meeting_id, &source.content_sha256)?
        {
            return Err(TranscriptError::AlreadyImported {
                source_id: existing.id,
            });
        }
        return Err(TranscriptError::from(error));
    }
    Ok(())
}

pub(crate) fn insert_segments(
    transaction: &Transaction<'_>,
    segments: &[NewTranscriptSegment],
) -> TranscriptResult<()> {
    let mut statement = transaction
        .prepare(
            "
            INSERT INTO transcript_segments(
                id, meeting_id, source_id, ordinal, speaker_label, speaker_name,
                start_offset_ms, end_offset_ms, original_text, translated_text,
                language, confidence, is_final, revision, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            ",
        )
        .map_err(TranscriptError::from)?;

    for segment in segments {
        let ordinal = sqlite_integer(segment.ordinal)?;
        let start_offset_ms = segment.start_offset_ms.map(sqlite_integer).transpose()?;
        let end_offset_ms = segment.end_offset_ms.map(sqlite_integer).transpose()?;
        statement
            .execute(params![
                segment.id,
                segment.meeting_id,
                segment.source_id,
                ordinal,
                segment.speaker_label,
                segment.speaker_name,
                start_offset_ms,
                end_offset_ms,
                segment.original_text,
                segment.translated_text,
                segment.language,
                segment.confidence,
                i64::from(segment.is_final),
            ])
            .map_err(TranscriptError::from)?;
    }
    Ok(())
}

pub(crate) fn get_source(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
) -> TranscriptResult<TranscriptSource> {
    let sql = format!(
        "SELECT {SOURCE_COLUMNS} FROM transcript_sources WHERE meeting_id = ?1 AND id = ?2"
    );
    connection
        .query_row(&sql, params![meeting_id, source_id], source_from_row)
        .optional()
        .map_err(TranscriptError::from)?
        .ok_or(TranscriptError::SourceNotFound)
}

pub(crate) fn list_sources(
    connection: &Connection,
    meeting_id: &str,
) -> TranscriptResult<Vec<TranscriptSource>> {
    let sql = format!(
        "SELECT {SOURCE_COLUMNS} FROM transcript_sources WHERE meeting_id = ?1 ORDER BY imported_at, id"
    );
    let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
    statement
        .query_map([meeting_id], source_from_row)
        .map_err(TranscriptError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(TranscriptError::from)
}

pub(crate) fn get_source_overview(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
) -> TranscriptResult<TranscriptSourceOverview> {
    let sql = format!(
        "SELECT {SOURCE_OVERVIEW_COLUMNS} FROM transcript_sources AS source \
         WHERE source.meeting_id = ?1 AND source.id = ?2"
    );
    connection
        .query_row(
            &sql,
            params![meeting_id, source_id],
            source_overview_from_row,
        )
        .optional()
        .map_err(TranscriptError::from)?
        .ok_or(TranscriptError::SourceNotFound)
}

pub(crate) fn list_source_overviews(
    connection: &Connection,
    meeting_id: &str,
) -> TranscriptResult<Vec<TranscriptSourceOverview>> {
    let sql = format!(
        "SELECT {SOURCE_OVERVIEW_COLUMNS} FROM transcript_sources AS source \
         WHERE source.meeting_id = ?1 ORDER BY source.imported_at, source.id"
    );
    let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
    statement
        .query_map([meeting_id], source_overview_from_row)
        .map_err(TranscriptError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(TranscriptError::from)
}

pub(crate) fn get_segment(
    connection: &Connection,
    meeting_id: &str,
    segment_id: &str,
) -> TranscriptResult<TranscriptSegment> {
    let sql = format!(
        "SELECT {SEGMENT_COLUMNS} FROM transcript_segments WHERE meeting_id = ?1 AND id = ?2"
    );
    connection
        .query_row(&sql, params![meeting_id, segment_id], segment_from_row)
        .optional()
        .map_err(TranscriptError::from)?
        .ok_or(TranscriptError::SegmentNotFound)
}

pub(crate) fn list_segments(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
) -> TranscriptResult<Vec<TranscriptSegment>> {
    let sql = format!(
        "SELECT {SEGMENT_COLUMNS} FROM transcript_segments WHERE meeting_id = ?1 AND source_id = ?2 ORDER BY ordinal, id"
    );
    let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
    statement
        .query_map(params![meeting_id, source_id], segment_from_row)
        .map_err(TranscriptError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(TranscriptError::from)
}

pub(crate) fn list_segment_page(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
    cursor: Option<u64>,
    limit: usize,
) -> TranscriptResult<TranscriptSegmentPage> {
    let total_count = connection
        .query_row(
            "SELECT COUNT(*) FROM transcript_segments WHERE meeting_id = ?1 AND source_id = ?2",
            params![meeting_id, source_id],
            |row| nonnegative_integer(row, 0),
        )
        .map_err(TranscriptError::from)?;
    let query_limit = sqlite_integer(
        u64::try_from(limit)
            .map_err(|_| TranscriptError::InvalidInput)?
            .checked_add(1)
            .ok_or(TranscriptError::InvalidInput)?,
    )?;
    let mut items = match cursor {
        Some(cursor) => {
            let cursor = sqlite_integer(cursor)?;
            let sql = format!(
                "SELECT {SEGMENT_COLUMNS} FROM transcript_segments \
                 WHERE meeting_id = ?1 AND source_id = ?2 AND ordinal > ?3 \
                 ORDER BY ordinal LIMIT ?4"
            );
            let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
            statement
                .query_map(
                    params![meeting_id, source_id, cursor, query_limit],
                    segment_from_row,
                )
                .map_err(TranscriptError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TranscriptError::from)?
        }
        None => {
            let sql = format!(
                "SELECT {SEGMENT_COLUMNS} FROM transcript_segments \
                 WHERE meeting_id = ?1 AND source_id = ?2 \
                 ORDER BY ordinal LIMIT ?3"
            );
            let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
            statement
                .query_map(
                    params![meeting_id, source_id, query_limit],
                    segment_from_row,
                )
                .map_err(TranscriptError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TranscriptError::from)?
        }
    };
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = has_more
        .then(|| items.last().map(|segment| segment.ordinal))
        .flatten();
    Ok(TranscriptSegmentPage {
        items,
        next_cursor,
        has_more,
        total_count,
    })
}

pub(crate) fn list_speaker_group_page(
    connection: &Connection,
    meeting_id: &str,
    source_id: &str,
    cursor: Option<&str>,
    limit: usize,
) -> TranscriptResult<TranscriptSpeakerGroupPage> {
    let total_count = connection
        .query_row(
            "SELECT COUNT(DISTINCT speaker_label) FROM transcript_segments \
             WHERE meeting_id = ?1 AND source_id = ?2 AND speaker_label IS NOT NULL",
            params![meeting_id, source_id],
            |row| nonnegative_integer(row, 0),
        )
        .map_err(TranscriptError::from)?;
    let query_limit = sqlite_integer(
        u64::try_from(limit)
            .map_err(|_| TranscriptError::InvalidInput)?
            .checked_add(1)
            .ok_or(TranscriptError::InvalidInput)?,
    )?;
    let columns = "
        speaker_label,
        MIN(speaker_name) AS minimum_name,
        MAX(speaker_name) AS maximum_name,
        SUM(CASE WHEN speaker_name IS NULL THEN 1 ELSE 0 END) AS null_name_count,
        COUNT(*) AS segment_count
    ";
    let mut items = match cursor {
        Some(cursor) => {
            let sql = format!(
                "SELECT {columns} FROM transcript_segments \
                 WHERE meeting_id = ?1 AND source_id = ?2 \
                   AND speaker_label IS NOT NULL AND speaker_label > ?3 \
                 GROUP BY speaker_label ORDER BY speaker_label LIMIT ?4"
            );
            let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
            statement
                .query_map(params![meeting_id, source_id, cursor, query_limit], |row| {
                    speaker_group_from_row(row)
                })
                .map_err(TranscriptError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TranscriptError::from)?
        }
        None => {
            let sql = format!(
                "SELECT {columns} FROM transcript_segments \
                 WHERE meeting_id = ?1 AND source_id = ?2 AND speaker_label IS NOT NULL \
                 GROUP BY speaker_label ORDER BY speaker_label LIMIT ?3"
            );
            let mut statement = connection.prepare(&sql).map_err(TranscriptError::from)?;
            statement
                .query_map(params![meeting_id, source_id, query_limit], |row| {
                    speaker_group_from_row(row)
                })
                .map_err(TranscriptError::from)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(TranscriptError::from)?
        }
    };
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = has_more
        .then(|| items.last().map(|group| group.speaker_label.clone()))
        .flatten();
    Ok(TranscriptSpeakerGroupPage {
        items,
        next_cursor,
        has_more,
        total_count,
    })
}

pub(crate) fn update_segment(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    segment_id: &str,
    speaker_name: Option<&str>,
    original_text: &str,
    expected_revision: u64,
) -> TranscriptResult<TranscriptSegment> {
    let expected_revision = sqlite_integer(expected_revision)?;
    let changed = transaction
        .execute(
            "
            UPDATE transcript_segments
            SET speaker_name = ?3,
                original_text = ?4,
                revision = revision + 1,
                corrected_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1 AND id = ?2 AND revision = ?5
            ",
            params![
                meeting_id,
                segment_id,
                speaker_name,
                original_text,
                expected_revision
            ],
        )
        .map_err(TranscriptError::from)?;

    if changed == 0 {
        return match get_segment(transaction, meeting_id, segment_id) {
            Ok(_) => Err(TranscriptError::RevisionConflict),
            Err(error) => Err(error),
        };
    }
    transaction
        .execute(
            "
            UPDATE transcript_sources
            SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = (
                SELECT source_id FROM transcript_segments WHERE meeting_id = ?1 AND id = ?2
            )
            ",
            params![meeting_id, segment_id],
        )
        .map_err(TranscriptError::from)?;
    get_segment(transaction, meeting_id, segment_id)
}

pub(crate) fn rename_speaker_in_source(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    source_id: &str,
    speaker_label: &str,
    speaker_name: Option<&str>,
) -> TranscriptResult<u64> {
    let changed = transaction
        .execute(
            "
            UPDATE transcript_segments
            SET speaker_name = ?4,
                revision = revision + 1,
                corrected_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE meeting_id = ?1
              AND source_id = ?2
              AND speaker_label = ?3
              AND speaker_name IS NOT ?4
            ",
            params![meeting_id, source_id, speaker_label, speaker_name],
        )
        .map_err(TranscriptError::from)?;
    if changed > 0 {
        transaction
            .execute(
                "UPDATE transcript_sources \
                 SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE meeting_id = ?1 AND id = ?2",
                params![meeting_id, source_id],
            )
            .map_err(TranscriptError::from)?;
    }
    u64::try_from(changed).map_err(|_| TranscriptError::PersistenceFailed)
}

pub(crate) fn delete_source(
    transaction: &Transaction<'_>,
    meeting_id: &str,
    source_id: &str,
) -> TranscriptResult<()> {
    let changed = transaction
        .execute(
            "DELETE FROM transcript_sources WHERE meeting_id = ?1 AND id = ?2",
            params![meeting_id, source_id],
        )
        .map_err(TranscriptError::from)?;
    if changed == 0 {
        return Err(TranscriptError::SourceNotFound);
    }
    Ok(())
}

pub(crate) fn build_summary_input(
    connection: &Connection,
    meeting_id: &str,
    source_ids: &[String],
) -> TranscriptResult<Vec<SummaryInputSegment>> {
    if source_ids.is_empty() {
        return Err(TranscriptError::InputInsufficient);
    }
    let mut unique_ids = HashSet::with_capacity(source_ids.len());
    if source_ids
        .iter()
        .any(|source_id| !unique_ids.insert(source_id.as_str()))
    {
        return Err(TranscriptError::InvalidInput);
    }

    let mut sources = Vec::with_capacity(source_ids.len());
    for source_id in source_ids {
        sources.push(get_source(connection, meeting_id, source_id)?);
    }
    sources.sort_by(|left, right| {
        left.imported_at
            .cmp(&right.imported_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut input = Vec::new();
    for source in sources {
        for segment in list_segments(connection, meeting_id, &source.id)? {
            if segment.is_final {
                input.push(SummaryInputSegment {
                    segment_id: segment.id,
                    source_id: segment.source_id,
                    ordinal: segment.ordinal,
                    speaker_label: segment.speaker_label,
                    speaker_name: segment.speaker_name,
                    start_offset_ms: segment.start_offset_ms,
                    end_offset_ms: segment.end_offset_ms,
                    original_text: segment.original_text,
                    translated_text: segment.translated_text,
                    language: segment.language,
                    confidence: segment.confidence,
                    is_final: segment.is_final,
                    revision: segment.revision,
                });
            }
        }
    }
    if input.is_empty() {
        return Err(TranscriptError::InputInsufficient);
    }
    Ok(input)
}

fn source_from_row(row: &Row<'_>) -> rusqlite::Result<TranscriptSource> {
    let source_type = row.get::<_, String>(2)?;
    let source_type = TranscriptSourceType::from_database(&source_type).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(TranscriptSource {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        source_type,
        display_name: row.get(3)?,
        original_filename: row.get(4)?,
        content_sha256: row.get(5)?,
        language: row.get(6)?,
        base_started_at: row.get(7)?,
        parser_name: row.get(8)?,
        parser_version: row.get(9)?,
        imported_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn source_overview_from_row(row: &Row<'_>) -> rusqlite::Result<TranscriptSourceOverview> {
    Ok(TranscriptSourceOverview {
        source: source_from_row(row)?,
        segment_count: nonnegative_integer(row, 13)?,
        start_offset_ms: optional_nonnegative_integer(row, 14)?,
        end_offset_ms: optional_nonnegative_integer(row, 15)?,
        speaker_count: nonnegative_integer(row, 16)?,
    })
}

fn speaker_group_from_row(row: &Row<'_>) -> rusqlite::Result<TranscriptSpeakerGroup> {
    let speaker_label = row.get(0)?;
    let minimum_name = row.get::<_, Option<String>>(1)?;
    let maximum_name = row.get::<_, Option<String>>(2)?;
    let null_name_count = nonnegative_integer(row, 3)?;
    let segment_count = nonnegative_integer(row, 4)?;
    let has_mixed_names =
        minimum_name != maximum_name || (null_name_count > 0 && null_name_count < segment_count);
    Ok(TranscriptSpeakerGroup {
        speaker_label,
        speaker_name: (!has_mixed_names).then_some(minimum_name).flatten(),
        has_mixed_names,
        segment_count,
    })
}

fn segment_from_row(row: &Row<'_>) -> rusqlite::Result<TranscriptSegment> {
    Ok(TranscriptSegment {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        source_id: row.get(2)?,
        ordinal: nonnegative_integer(row, 3)?,
        speaker_label: row.get(4)?,
        speaker_name: row.get(5)?,
        start_offset_ms: optional_nonnegative_integer(row, 6)?,
        end_offset_ms: optional_nonnegative_integer(row, 7)?,
        original_text: row.get(8)?,
        translated_text: row.get(9)?,
        language: row.get(10)?,
        confidence: row.get(11)?,
        is_final: row.get::<_, i64>(12)? == 1,
        revision: nonnegative_integer(row, 13)?,
        created_at: row.get(14)?,
        corrected_at: row.get(15)?,
    })
}

fn sqlite_integer(value: u64) -> TranscriptResult<i64> {
    i64::try_from(value).map_err(|_| TranscriptError::InvalidInput)
}

fn nonnegative_integer(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(index)?;
    u64::try_from(value).map_err(|_| integer_conversion_error(index))
}

fn optional_nonnegative_integer(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u64::try_from(value).map_err(|_| integer_conversion_error(index)))
        .transpose()
}

fn integer_conversion_error(index: usize) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Integer,
        Box::new(TranscriptError::PersistenceFailed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migration;

    const MEETING_A: &str = "00000000-0000-4000-8000-000000000101";
    const MEETING_B: &str = "00000000-0000-4000-8000-000000000102";
    const SOURCE_A: &str = "00000000-0000-4000-8000-000000000103";
    const SOURCE_B: &str = "00000000-0000-4000-8000-000000000104";
    const SEGMENT_A: &str = "00000000-0000-4000-8000-000000000105";
    const SEGMENT_B: &str = "00000000-0000-4000-8000-000000000106";

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().expect("database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should enable");
        migration::run(&mut connection).expect("schema should apply");
        connection
            .execute_batch(&format!(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at
                ) VALUES
                    (
                        '{MEETING_A}', 'Synthetic meeting A', 'draft', 'en-AU',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    ),
                    (
                        '{MEETING_B}', 'Synthetic meeting B', 'draft', 'en-AU',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    );
                "
            ))
            .expect("meetings should insert");
        connection
    }

    fn source(id: &str, meeting_id: &str, hash_byte: char) -> NewTranscriptSource {
        NewTranscriptSource {
            id: id.to_string(),
            meeting_id: meeting_id.to_string(),
            source_type: TranscriptSourceType::PastedText,
            display_name: "Synthetic transcript".to_string(),
            original_filename: None,
            content_sha256: hash_byte.to_string().repeat(64),
            language: Some("en-AU".to_string()),
            base_started_at: None,
            parser_name: "plain_text".to_string(),
            parser_version: "1".to_string(),
        }
    }

    fn segment(
        id: &str,
        meeting_id: &str,
        source_id: &str,
        ordinal: u64,
        is_final: bool,
    ) -> NewTranscriptSegment {
        NewTranscriptSegment {
            id: id.to_string(),
            meeting_id: meeting_id.to_string(),
            source_id: source_id.to_string(),
            ordinal,
            speaker_label: Some("Speaker 1".to_string()),
            speaker_name: None,
            start_offset_ms: None,
            end_offset_ms: None,
            original_text: format!("Synthetic segment {ordinal}"),
            translated_text: None,
            language: Some("en-AU".to_string()),
            confidence: None,
            is_final,
        }
    }

    #[test]
    fn repository_persists_updates_summarizes_and_cascades_sources() {
        let mut connection = database();
        let transaction = connection.transaction().expect("transaction should begin");
        insert_source(&transaction, &source(SOURCE_A, MEETING_A, 'a'))
            .expect("source should insert");
        insert_segments(
            &transaction,
            &[
                segment(SEGMENT_A, MEETING_A, SOURCE_A, 0, true),
                segment(SEGMENT_B, MEETING_A, SOURCE_A, 1, false),
            ],
        )
        .expect("segments should insert");
        transaction.commit().expect("transaction should commit");

        assert_eq!(
            find_source_by_hash(&connection, MEETING_A, &"a".repeat(64))
                .expect("hash lookup should work")
                .expect("source should exist")
                .id,
            SOURCE_A
        );
        assert_eq!(
            list_sources(&connection, MEETING_A)
                .expect("sources should list")
                .len(),
            1
        );
        assert_eq!(
            list_segments(&connection, MEETING_A, SOURCE_A)
                .expect("segments should list")
                .len(),
            2
        );
        let summary = build_summary_input(&connection, MEETING_A, &[SOURCE_A.to_string()])
            .expect("summary input should build");
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].segment_id, SEGMENT_A);

        let transaction = connection.transaction().expect("transaction should begin");
        let updated = update_segment(
            &transaction,
            MEETING_A,
            SEGMENT_A,
            Some("Confirmed Speaker"),
            "Corrected synthetic text",
            1,
        )
        .expect("segment should update");
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.speaker_name.as_deref(), Some("Confirmed Speaker"));
        assert!(updated.corrected_at.is_some());
        assert_eq!(
            update_segment(&transaction, MEETING_A, SEGMENT_A, None, "Stale update", 1,),
            Err(TranscriptError::RevisionConflict)
        );
        transaction.commit().expect("transaction should commit");

        let transaction = connection.transaction().expect("transaction should begin");
        delete_source(&transaction, MEETING_A, SOURCE_A).expect("source should delete");
        transaction.commit().expect("transaction should commit");
        assert_eq!(
            get_source(&connection, MEETING_A, SOURCE_A),
            Err(TranscriptError::SourceNotFound)
        );
        let segment_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM transcript_segments", [], |row| {
                row.get(0)
            })
            .expect("segments should count");
        assert_eq!(segment_count, 0);
    }

    #[test]
    fn repository_returns_stable_duplicate_ownership_and_input_errors() {
        let mut connection = database();
        let transaction = connection.transaction().expect("transaction should begin");
        insert_source(&transaction, &source(SOURCE_A, MEETING_A, 'b'))
            .expect("first source should insert");
        insert_segments(
            &transaction,
            &[segment(SEGMENT_A, MEETING_A, SOURCE_A, 0, false)],
        )
        .expect("segment should insert");
        assert_eq!(
            insert_source(&transaction, &source(SOURCE_B, MEETING_A, 'b')),
            Err(TranscriptError::AlreadyImported {
                source_id: SOURCE_A.to_string()
            })
        );
        transaction.commit().expect("transaction should commit");

        assert_eq!(
            build_summary_input(&connection, MEETING_B, &[SOURCE_A.to_string()]),
            Err(TranscriptError::SourceNotFound)
        );
        assert_eq!(
            build_summary_input(
                &connection,
                MEETING_A,
                &[SOURCE_A.to_string(), SOURCE_A.to_string()]
            ),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            build_summary_input(&connection, MEETING_A, &[SOURCE_A.to_string()]),
            Err(TranscriptError::InputInsufficient)
        );
    }

    #[test]
    fn repository_pages_segments_and_speaker_groups_and_renames_only_the_target_group() {
        let mut connection = database();
        let transaction = connection.transaction().expect("transaction should begin");
        insert_source(&transaction, &source(SOURCE_A, MEETING_A, 'c'))
            .expect("source A should insert");
        insert_source(&transaction, &source(SOURCE_B, MEETING_A, 'd'))
            .expect("source B should insert");
        let mut segments = (0..5)
            .map(|ordinal| {
                segment(
                    &format!("00000000-0000-4000-8000-{ordinal:012}"),
                    MEETING_A,
                    SOURCE_A,
                    ordinal,
                    true,
                )
            })
            .collect::<Vec<_>>();
        segments[0].speaker_label = Some("Speaker A".to_string());
        segments[0].speaker_name = Some("Old name".to_string());
        segments[0].start_offset_ms = Some(1_000);
        segments[0].end_offset_ms = Some(2_000);
        segments[1].speaker_label = Some("Speaker A".to_string());
        segments[1].speaker_name = None;
        segments[1].start_offset_ms = Some(2_000);
        segments[1].end_offset_ms = Some(3_000);
        segments[2].speaker_label = Some("Speaker B".to_string());
        segments[2].speaker_name = Some("Stable name".to_string());
        segments[2].start_offset_ms = Some(3_000);
        segments[2].end_offset_ms = Some(4_000);
        segments[3].speaker_label = Some("Speaker C".to_string());
        segments[4].speaker_label = Some("Speaker D".to_string());
        insert_segments(&transaction, &segments).expect("source A segments should insert");
        let mut other = segment(SEGMENT_B, MEETING_A, SOURCE_B, 0, true);
        other.speaker_label = Some("Speaker A".to_string());
        insert_segments(&transaction, &[other]).expect("source B segment should insert");
        transaction.commit().expect("transaction should commit");

        let overview =
            get_source_overview(&connection, MEETING_A, SOURCE_A).expect("overview should load");
        assert_eq!(overview.segment_count, 5);
        assert_eq!(overview.start_offset_ms, Some(1_000));
        assert_eq!(overview.end_offset_ms, Some(4_000));
        assert_eq!(overview.speaker_count, 4);

        let first = list_segment_page(&connection, MEETING_A, SOURCE_A, None, 2)
            .expect("first segment page should load");
        assert_eq!(first.total_count, 5);
        assert_eq!(
            first
                .items
                .iter()
                .map(|item| item.ordinal)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(first.has_more);
        assert_eq!(first.next_cursor, Some(1));
        let second = list_segment_page(&connection, MEETING_A, SOURCE_A, first.next_cursor, 2)
            .expect("second segment page should load");
        assert_eq!(
            second
                .items
                .iter()
                .map(|item| item.ordinal)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(second.has_more);
        let final_page = list_segment_page(&connection, MEETING_A, SOURCE_A, second.next_cursor, 2)
            .expect("final segment page should load");
        assert_eq!(final_page.items[0].ordinal, 4);
        assert!(!final_page.has_more);
        assert_eq!(final_page.next_cursor, None);

        let groups = list_speaker_group_page(&connection, MEETING_A, SOURCE_A, None, 2)
            .expect("first speaker page should load");
        assert_eq!(groups.total_count, 4);
        assert_eq!(groups.items.len(), 2);
        assert_eq!(groups.items[0].speaker_label, "Speaker A");
        assert_eq!(groups.items[0].segment_count, 2);
        assert!(groups.items[0].has_mixed_names);
        assert_eq!(groups.items[0].speaker_name, None);
        assert_eq!(groups.next_cursor.as_deref(), Some("Speaker B"));
        let remaining = list_speaker_group_page(
            &connection,
            MEETING_A,
            SOURCE_A,
            groups.next_cursor.as_deref(),
            2,
        )
        .expect("second speaker page should load");
        assert_eq!(
            remaining
                .items
                .iter()
                .map(|group| group.speaker_label.as_str())
                .collect::<Vec<_>>(),
            ["Speaker C", "Speaker D"]
        );
        assert!(!remaining.has_more);

        let transaction = connection.transaction().expect("rename should begin");
        assert_eq!(
            rename_speaker_in_source(
                &transaction,
                MEETING_A,
                SOURCE_A,
                "Speaker A",
                Some("Confirmed name")
            )
            .expect("speaker should rename"),
            2
        );
        transaction.commit().expect("rename should commit");
        let renamed =
            list_segments(&connection, MEETING_A, SOURCE_A).expect("renamed segments should list");
        for segment in &renamed[..2] {
            assert_eq!(segment.speaker_name.as_deref(), Some("Confirmed name"));
            assert_eq!(segment.revision, 2);
            assert!(segment.corrected_at.is_some());
        }
        assert_eq!(
            list_segments(&connection, MEETING_A, SOURCE_B).expect("other source should list")[0]
                .speaker_name,
            None
        );

        let transaction = connection
            .transaction()
            .expect("idempotent rename should begin");
        assert_eq!(
            rename_speaker_in_source(
                &transaction,
                MEETING_A,
                SOURCE_A,
                "Speaker A",
                Some("Confirmed name")
            )
            .expect("idempotent rename should succeed"),
            0
        );
        transaction
            .commit()
            .expect("idempotent rename should commit");

        let transaction = connection.transaction().expect("clear should begin");
        assert_eq!(
            rename_speaker_in_source(&transaction, MEETING_A, SOURCE_A, "Speaker A", None)
                .expect("speaker names should clear"),
            2
        );
        transaction.commit().expect("clear should commit");
        for segment in &list_segments(&connection, MEETING_A, SOURCE_A)
            .expect("cleared segments should list")[..2]
        {
            assert_eq!(segment.speaker_name, None);
            assert_eq!(segment.revision, 3);
        }
    }
}
