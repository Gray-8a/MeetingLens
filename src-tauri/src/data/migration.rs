use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::error::{DataError, DataResult};

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: r#"
        CREATE TABLE meetings (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200),
            status TEXT NOT NULL CHECK(status IN (
                'draft', 'recording', 'processing', 'ready', 'failed', 'archived'
            )),
            primary_language TEXT NOT NULL CHECK(length(trim(primary_language)) BETWEEN 1 AND 35),
            selected_window_json TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            CHECK(ended_at IS NULL OR started_at IS NOT NULL),
            CHECK(ended_at IS NULL OR ended_at >= started_at)
        );

        CREATE INDEX idx_meetings_updated_at
            ON meetings(updated_at DESC, id);
        CREATE INDEX idx_meetings_status
            ON meetings(status, updated_at DESC);
    "#,
    },
    Migration {
        version: 2,
        sql: r#"
        ALTER TABLE meetings RENAME TO meetings_v1;

        CREATE TABLE meetings (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 200),
            status TEXT NOT NULL CHECK(status IN (
                'draft', 'recording', 'processing', 'ready', 'failed', 'archived',
                'deleting', 'delete_failed'
            )),
            primary_language TEXT NOT NULL CHECK(length(trim(primary_language)) BETWEEN 1 AND 35),
            selected_window_json TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            deletion_error_code TEXT,
            CHECK(ended_at IS NULL OR started_at IS NOT NULL),
            CHECK(ended_at IS NULL OR ended_at >= started_at),
            CHECK(status NOT IN ('deleting', 'delete_failed') OR deleted_at IS NOT NULL)
        );

        INSERT INTO meetings(
            id, title, status, primary_language, selected_window_json,
            started_at, ended_at, created_at, updated_at, deleted_at,
            deletion_error_code
        )
        SELECT
            id, title, status, primary_language, selected_window_json,
            started_at, ended_at, created_at, updated_at, deleted_at,
            NULL
        FROM meetings_v1;

        DROP TABLE meetings_v1;

        CREATE INDEX idx_meetings_updated_at
            ON meetings(updated_at DESC, id);
        CREATE INDEX idx_meetings_status
            ON meetings(status, updated_at DESC);

        CREATE TABLE documents (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            meeting_id TEXT NOT NULL,
            original_filename TEXT NOT NULL CHECK(length(original_filename) BETWEEN 1 AND 255),
            safe_extension TEXT NOT NULL CHECK(safe_extension IN (
                'pdf', 'docx', 'pptx', 'xlsx', 'txt', 'md', 'csv', 'png', 'jpg', 'jpeg'
            )),
            detected_media_type TEXT,
            size_bytes INTEGER CHECK(size_bytes IS NULL OR size_bytes >= 0),
            sha256 TEXT CHECK(
                sha256 IS NULL OR (
                    length(sha256) = 64
                    AND sha256 = lower(sha256)
                    AND sha256 NOT GLOB '*[^0-9a-f]*'
                )
            ),
            status TEXT NOT NULL CHECK(status IN (
                'importing', 'staged', 'parsing', 'parsed',
                'import_failed', 'parse_failed', 'deleting', 'delete_failed'
            )),
            storage_relative_path TEXT NOT NULL CHECK(length(storage_relative_path) BETWEEN 1 AND 1024),
            imported_at TEXT,
            updated_at TEXT NOT NULL,
            failure_code TEXT,
            failure_message_safe TEXT,
            parser_version_optional TEXT,
            page_count_optional INTEGER CHECK(page_count_optional IS NULL OR page_count_optional >= 0),
            created_at TEXT NOT NULL,
            FOREIGN KEY(meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_documents_meeting_status
            ON documents(meeting_id, status, updated_at DESC);
        CREATE UNIQUE INDEX idx_documents_meeting_sha256
            ON documents(meeting_id, sha256)
            WHERE sha256 IS NOT NULL;

        CREATE TABLE deletion_jobs (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            entity_kind TEXT NOT NULL CHECK(entity_kind IN ('meeting', 'document')),
            meeting_id TEXT NOT NULL,
            document_id TEXT,
            status TEXT NOT NULL CHECK(status IN ('pending', 'failed')),
            trash_relative_path TEXT,
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 3),
            error_code TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(
                (entity_kind = 'meeting' AND document_id IS NULL)
                OR (entity_kind = 'document' AND document_id IS NOT NULL)
            ),
            FOREIGN KEY(meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
            FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
        );

        CREATE UNIQUE INDEX idx_deletion_jobs_meeting
            ON deletion_jobs(meeting_id)
            WHERE entity_kind = 'meeting';
        CREATE UNIQUE INDEX idx_deletion_jobs_document
            ON deletion_jobs(document_id)
            WHERE entity_kind = 'document';
        CREATE INDEX idx_deletion_jobs_status
            ON deletion_jobs(status, updated_at, id);
    "#,
    },
    Migration {
        version: 3,
        sql: r#"
        CREATE TABLE transcript_sources (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            meeting_id TEXT NOT NULL,
            source_type TEXT NOT NULL CHECK(source_type IN (
                'pasted_text', 'imported_txt', 'imported_srt', 'imported_vtt',
                'live_asr', 'platform_export'
            )),
            display_name TEXT NOT NULL CHECK(
                length(trim(display_name)) > 0
                AND length(CAST(display_name AS BLOB)) <= 255
            ),
            original_filename TEXT CHECK(
                original_filename IS NULL OR (
                    length(original_filename) > 0
                    AND length(CAST(original_filename AS BLOB)) <= 255
                )
            ),
            content_sha256 TEXT NOT NULL CHECK(
                length(content_sha256) = 64
                AND content_sha256 = lower(content_sha256)
                AND content_sha256 NOT GLOB '*[^0-9a-f]*'
            ),
            language TEXT CHECK(
                language IS NULL OR length(trim(language)) BETWEEN 1 AND 35
            ),
            base_started_at TEXT,
            parser_name TEXT NOT NULL CHECK(
                length(trim(parser_name)) > 0
                AND length(CAST(parser_name AS BLOB)) <= 64
            ),
            parser_version TEXT NOT NULL CHECK(
                length(trim(parser_version)) > 0
                AND length(CAST(parser_version AS BLOB)) <= 64
            ),
            imported_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(id, meeting_id),
            FOREIGN KEY(meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_transcript_sources_meeting
            ON transcript_sources(meeting_id, imported_at, id);
        CREATE INDEX idx_transcript_sources_meeting_type
            ON transcript_sources(meeting_id, source_type, imported_at, id);
        CREATE UNIQUE INDEX idx_transcript_sources_meeting_sha256
            ON transcript_sources(meeting_id, content_sha256);

        CREATE TABLE transcript_segments (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            meeting_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 19999),
            speaker_label TEXT CHECK(
                speaker_label IS NULL OR (
                    length(trim(speaker_label)) > 0
                    AND length(CAST(speaker_label AS BLOB)) <= 255
                )
            ),
            speaker_name TEXT CHECK(
                speaker_name IS NULL OR (
                    length(trim(speaker_name)) > 0
                    AND length(CAST(speaker_name AS BLOB)) <= 255
                )
            ),
            start_offset_ms INTEGER CHECK(start_offset_ms IS NULL OR start_offset_ms >= 0),
            end_offset_ms INTEGER CHECK(
                end_offset_ms IS NULL OR (
                    start_offset_ms IS NOT NULL
                    AND end_offset_ms >= start_offset_ms
                )
            ),
            original_text TEXT NOT NULL CHECK(
                length(trim(original_text)) > 0
                AND length(CAST(original_text AS BLOB)) <= 65536
            ),
            translated_text TEXT CHECK(
                translated_text IS NULL OR (
                    length(trim(translated_text)) > 0
                    AND length(CAST(translated_text AS BLOB)) <= 65536
                )
            ),
            language TEXT CHECK(
                language IS NULL OR length(trim(language)) BETWEEN 1 AND 35
            ),
            confidence REAL CHECK(confidence IS NULL OR confidence BETWEEN 0.0 AND 1.0),
            is_final INTEGER NOT NULL CHECK(is_final IN (0, 1)),
            revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
            created_at TEXT NOT NULL,
            corrected_at TEXT,
            UNIQUE(source_id, ordinal),
            FOREIGN KEY(meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
            FOREIGN KEY(source_id, meeting_id)
                REFERENCES transcript_sources(id, meeting_id) ON DELETE CASCADE
        );

        CREATE INDEX idx_transcript_segments_meeting
            ON transcript_segments(meeting_id, source_id, ordinal);
    "#,
    },
    Migration {
        version: 4,
        sql: r#"
        CREATE TABLE summaries (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            meeting_id TEXT NOT NULL,
            series_id TEXT NOT NULL CHECK(length(series_id) = 36),
            previous_summary_id TEXT CHECK(
                previous_summary_id IS NULL OR length(previous_summary_id) = 36
            ),
            revision INTEGER NOT NULL CHECK(revision >= 1),
            run_purpose TEXT NOT NULL CHECK(run_purpose IN ('checkpoint', 'final')),
            trigger_kind TEXT NOT NULL CHECK(trigger_kind IN ('manual', 'interval', 'topic', 'final')),
            meeting_title_snapshot TEXT NOT NULL CHECK(
                length(trim(meeting_title_snapshot)) BETWEEN 1 AND 200
            ),
            meeting_primary_language_snapshot TEXT NOT NULL CHECK(
                length(trim(meeting_primary_language_snapshot)) BETWEEN 1 AND 35
            ),
            template TEXT NOT NULL CHECK(template IN ('detailed', 'concise', 'actions_decisions')),
            output_language TEXT NOT NULL CHECK(output_language IN ('zh', 'en', 'bilingual')),
            status TEXT NOT NULL CHECK(status IN (
                'queued', 'running', 'completed', 'failed', 'cancelled'
            )),
            provider TEXT NOT NULL CHECK(
                length(trim(provider)) > 0
                AND length(CAST(provider AS BLOB)) <= 64
            ),
            model TEXT NOT NULL CHECK(
                length(trim(model)) > 0
                AND length(CAST(model AS BLOB)) <= 128
            ),
            prompt_version TEXT NOT NULL CHECK(
                length(trim(prompt_version)) > 0
                AND length(CAST(prompt_version AS BLOB)) <= 64
            ),
            content_json TEXT CHECK(
                content_json IS NULL OR (
                    length(trim(content_json)) > 0
                    AND length(CAST(content_json AS BLOB)) <= 1048576
                )
            ),
            error_code TEXT CHECK(
                error_code IS NULL OR (
                    length(trim(error_code)) > 0
                    AND length(CAST(error_code AS BLOB)) <= 64
                )
            ),
            source_count INTEGER NOT NULL CHECK(source_count BETWEEN 1 AND 20000),
            input_segment_count INTEGER NOT NULL CHECK(input_segment_count BETWEEN 1 AND 20000),
            input_bytes INTEGER NOT NULL CHECK(input_bytes BETWEEN 1 AND 10485760),
            is_stale INTEGER NOT NULL DEFAULT 0 CHECK(is_stale IN (0, 1)),
            stale_reason TEXT CHECK(stale_reason IN ('segment_revised', 'source_deleted')),
            stale_at TEXT,
            is_user_edited INTEGER NOT NULL DEFAULT 0 CHECK(is_user_edited IN (0, 1)),
            snapshot_created_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            cancel_requested_at TEXT,
            updated_at TEXT NOT NULL,
            UNIQUE(id, meeting_id),
            UNIQUE(series_id, revision),
            CHECK(
                (revision = 1 AND previous_summary_id IS NULL)
                OR (revision > 1 AND previous_summary_id IS NOT NULL)
            ),
            CHECK(
                (is_stale = 0 AND stale_reason IS NULL AND stale_at IS NULL)
                OR (is_stale = 1 AND stale_reason IS NOT NULL AND stale_at IS NOT NULL)
            ),
            CHECK(
                (status = 'queued' AND content_json IS NULL AND error_code IS NULL
                    AND started_at IS NULL AND finished_at IS NULL AND cancel_requested_at IS NULL)
                OR (status = 'running' AND content_json IS NULL AND error_code IS NULL
                    AND started_at IS NOT NULL AND finished_at IS NULL)
                OR (status = 'completed' AND content_json IS NOT NULL AND error_code IS NULL
                    AND started_at IS NOT NULL AND finished_at IS NOT NULL)
                OR (status = 'failed' AND content_json IS NULL AND error_code IS NOT NULL
                    AND started_at IS NOT NULL AND finished_at IS NOT NULL)
                OR (status = 'cancelled' AND content_json IS NULL AND error_code IS NULL
                    AND finished_at IS NOT NULL AND cancel_requested_at IS NOT NULL)
            ),
            FOREIGN KEY(meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
        );

        CREATE INDEX idx_summaries_meeting_created
            ON summaries(meeting_id, created_at DESC, id);
        CREATE INDEX idx_summaries_status_updated
            ON summaries(status, updated_at, id);
        CREATE INDEX idx_summaries_meeting_stale
            ON summaries(meeting_id, stale_at, id)
            WHERE is_stale = 1;
        CREATE UNIQUE INDEX idx_summaries_series_active
            ON summaries(series_id)
            WHERE status IN ('queued', 'running');

        CREATE TABLE summary_sources (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            summary_id TEXT NOT NULL,
            meeting_id TEXT NOT NULL,
            source_id_snapshot TEXT NOT NULL CHECK(length(source_id_snapshot) = 36),
            segment_id_snapshot TEXT NOT NULL CHECK(length(segment_id_snapshot) = 36),
            segment_revision INTEGER NOT NULL CHECK(segment_revision >= 1),
            source_ordinal INTEGER NOT NULL CHECK(source_ordinal BETWEEN 0 AND 19999),
            segment_ordinal INTEGER NOT NULL CHECK(segment_ordinal BETWEEN 0 AND 19999),
            global_ordinal INTEGER NOT NULL CHECK(global_ordinal BETWEEN 0 AND 19999),
            source_type_snapshot TEXT NOT NULL CHECK(source_type_snapshot IN (
                'pasted_text', 'imported_txt', 'imported_srt', 'imported_vtt',
                'live_asr', 'platform_export'
            )),
            source_display_name_snapshot TEXT CHECK(
                source_display_name_snapshot IS NULL OR (
                    length(trim(source_display_name_snapshot)) > 0
                    AND length(CAST(source_display_name_snapshot AS BLOB)) <= 255
                )
            ),
            source_language_snapshot TEXT CHECK(
                source_language_snapshot IS NULL OR length(trim(source_language_snapshot)) BETWEEN 1 AND 35
            ),
            source_base_started_at_snapshot TEXT,
            speaker_label_snapshot TEXT CHECK(
                speaker_label_snapshot IS NULL OR (
                    length(trim(speaker_label_snapshot)) > 0
                    AND length(CAST(speaker_label_snapshot AS BLOB)) <= 255
                )
            ),
            speaker_name_snapshot TEXT CHECK(
                speaker_name_snapshot IS NULL OR (
                    length(trim(speaker_name_snapshot)) > 0
                    AND length(CAST(speaker_name_snapshot AS BLOB)) <= 255
                )
            ),
            start_offset_ms_snapshot INTEGER CHECK(
                start_offset_ms_snapshot IS NULL OR start_offset_ms_snapshot >= 0
            ),
            end_offset_ms_snapshot INTEGER CHECK(
                end_offset_ms_snapshot IS NULL OR (
                    start_offset_ms_snapshot IS NOT NULL
                    AND end_offset_ms_snapshot >= start_offset_ms_snapshot
                )
            ),
            original_text_snapshot TEXT CHECK(
                original_text_snapshot IS NULL OR (
                    length(trim(original_text_snapshot)) > 0
                    AND length(CAST(original_text_snapshot AS BLOB)) <= 65536
                )
            ),
            language_snapshot TEXT CHECK(
                language_snapshot IS NULL OR length(trim(language_snapshot)) BETWEEN 1 AND 35
            ),
            confidence_snapshot REAL CHECK(
                confidence_snapshot IS NULL OR confidence_snapshot BETWEEN 0.0 AND 1.0
            ),
            is_final_snapshot INTEGER NOT NULL CHECK(is_final_snapshot = 1),
            snapshot_at TEXT NOT NULL,
            source_deleted INTEGER NOT NULL DEFAULT 0 CHECK(source_deleted IN (0, 1)),
            redacted_at TEXT,
            UNIQUE(id, summary_id, meeting_id),
            UNIQUE(summary_id, segment_id_snapshot, segment_revision),
            UNIQUE(summary_id, global_ordinal),
            CHECK(
                (source_deleted = 0 AND source_display_name_snapshot IS NOT NULL
                    AND original_text_snapshot IS NOT NULL AND redacted_at IS NULL)
                OR (source_deleted = 1 AND original_text_snapshot IS NULL AND redacted_at IS NOT NULL)
            ),
            FOREIGN KEY(summary_id, meeting_id)
                REFERENCES summaries(id, meeting_id) ON DELETE CASCADE
        );

        CREATE INDEX idx_summary_sources_live_source
            ON summary_sources(meeting_id, source_id_snapshot, summary_id);
        CREATE INDEX idx_summary_sources_live_segment
            ON summary_sources(meeting_id, segment_id_snapshot, segment_revision, summary_id);

        CREATE TABLE evidence_items (
            id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
            summary_id TEXT NOT NULL,
            meeting_id TEXT NOT NULL,
            summary_source_id TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind = 'transcript'),
            source_id_snapshot TEXT NOT NULL CHECK(length(source_id_snapshot) = 36),
            transcript_segment_id_snapshot TEXT NOT NULL CHECK(
                length(transcript_segment_id_snapshot) = 36
            ),
            segment_revision INTEGER NOT NULL CHECK(segment_revision >= 1),
            label TEXT NOT NULL CHECK(
                length(trim(label)) > 0 AND length(CAST(label AS BLOB)) <= 255
            ),
            speaker_snapshot TEXT CHECK(
                speaker_snapshot IS NULL OR (
                    length(trim(speaker_snapshot)) > 0
                    AND length(CAST(speaker_snapshot AS BLOB)) <= 255
                )
            ),
            start_offset_ms INTEGER CHECK(start_offset_ms IS NULL OR start_offset_ms >= 0),
            end_offset_ms INTEGER CHECK(
                end_offset_ms IS NULL OR (
                    start_offset_ms IS NOT NULL AND end_offset_ms >= start_offset_ms
                )
            ),
            ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 0 AND 19999),
            global_ordinal INTEGER NOT NULL CHECK(global_ordinal BETWEEN 0 AND 19999),
            excerpt TEXT NOT NULL CHECK(
                length(trim(excerpt)) > 0
                AND length(CAST(excerpt AS BLOB)) <= 1024
            ),
            source_deleted INTEGER NOT NULL DEFAULT 0 CHECK(source_deleted IN (0, 1)),
            created_at TEXT NOT NULL,
            UNIQUE(summary_id, summary_source_id),
            FOREIGN KEY(summary_source_id, summary_id, meeting_id)
                REFERENCES summary_sources(id, summary_id, meeting_id),
            FOREIGN KEY(summary_id, meeting_id)
                REFERENCES summaries(id, meeting_id) ON DELETE CASCADE
        );

        CREATE INDEX idx_evidence_items_summary_order
            ON evidence_items(summary_id, global_ordinal, id);
        CREATE INDEX idx_evidence_items_source
            ON evidence_items(meeting_id, source_id_snapshot, summary_id);
    "#,
    },
];

pub(crate) fn run(connection: &mut Connection) -> DataResult<()> {
    apply(connection, MIGRATIONS)
}

#[cfg(debug_assertions)]
pub(crate) fn run_through_v2(connection: &mut Connection) -> DataResult<()> {
    apply(connection, &MIGRATIONS[..2])
}

#[cfg(debug_assertions)]
pub(crate) fn run_through_v3(connection: &mut Connection) -> DataResult<()> {
    apply(connection, &MIGRATIONS[..3])
}

fn apply(connection: &mut Connection, migrations: &[Migration]) -> DataResult<()> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(DataError::from)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY NOT NULL CHECK(version > 0),
                applied_at TEXT NOT NULL,
                checksum TEXT NOT NULL
            );
            ",
        )
        .map_err(DataError::from)?;

    for migration in migrations {
        let expected_checksum = checksum(migration.sql);
        let recorded_checksum = transaction
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(DataError::from)?;

        match recorded_checksum {
            Some(recorded) if recorded == expected_checksum => continue,
            Some(_) => return Err(DataError::MigrationConflict),
            None => {}
        }

        transaction
            .execute_batch(migration.sql)
            .map_err(DataError::from)?;
        transaction
            .execute(
                "
                INSERT INTO schema_migrations(version, applied_at, checksum)
                VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?2)
                ",
                params![migration.version, expected_checksum],
            )
            .map_err(DataError::from)?;
    }

    transaction.commit().map_err(DataError::from)
}

