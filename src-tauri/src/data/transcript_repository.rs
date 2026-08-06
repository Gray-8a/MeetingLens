use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use super::transcript_error::{TranscriptError, TranscriptResult};
use super::transcript_model::{
    NewTranscriptSegment, NewTranscriptSource, SummaryInputSegment, TranscriptSegment,
    TranscriptSource, TranscriptSourceType,
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
}
