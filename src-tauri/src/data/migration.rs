use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::error::{DataError, DataResult};

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
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
}];

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
        assert_eq!(count, 1);
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
}