fn checksum(sql: &str) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let value = sql.as_bytes().iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    });

    format!("fnv1a64:{value:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reapplying_the_same_migration_is_idempotent() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");

        apply(&mut connection, MIGRATIONS).expect("first migration run should succeed");
        apply(&mut connection, MIGRATIONS).expect("second migration run should succeed");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count should be readable");
        assert_eq!(count, 4);
    }

    #[test]
    fn rejects_a_changed_migration_checksum() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        apply(&mut connection, MIGRATIONS).expect("migration should succeed");
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'changed' WHERE version = 3",
                [],
            )
            .expect("test should alter the checksum");

        assert_eq!(
            apply(&mut connection, MIGRATIONS),
            Err(DataError::MigrationConflict)
        );
    }

    #[test]
    fn rolls_back_the_entire_batch_when_a_migration_fails() {
        let migrations = [Migration {
            version: 1,
            sql: "CREATE TABLE transient_table(id INTEGER); INVALID SQL;",
        }];
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");

        assert_eq!(
            apply(&mut connection, &migrations),
            Err(DataError::Database)
        );

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('schema_migrations', 'transient_table')",
                [],
                |row| row.get(0),
            )
            .expect("schema should be inspectable");
        assert_eq!(table_count, 0);
    }

    #[test]
    fn upgrades_v1_meeting_data_without_loss_and_enables_document_tables() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        apply(&mut connection, &MIGRATIONS[..1]).expect("v1 should apply");
        connection
            .execute_batch(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, started_at, ended_at,
                    created_at, updated_at
                ) VALUES
                    (
                        '00000000-0000-4000-8000-000000000001',
                        'Synthetic legacy draft', 'draft', 'en-AU', NULL, NULL,
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    ),
                    (
                        '00000000-0000-4000-8000-000000000002',
                        'Synthetic legacy recording', 'recording', 'en-AU',
                        '2026-01-01T00:00:01.000Z', NULL,
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:01.000Z'
                    ),
                    (
                        '00000000-0000-4000-8000-000000000003',
                        'Synthetic legacy ready', 'ready', 'en-AU',
                        '2026-01-01T00:00:01.000Z', '2026-01-01T00:00:02.000Z',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:02.000Z'
                    );
                ",
            )
            .expect("legacy meetings should insert");

        apply(&mut connection, MIGRATIONS).expect("v2 should apply");

        let meetings: Vec<(String, String, Option<String>)> = {
            let mut statement = connection
                .prepare("SELECT title, status, deletion_error_code FROM meetings ORDER BY id")
                .expect("migrated meetings should prepare");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .expect("migrated meetings should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("migrated meetings should read")
        };
        assert_eq!(
            meetings,
            vec![
                (
                    "Synthetic legacy draft".to_string(),
                    "draft".to_string(),
                    None,
                ),
                (
                    "Synthetic legacy recording".to_string(),
                    "recording".to_string(),
                    None,
                ),
                (
                    "Synthetic legacy ready".to_string(),
                    "ready".to_string(),
                    None,
                ),
            ]
        );
        let status_count: i64 = connection
            .query_row("SELECT COUNT(DISTINCT status) FROM meetings", [], |row| {
                row.get(0)
            })
            .expect("migrated statuses should read");
        assert_eq!(status_count, 3);
        for table in ["documents", "deletion_jobs"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("schema should be readable");
            assert_eq!(exists, 1);
        }
    }

    #[test]
    fn creates_document_foreign_keys_indexes_and_new_meeting_states() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should enable");
        apply(&mut connection, MIGRATIONS).expect("schema should apply");
        connection
            .execute(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at, deleted_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000010',
                    'Synthetic deleting meeting', 'deleting', 'en-AU',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z',
                    '2026-01-01T00:00:00.000Z'
                )
                ",
                [],
            )
            .expect("new meeting status should be legal with a tombstone");
        assert!(
            connection
                .execute(
                    "
                    INSERT INTO documents(
                        id, meeting_id, original_filename, safe_extension,
                        status, storage_relative_path, created_at, updated_at
                    ) VALUES (
                        '00000000-0000-4000-8000-000000000011',
                        '00000000-0000-4000-8000-999999999999',
                        'synthetic.txt', 'txt', 'importing',
                        'meeting-data/staging/00000000-0000-4000-8000-000000000011.tmp',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    )
                    ",
                    [],
                )
                .is_err()
        );
        let index_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*) FROM sqlite_master
                WHERE type = 'index' AND name IN (
                    'idx_documents_meeting_status',
                    'idx_documents_meeting_sha256',
                    'idx_deletion_jobs_meeting',
                    'idx_deletion_jobs_document',
                    'idx_deletion_jobs_status'
                )
                ",
                [],
                |row| row.get(0),
            )
            .expect("indexes should inspect");
        assert_eq!(index_count, 5);
        let foreign_keys: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('documents') WHERE \"table\" = 'meetings'",
                [],
                |row| row.get(0),
            )
            .expect("foreign keys should inspect");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn upgrades_v2_data_without_loss_and_records_schema_v3() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should enable");
        apply(&mut connection, &MIGRATIONS[..2]).expect("v2 should apply");
        connection
            .execute_batch(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000020',
                    'Synthetic v2 meeting', 'draft', 'en-AU',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                );

                INSERT INTO documents(
                    id, meeting_id, original_filename, safe_extension,
                    size_bytes, sha256, status, storage_relative_path,
                    imported_at, updated_at, created_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000021',
                    '00000000-0000-4000-8000-000000000020',
                    'synthetic.txt', 'txt', 9,
                    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                    'staged',
                    'meeting-data/meetings/00000000-0000-4000-8000-000000000020/documents/00000000-0000-4000-8000-000000000021.txt',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z',
                    '2026-01-01T00:00:00.000Z'
                );

                INSERT INTO deletion_jobs(
                    id, entity_kind, meeting_id, document_id, status,
                    attempt_count, created_at, updated_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000022', 'document',
                    '00000000-0000-4000-8000-000000000020',
                    '00000000-0000-4000-8000-000000000021', 'pending', 0,
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                );
                ",
            )
            .expect("v2 data should insert");

        apply(&mut connection, MIGRATIONS).expect("current schema should apply");

        for table in ["meetings", "documents", "deletion_jobs"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("legacy rows should remain readable");
            assert_eq!(count, 1, "{table} should keep its v2 row");
        }
        for table in ["transcript_sources", "transcript_segments"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("v3 table should be readable");
            assert_eq!(count, 0);
        }
        for table in ["summaries", "summary_sources", "evidence_items"] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("v4 table should be readable");
            assert_eq!(count, 0);
        }
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration records should be readable");
        assert_eq!(migration_count, 4);
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check should run");
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn upgrades_real_schema_v3_without_losing_transcript_data() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should enable");
        apply(&mut connection, &MIGRATIONS[..3]).expect("v3 should apply");
        connection
            .execute_batch(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000060',
                    'Synthetic v3 summary upgrade', 'draft', 'en-AU',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                );
                INSERT INTO transcript_sources(
                    id, meeting_id, source_type, display_name, content_sha256,
                    parser_name, parser_version, imported_at, created_at, updated_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000061',
                    '00000000-0000-4000-8000-000000000060',
                    'pasted_text', 'Synthetic source',
                    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                    'plain_text', '1', '2026-01-01T00:00:00.000Z',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                );
                INSERT INTO transcript_segments(
                    id, meeting_id, source_id, ordinal, original_text,
                    is_final, revision, created_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000062',
                    '00000000-0000-4000-8000-000000000060',
                    '00000000-0000-4000-8000-000000000061', 0,
                    'Synthetic legacy v3 transcript', 1, 1,
                    '2026-01-01T00:00:00.000Z'
                );
                ",
            )
            .expect("v3 transcript should insert");

        apply(&mut connection, MIGRATIONS).expect("v4 should apply");
        let transcript_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM transcript_segments", [], |row| {
                row.get(0)
            })
            .expect("transcript count should read");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count should read");
        let summary_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
                 AND name IN ('summaries', 'summary_sources', 'evidence_items')",
                [],
                |row| row.get(0),
            )
            .expect("summary tables should inspect");
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity should check");
        let foreign_key_violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("foreign keys should check");
        assert_eq!(transcript_count, 1);
        assert_eq!(migration_count, 4);
        assert_eq!(summary_table_count, 3);
        assert_eq!(integrity, "ok");
        assert_eq!(foreign_key_violations, 0);
    }

    #[test]
    fn rolls_back_schema_v3_without_touching_existing_v2_data() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        apply(&mut connection, &MIGRATIONS[..2]).expect("v2 should apply");
        connection
            .execute_batch(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at
                ) VALUES (
                    '00000000-0000-4000-8000-000000000030',
                    'Synthetic rollback meeting', 'draft', 'en-AU',
                    '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                );
                ",
            )
            .expect("v2 meeting should insert");
        let failing_v3 = [Migration {
            version: 3,
            sql: "CREATE TABLE transient_transcript(id INTEGER); INVALID SQL;",
        }];

        assert_eq!(
            apply(&mut connection, &failing_v3),
            Err(DataError::Database)
        );

        let meeting_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
            .expect("v2 meeting should remain readable");
        let transient_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transient_transcript'",
                [],
                |row| row.get(0),
            )
            .expect("schema should be inspectable");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration records should be readable");
        assert_eq!(meeting_count, 1);
        assert_eq!(transient_count, 0);
        assert_eq!(migration_count, 2);
    }

    #[test]
    fn schema_v3_enforces_transcript_ownership_order_hash_and_cascade_constraints() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys should enable");
        apply(&mut connection, MIGRATIONS).expect("schema should apply");
        connection
            .execute_batch(
                "
                INSERT INTO meetings(
                    id, title, status, primary_language, created_at, updated_at
                ) VALUES
                    (
                        '00000000-0000-4000-8000-000000000040',
                        'Synthetic transcript meeting A', 'draft', 'en-AU',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    ),
                    (
                        '00000000-0000-4000-8000-000000000041',
                        'Synthetic transcript meeting B', 'draft', 'en-AU',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    );

                INSERT INTO transcript_sources(
                    id, meeting_id, source_type, display_name, content_sha256,
                    parser_name, parser_version, imported_at, created_at, updated_at
                ) VALUES
                    (
                        '00000000-0000-4000-8000-000000000042',
                        '00000000-0000-4000-8000-000000000040',
                        'pasted_text', 'Synthetic source A',
                        'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                        'plain_text', '1', '2026-01-01T00:00:00.000Z',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    ),
                    (
                        '00000000-0000-4000-8000-000000000043',
                        '00000000-0000-4000-8000-000000000041',
                        'imported_txt', 'Synthetic source B',
                        'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                        'plain_text', '1', '2026-01-01T00:00:00.000Z',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    );

                INSERT INTO transcript_segments(
                    id, meeting_id, source_id, ordinal, speaker_label,
                    start_offset_ms, end_offset_ms, original_text,
                    is_final, revision, created_at
                ) VALUES
                    (
                        '00000000-0000-4000-8000-000000000044',
                        '00000000-0000-4000-8000-000000000040',
                        '00000000-0000-4000-8000-000000000042', 0, 'Speaker 1',
                        0, 1000, 'Synthetic transcript text', 1, 1,
                        '2026-01-01T00:00:00.000Z'
                    ),
                    (
                        '00000000-0000-4000-8000-000000000049',
                        '00000000-0000-4000-8000-000000000040',
                        '00000000-0000-4000-8000-000000000042', 1, NULL,
                        1000, 1000, 'Synthetic zero-duration cue', 1, 1,
                        '2026-01-01T00:00:00.000Z'
                    );
                ",
            )
            .expect("valid transcript data should insert");

        assert!(
            connection
                .execute(
                    "
                    INSERT INTO transcript_sources(
                        id, meeting_id, source_type, display_name, content_sha256,
                        parser_name, parser_version, imported_at, created_at, updated_at
                    ) VALUES (
                        '00000000-0000-4000-8000-000000000045',
                        '00000000-0000-4000-8000-000000000040',
                        'imported_txt', 'Duplicate hash',
                        'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                        'plain_text', '1', '2026-01-01T00:00:00.000Z',
                        '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z'
                    )
                    ",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "
                    INSERT INTO transcript_segments(
                        id, meeting_id, source_id, ordinal, original_text,
                        is_final, revision, created_at
                    ) VALUES (
                        '00000000-0000-4000-8000-000000000046',
                        '00000000-0000-4000-8000-000000000041',
                        '00000000-0000-4000-8000-000000000042', 1,
                        'Cross-meeting text', 1, 1,
                        '2026-01-01T00:00:00.000Z'
                    )
                    ",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "
                    INSERT INTO transcript_segments(
                        id, meeting_id, source_id, ordinal, start_offset_ms,
                        end_offset_ms, original_text, is_final, revision, created_at
                    ) VALUES (
                        '00000000-0000-4000-8000-000000000047',
                        '00000000-0000-4000-8000-000000000040',
                        '00000000-0000-4000-8000-000000000042', 2, 1000, 999,
                        'Invalid offset text', 1, 1,
                        '2026-01-01T00:00:00.000Z'
                    )
                    ",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "
                    INSERT INTO transcript_segments(
                        id, meeting_id, source_id, ordinal, original_text,
                        is_final, revision, created_at
                    ) VALUES (
                        '00000000-0000-4000-8000-000000000048',
                        '00000000-0000-4000-8000-000000000040',
                        '00000000-0000-4000-8000-000000000042', 0,
                        'Duplicate ordinal text', 1, 1,
                        '2026-01-01T00:00:00.000Z'
                    )
                    ",
                    [],
                )
                .is_err()
        );

        let index_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*) FROM sqlite_master
                WHERE type = 'index' AND name IN (
                    'idx_transcript_sources_meeting',
                    'idx_transcript_sources_meeting_type',
                    'idx_transcript_sources_meeting_sha256',
                    'idx_transcript_segments_meeting'
                )
                ",
                [],
                |row| row.get(0),
            )
            .expect("transcript indexes should inspect");
        assert_eq!(index_count, 4);

        connection
            .execute(
                "DELETE FROM transcript_sources WHERE id = '00000000-0000-4000-8000-000000000042'",
                [],
            )
            .expect("source should delete");
        let segment_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM transcript_segments", [], |row| {
                row.get(0)
            })
            .expect("segments should be readable");
        let source_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM transcript_sources", [], |row| {
                row.get(0)
            })
            .expect("sources should be readable");
        assert_eq!(segment_count, 0);
        assert_eq!(source_count, 1);

        connection
            .execute(
                "DELETE FROM meetings WHERE id = '00000000-0000-4000-8000-000000000041'",
                [],
            )
            .expect("meeting should delete");
        let source_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM transcript_sources", [], |row| {
                row.get(0)
            })
            .expect("sources should be readable");
        assert_eq!(source_count, 0);
    }
}
