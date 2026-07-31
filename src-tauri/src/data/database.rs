use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use super::error::{DataError, DataResult};
use super::migration;
use super::paths::StoragePaths;

#[derive(Clone, Debug)]
pub(crate) struct Database {
    inner: Arc<DatabaseInner>,
}

#[derive(Debug)]
struct DatabaseInner {
    paths: StoragePaths,
    _storage_lock: File,
    mutation_lock: Mutex<()>,
}

impl Database {
    pub(crate) fn initialize(app_data_root: impl AsRef<Path>) -> DataResult<Self> {
        let paths = StoragePaths::initialize(app_data_root)?;
        let storage_lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(paths.storage_lock())
            .map_err(|_| DataError::StorageUnavailable)?;
        match storage_lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) | Err(TryLockError::Error(_)) => {
                return Err(DataError::StorageUnavailable);
            }
        }
        let database = Self {
            inner: Arc::new(DatabaseInner {
                paths,
                _storage_lock: storage_lock,
                mutation_lock: Mutex::new(()),
            }),
        };
        let mut connection = database.connect()?;
        migration::run(&mut connection)?;
        Ok(database)
    }

    pub(crate) fn connect(&self) -> DataResult<Connection> {
        let connection = Connection::open_with_flags(
            self.inner.paths.database(),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(DataError::from)?;

        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(DataError::from)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(DataError::from)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(DataError::from)?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(DataError::from)?;

        Ok(connection)
    }

    pub(crate) fn paths(&self) -> &StoragePaths {
        &self.inner.paths
    }

    pub(crate) fn mutation_guard(&self) -> DataResult<MutexGuard<'_, ()>> {
        self.inner
            .mutation_lock
            .lock()
            .map_err(|_| DataError::StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use uuid::Uuid;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("meetinglens-database-{}", Uuid::new_v4())))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn enables_foreign_keys_and_wal_for_every_connection() {
        let directory = TestDirectory::new();
        let database = Database::initialize(&directory.0).expect("database should initialize");
        let connection = database.connect().expect("database should connect");

        let foreign_keys: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .expect("foreign key setting should be readable");
        let journal_mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("journal mode should be readable");

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    }

    #[test]
    fn second_writer_for_the_same_storage_root_fails_closed() {
        let directory = TestDirectory::new();
        let first = Database::initialize(&directory.0).expect("first writer should initialize");

        assert!(matches!(
            Database::initialize(&directory.0),
            Err(DataError::StorageUnavailable)
        ));

        drop(first);
        Database::initialize(&directory.0).expect("lock should release with the owner");
    }
}
