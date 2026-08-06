use std::path::Path;

use rusqlite::{Transaction, TransactionBehavior};
use uuid::Uuid;

use super::database::Database;
use super::document_service::{DocumentService, execute_meeting_delete};
use super::error::{DataError, DataResult};
use super::model::{CreateMeeting, Meeting, MeetingStatus, UpdateMeeting};
use super::paths::StoragePaths;
use super::repository::{self, NewMeeting};
use super::transcript_service::TranscriptService;
use super::transcript_workflow::TranscriptImportWorkflow;

const MAX_TITLE_CHARS: usize = 200;
const MAX_LANGUAGE_CHARS: usize = 35;

#[derive(Clone, Debug)]
pub struct MeetingService {
    database: Database,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedMeeting {
    id: String,
    title: String,
    primary_language: String,
}

impl MeetingService {
    pub fn initialize(app_data_root: impl AsRef<Path>) -> DataResult<Self> {
        let service = Self {
            database: Database::initialize(app_data_root)?,
        };
        DocumentService::new(service.database.clone())
            .reconcile()
            .map_err(|_| DataError::StorageUnavailable)?;
        Ok(service)
    }

    pub fn storage_paths(&self) -> &StoragePaths {
        self.database.paths()
    }

    pub fn documents(&self) -> DocumentService {
        DocumentService::new(self.database.clone())
    }

    pub fn transcripts(&self) -> TranscriptService {
        TranscriptService::new(self.database.clone())
    }

    pub fn transcript_imports(&self) -> TranscriptImportWorkflow {
        TranscriptImportWorkflow::new(self.database.clone())
    }

