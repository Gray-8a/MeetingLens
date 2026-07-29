use std::path::Path;

use rusqlite::TransactionBehavior;
use uuid::Uuid;

use super::database::Database;
use super::error::{DataError, DataResult};
use super::model::{CreateMeeting, Meeting, UpdateMeeting};
use super::paths::StoragePaths;
use super::repository::{self, NewMeeting};

const MAX_TITLE_CHARS: usize = 200;
const MAX_LANGUAGE_CHARS: usize = 35;

#[derive(Clone, Debug)]
pub struct MeetingService {
    database: Database,
}

impl MeetingService {
    pub fn initialize(app_data_root: impl AsRef<Path>) -> DataResult<Self> {
        Ok(Self {
            database: Database::initialize(app_data_root)?,
        })
    }

    pub fn storage_paths(&self) -> &StoragePaths {
        self.database.paths()
    }

    pub fn create(&self, input: CreateMeeting) -> DataResult<Meeting> {
        let title = normalize_required(&input.title, MAX_TITLE_CHARS)?;
        let primary_language = normalize_required(&input.primary_language, MAX_LANGUAGE_CHARS)?;
        let id = Uuid::new_v4().to_string();
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;

        repository::insert(
            &transaction,
            NewMeeting {
                id: &id,
                title: &title,
                primary_language: &primary_language,
            },
        )?;
        let meeting = repository::get(&transaction, &id)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub fn get(&self, id: &str) -> DataResult<Meeting> {
        validate_id(id)?;
        repository::get(&self.database.connect()?, id)
    }

    pub fn list(&self) -> DataResult<Vec<Meeting>> {
        repository::list(&self.database.connect()?)
    }

    pub fn update(&self, id: &str, changes: UpdateMeeting) -> DataResult<Meeting> {
        validate_id(id)?;
        if changes == UpdateMeeting::default() {
            return Err(DataError::InvalidInput);
        }

        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let current = repository::get(&transaction, id)?;
        let title = changes
            .title
            .as_deref()
            .map(|value| normalize_required(value, MAX_TITLE_CHARS))
            .transpose()?
            .unwrap_or(current.title);
        let primary_language = changes
            .primary_language
            .as_deref()
            .map(|value| normalize_required(value, MAX_LANGUAGE_CHARS))
            .transpose()?
            .unwrap_or(current.primary_language);
        let status = changes.status.unwrap_or(current.status);

        repository::update(&transaction, id, &title, status, &primary_language)?;
        let meeting = repository::get(&transaction, id)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub fn delete(&self, id: &str) -> DataResult<()> {
        validate_id(id)?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        repository::delete(&transaction, id)?;
        transaction.commit().map_err(DataError::from)
    }
}

fn normalize_required(value: &str, max_chars: usize) -> DataResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        return Err(DataError::InvalidInput);
    }
    Ok(value.to_string())
}

fn validate_id(id: &str) -> DataResult<()> {
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| DataError::InvalidInput)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::params;

    use super::*;
    use crate::data::model::MeetingStatus;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("meetinglens-data-core-{}", Uuid::new_v4())))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_input(title: &str) -> CreateMeeting {
        CreateMeeting {
            title: title.to_string(),
            primary_language: "en-AU".to_string(),
        }
    }

    #[test]
    fn creates_reads_updates_lists_and_deletes_a_meeting() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");

        let created = service
            .create(create_input("Synthetic planning meeting"))
            .expect("meeting should be created");
        assert!(Uuid::parse_str(&created.id).is_ok());
        assert_eq!(created.status, MeetingStatus::Draft);
        assert!(created.created_at.ends_with('Z'));
        assert_eq!(
            service.get(&created.id).expect("meeting should exist"),
            created
        );

        let updated = service
            .update(
                &created.id,
                UpdateMeeting {
                    title: Some("Synthetic project review".to_string()),
                    status: Some(MeetingStatus::Ready),
                    primary_language: Some("zh-CN".to_string()),
                },
            )
            .expect("meeting should update");
        assert_eq!(updated.title, "Synthetic project review");
        assert_eq!(updated.status, MeetingStatus::Ready);
        assert_eq!(updated.primary_language, "zh-CN");
        assert_eq!(service.list().expect("meetings should list"), vec![updated]);

        service.delete(&created.id).expect("meeting should delete");
        assert_eq!(service.get(&created.id), Err(DataError::NotFound));
        assert!(service.list().expect("meeting list should load").is_empty());
    }

    #[test]
    fn persists_across_service_restarts() {
        let directory = TestDirectory::new();
        let meeting_id = {
            let service =
                MeetingService::initialize(&directory.0).expect("service should initialize");
            service
                .create(create_input("Synthetic persistence check"))
                .expect("meeting should be created")
                .id
        };

        let reopened = MeetingService::initialize(&directory.0).expect("service should reopen");
        assert_eq!(
            reopened
                .get(&meeting_id)
                .expect("meeting should persist")
                .title,
            "Synthetic persistence check"
        );
        assert!(reopened.storage_paths().database().exists());
    }

    #[test]
    fn delete_rolls_back_if_the_final_delete_fails() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = service
            .create(create_input("Synthetic rollback check"))
            .expect("meeting should be created");
        let connection = service.database.connect().expect("database should connect");
        connection
            .execute_batch(
                "
                CREATE TRIGGER synthetic_delete_failure
                BEFORE DELETE ON meetings
                BEGIN
                    SELECT RAISE(ABORT, 'synthetic failure');
                END;
                ",
            )
            .expect("failure trigger should be installed");
        drop(connection);

        assert_eq!(service.delete(&meeting.id), Err(DataError::Database));
        assert_eq!(
            service
                .get(&meeting.id)
                .expect("rolled-back meeting should remain")
                .title,
            "Synthetic rollback check"
        );

        let connection = service.database.connect().expect("database should connect");
        let deleted_at: Option<String> = connection
            .query_row(
                "SELECT deleted_at FROM meetings WHERE id = ?1",
                params![meeting.id],
                |row| row.get(0),
            )
            .expect("deleted marker should be readable");
        assert_eq!(deleted_at, None);
    }

    #[test]
    fn returns_stable_errors_for_missing_or_invalid_meetings() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");
        let missing_id = Uuid::new_v4().to_string();

        assert_eq!(service.get(&missing_id), Err(DataError::NotFound));
        assert_eq!(service.delete(&missing_id), Err(DataError::NotFound));
        assert_eq!(service.get("not-a-uuid"), Err(DataError::InvalidInput));
        assert_eq!(
            service.update(
                &missing_id,
                UpdateMeeting {
                    title: Some("Synthetic missing meeting".to_string()),
                    ..UpdateMeeting::default()
                }
            ),
            Err(DataError::NotFound)
        );
        assert_eq!(
            service.update(&missing_id, UpdateMeeting::default()),
            Err(DataError::InvalidInput)
        );
    }

    #[test]
    fn rejects_empty_oversized_and_control_character_fields() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");

        assert_eq!(
            service.create(create_input("   ")),
            Err(DataError::InvalidInput)
        );
        assert_eq!(
            service.create(create_input(&"x".repeat(MAX_TITLE_CHARS + 1))),
            Err(DataError::InvalidInput)
        );
        assert_eq!(
            service.create(create_input("Synthetic\nmeeting")),
            Err(DataError::InvalidInput)
        );
    }
}
