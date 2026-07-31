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
];

pub(crate) fn run(connection: &mut Connection) -> DataResult<()> {
    apply(connection, MIGRATIONS)
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
        assert_eq!(count, 2);
    }

    #[test]
    fn rejects_a_changed_migration_checksum() {
        let mut connection = Connection::open_in_memory().expect("in-memory database should open");
        apply(&mut connection, MIGRATIONS).expect("migration should succeed");
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'changed' WHERE version = 1",
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
}