    pub fn create(&self, input: CreateMeeting) -> DataResult<Meeting> {
        let prepared = Self::prepare_create(input)?;
        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let meeting = Self::insert_prepared(&transaction, &prepared)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub(crate) fn prepare_create(input: CreateMeeting) -> DataResult<PreparedMeeting> {
        Ok(PreparedMeeting {
            id: Uuid::new_v4().to_string(),
            title: normalize_required(&input.title, MAX_TITLE_CHARS)?,
            primary_language: normalize_required(&input.primary_language, MAX_LANGUAGE_CHARS)?,
        })
    }

    pub(crate) fn insert_prepared(
        transaction: &Transaction<'_>,
        prepared: &PreparedMeeting,
    ) -> DataResult<Meeting> {
        repository::insert(
            transaction,
            NewMeeting {
                id: &prepared.id,
                title: &prepared.title,
                primary_language: &prepared.primary_language,
            },
        )?;
        repository::get(transaction, &prepared.id)
    }

    pub fn get(&self, id: &str) -> DataResult<Meeting> {
        validate_id(id)?;
        repository::get(&self.database.connect()?, id)
    }

    pub fn get_with_deletion_state(&self, id: &str) -> DataResult<Meeting> {
        validate_id(id)?;
        repository::get_including_deleted(&self.database.connect()?, id)
    }

    pub fn list(&self) -> DataResult<Vec<Meeting>> {
        repository::list(&self.database.connect()?)
    }

    pub fn list_with_deletion_state(&self) -> DataResult<Vec<Meeting>> {
        repository::list_including_deleted(&self.database.connect()?)
    }

    pub fn update(&self, id: &str, changes: UpdateMeeting) -> DataResult<Meeting> {
        validate_id(id)?;
        if changes == UpdateMeeting::default() {
            return Err(DataError::InvalidInput);
        }

        let _guard = self.database.mutation_guard()?;
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
        repository::update(&transaction, id, &title, &primary_language)?;
        let meeting = repository::get(&transaction, id)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub fn start(&self, id: &str) -> DataResult<Meeting> {
        validate_id(id)?;
        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let current = repository::get(&transaction, id)?;
        if current.status != MeetingStatus::Draft
            || current.started_at.is_some()
            || current.ended_at.is_some()
        {
            return Err(DataError::InvalidState);
        }

        repository::start(&transaction, id)?;
        let meeting = repository::get(&transaction, id)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub fn end(&self, id: &str) -> DataResult<Meeting> {
        validate_id(id)?;
        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let current = repository::get(&transaction, id)?;
        if current.status != MeetingStatus::Recording
            || current.started_at.is_none()
            || current.ended_at.is_some()
        {
            return Err(DataError::InvalidState);
        }

        repository::end(&transaction, id)?;
        let meeting = repository::get(&transaction, id)?;
        transaction.commit().map_err(DataError::from)?;
        Ok(meeting)
    }

    pub fn delete(&self, id: &str) -> DataResult<()> {
        validate_id(id)?;
        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let current = repository::get_including_deleted(&connection, id)?;
        if current.status == MeetingStatus::Deleting {
            return Err(DataError::MeetingDeletePending);
        }
        if !matches!(
            current.status,
            MeetingStatus::Draft | MeetingStatus::Ready | MeetingStatus::DeleteFailed
        ) {
            return Err(DataError::InvalidState);
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        repository::begin_delete(&transaction, id, &Uuid::new_v4().to_string())?;
        transaction.commit().map_err(DataError::from)?;
        execute_meeting_delete(&self.database, id)
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
    let parsed = Uuid::parse_str(id).map_err(|_| DataError::InvalidId)?;
    if parsed.to_string() != id {
        return Err(DataError::InvalidId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::params;

    use super::*;
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
                    primary_language: Some("zh-CN".to_string()),
                },
            )
            .expect("meeting should update");
        assert_eq!(updated.title, "Synthetic project review");
        assert_eq!(updated.status, MeetingStatus::Draft);
        assert_eq!(updated.primary_language, "zh-CN");
        assert_eq!(service.list().expect("meetings should list"), vec![updated]);

        service.delete(&created.id).expect("meeting should delete");
        assert_eq!(service.get(&created.id), Err(DataError::NotFound));
        assert!(service.list().expect("meeting list should load").is_empty());
    }

    #[test]
    fn enforces_the_synthetic_meeting_lifecycle_and_server_timestamps() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");
        let draft = service
            .create(create_input("Synthetic lifecycle meeting"))
            .expect("meeting should be created");

        assert_eq!(service.end(&draft.id), Err(DataError::InvalidState));

        let recording = service.start(&draft.id).expect("draft should start");
        let started_at = recording
            .started_at
            .clone()
            .expect("start should set started_at");
        assert_eq!(recording.status, MeetingStatus::Recording);
        assert!(started_at.ends_with('Z'));
        assert_eq!(recording.ended_at, None);
        assert_eq!(service.start(&draft.id), Err(DataError::InvalidState));
        assert_eq!(
            service
                .get(&draft.id)
                .expect("recording meeting should remain")
                .started_at,
            Some(started_at.clone())
        );

        let ready = service.end(&draft.id).expect("recording should end");
        let ended_at = ready.ended_at.expect("end should set ended_at");
        assert_eq!(ready.status, MeetingStatus::Ready);
        assert!(ended_at.ends_with('Z'));
        assert!(ended_at >= started_at);
        assert_eq!(service.end(&draft.id), Err(DataError::InvalidState));
        assert_eq!(service.start(&draft.id), Err(DataError::InvalidState));
    }

    #[test]
    fn only_deletes_draft_or_ready_meetings() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");

        let draft = service
            .create(create_input("Synthetic draft deletion"))
            .expect("draft should be created");
        service.delete(&draft.id).expect("draft should delete");
        assert_eq!(service.get(&draft.id), Err(DataError::NotFound));

        let meeting = service
            .create(create_input("Synthetic ready deletion"))
            .expect("meeting should be created");
        service.start(&meeting.id).expect("meeting should start");
        assert_eq!(service.delete(&meeting.id), Err(DataError::InvalidState));
        service.end(&meeting.id).expect("meeting should end");
        service.delete(&meeting.id).expect("ready should delete");
        assert_eq!(service.get(&meeting.id), Err(DataError::NotFound));
    }

    #[test]
    fn rejects_deletion_for_reserved_states() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");

        for status in ["processing", "failed", "archived"] {
            let meeting = service
                .create(create_input("Synthetic reserved state"))
                .expect("meeting should be created");
            let connection = service.database.connect().expect("database should connect");
            connection
                .execute(
                    "UPDATE meetings SET status = ?2 WHERE id = ?1",
                    params![&meeting.id, status],
                )
                .expect("test should set reserved state");
            drop(connection);

            assert_eq!(service.delete(&meeting.id), Err(DataError::InvalidState));
        }
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
    fn failed_delete_is_tombstoned_and_can_be_retried() {
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

        assert_eq!(
            service.delete(&meeting.id),
            Err(DataError::MeetingDeleteFailed)
        );
        assert_eq!(service.get(&meeting.id), Err(DataError::NotFound));
        let connection = service.database.connect().expect("database should connect");
        let failed = repository::get_including_deleted(&connection, &meeting.id)
            .expect("tombstoned meeting should remain for recovery");
        assert_eq!(failed.status, MeetingStatus::DeleteFailed);
        assert_eq!(
            failed.deletion_error_code.as_deref(),
            Some("meeting_delete_failed")
        );
        assert_eq!(
            service
                .get_with_deletion_state(&meeting.id)
                .expect("UI query should expose the deletion state")
                .status,
            MeetingStatus::DeleteFailed
        );
        assert!(
            service
                .list_with_deletion_state()
                .expect("UI list should expose deletion states")
                .iter()
                .any(|candidate| candidate.id == meeting.id)
        );
        connection
            .execute_batch("DROP TRIGGER synthetic_delete_failure;")
            .expect("failure trigger should be removed");
        drop(connection);

        service
            .delete(&meeting.id)
            .expect("explicit retry should finish deletion");
        assert_eq!(
            repository::get_including_deleted(
                &service.database.connect().expect("database should connect"),
                &meeting.id,
            ),
            Err(DataError::NotFound)
        );
    }

    #[test]
    fn repeated_delete_while_tombstone_is_pending_returns_stable_pending() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = service
            .create(create_input("Synthetic pending deletion"))
            .expect("meeting should be created");
        let mut connection = service.database.connect().expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        repository::begin_delete(&transaction, &meeting.id, &Uuid::new_v4().to_string())
            .expect("deletion should begin");
        transaction.commit().expect("tombstone should commit");

        assert_eq!(
            service.delete(&meeting.id),
            Err(DataError::MeetingDeletePending)
        );
    }

    #[test]
    fn returns_stable_errors_for_missing_or_invalid_meetings() {
        let directory = TestDirectory::new();
        let service = MeetingService::initialize(&directory.0).expect("service should initialize");
        let missing_id = Uuid::new_v4().to_string();

        assert_eq!(service.get(&missing_id), Err(DataError::NotFound));
        assert_eq!(service.delete(&missing_id), Err(DataError::NotFound));
        assert_eq!(service.get("not-a-uuid"), Err(DataError::InvalidId));
        assert_eq!(
            service.get(&Uuid::new_v4().simple().to_string()),
            Err(DataError::InvalidId)
        );
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
