use std::fmt;

use rusqlite::TransactionBehavior;

use super::database::Database;
use super::error::DataError;
use super::model::{CreateMeeting, Meeting};
use super::repository;
use super::service::MeetingService;
use super::transcript_error::TranscriptError;
use super::transcript_model::{
    StructuredAudioTranscript, TranscriptImportInput, TranscriptImportOutcome,
    TranscriptSourceOverview, TranscriptSourceType,
};
use super::transcript_repository;
use super::transcript_service::TranscriptService;

#[derive(Clone, Debug)]
pub struct TranscriptImportWorkflow {
    database: Database,
    transcripts: TranscriptService,
}

#[derive(Clone, PartialEq)]
pub struct TranscriptImportWorkflowOutcome {
    pub meeting: Meeting,
    pub transcript: TranscriptImportOutcome,
    pub source: TranscriptSourceOverview,
    pub duplicate: bool,
}

impl fmt::Debug for TranscriptImportWorkflowOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptImportWorkflowOutcome")
            .field("meeting_id", &self.meeting.id)
            .field("source_id", &self.source.source.id)
            .field("segment_count", &self.source.segment_count)
            .field("duplicate", &self.duplicate)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptWorkflowError {
    Meeting(DataError),
    Transcript(TranscriptError),
}

impl TranscriptWorkflowError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Meeting(error) => error.code(),
            Self::Transcript(error) => error.code(),
        }
    }

    pub const fn safe_message(&self) -> &'static str {
        match self {
            Self::Meeting(error) => error.safe_message(),
            Self::Transcript(error) => error.safe_message(),
        }
    }

    pub const fn retryable(&self) -> bool {
        match self {
            Self::Meeting(error) => error.retryable(),
            Self::Transcript(error) => error.retryable(),
        }
    }
}

impl fmt::Display for TranscriptWorkflowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for TranscriptWorkflowError {}

impl From<DataError> for TranscriptWorkflowError {
    fn from(error: DataError) -> Self {
        Self::Meeting(error)
    }
}

impl From<TranscriptError> for TranscriptWorkflowError {
    fn from(error: TranscriptError) -> Self {
        Self::Transcript(error)
    }
}

pub type TranscriptWorkflowResult<T> = Result<T, TranscriptWorkflowError>;

impl TranscriptImportWorkflow {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            transcripts: TranscriptService::new(database.clone()),
            database,
        }
    }

    pub fn create_meeting_from_transcript(
        &self,
        meeting: CreateMeeting,
        source_type: TranscriptSourceType,
        transcript: TranscriptImportInput,
    ) -> TranscriptWorkflowResult<TranscriptImportWorkflowOutcome> {
        let prepared_meeting = MeetingService::prepare_create(meeting)?;
        let prepared_transcript = self.transcripts.prepare_import(source_type, transcript)?;

        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let meeting = MeetingService::insert_prepared(&transaction, &prepared_meeting)?;
        let transcript =
            self.transcripts
                .import_prepared(&transaction, &meeting.id, prepared_transcript)?;
        let duplicate = matches!(transcript, TranscriptImportOutcome::Duplicate { .. });
        let source = transcript_repository::get_source_overview(
            &transaction,
            &meeting.id,
            transcript_source_id(&transcript),
        )?;
        transaction.commit().map_err(DataError::from)?;

        Ok(TranscriptImportWorkflowOutcome {
            meeting,
            transcript,
            source,
            duplicate,
        })
    }

    pub fn create_meeting_from_audio_transcript(
        &self,
        meeting: CreateMeeting,
        transcript: StructuredAudioTranscript,
    ) -> TranscriptWorkflowResult<TranscriptImportWorkflowOutcome> {
        let prepared_meeting = MeetingService::prepare_create(meeting)?;
        let prepared_transcript = self.transcripts.prepare_audio_import(transcript)?;

        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let meeting = MeetingService::insert_prepared(&transaction, &prepared_meeting)?;
        let transcript =
            self.transcripts
                .import_prepared(&transaction, &meeting.id, prepared_transcript)?;
        let duplicate = matches!(transcript, TranscriptImportOutcome::Duplicate { .. });
        let source = transcript_repository::get_source_overview(
            &transaction,
            &meeting.id,
            transcript_source_id(&transcript),
        )?;
        transaction.commit().map_err(DataError::from)?;

        Ok(TranscriptImportWorkflowOutcome {
            meeting,
            transcript,
            source,
            duplicate,
        })
    }

    pub fn add_transcript_to_meeting(
        &self,
        meeting_id: &str,
        source_type: TranscriptSourceType,
        transcript: TranscriptImportInput,
    ) -> TranscriptWorkflowResult<TranscriptImportWorkflowOutcome> {
        let prepared_transcript = self.transcripts.prepare_import(source_type, transcript)?;

        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let transcript =
            self.transcripts
                .import_prepared(&transaction, meeting_id, prepared_transcript)?;
        let meeting = repository::get(&transaction, meeting_id)?;
        let duplicate = matches!(transcript, TranscriptImportOutcome::Duplicate { .. });
        let source = transcript_repository::get_source_overview(
            &transaction,
            meeting_id,
            transcript_source_id(&transcript),
        )?;
        transaction.commit().map_err(DataError::from)?;

        Ok(TranscriptImportWorkflowOutcome {
            meeting,
            transcript,
            source,
            duplicate,
        })
    }

    pub fn add_audio_transcript_to_meeting(
        &self,
        meeting_id: &str,
        transcript: StructuredAudioTranscript,
    ) -> TranscriptWorkflowResult<TranscriptImportWorkflowOutcome> {
        let prepared_transcript = self.transcripts.prepare_audio_import(transcript)?;

        let _guard = self.database.mutation_guard()?;
        let mut connection = self.database.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DataError::from)?;
        let transcript =
            self.transcripts
                .import_prepared(&transaction, meeting_id, prepared_transcript)?;
        let meeting = repository::get(&transaction, meeting_id)?;
        let duplicate = matches!(transcript, TranscriptImportOutcome::Duplicate { .. });
        let source = transcript_repository::get_source_overview(
            &transaction,
            meeting_id,
            transcript_source_id(&transcript),
        )?;
        transaction.commit().map_err(DataError::from)?;

        Ok(TranscriptImportWorkflowOutcome {
            meeting,
            transcript,
            source,
            duplicate,
        })
    }
}

fn transcript_source_id(outcome: &TranscriptImportOutcome) -> &str {
    match outcome {
        TranscriptImportOutcome::Imported { source, .. } => &source.id,
        TranscriptImportOutcome::Duplicate { source_id } => source_id,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rusqlite::TransactionBehavior;
    use uuid::Uuid;

    use super::*;
    use crate::data::MeetingStatus;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "meetinglens-transcript-workflow-{}",
                Uuid::new_v4()
            )))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_meeting(title: &str) -> CreateMeeting {
        CreateMeeting {
            title: title.to_string(),
            primary_language: "en-AU".to_string(),
        }
    }

    fn pasted(display_name: &str, content: &[u8]) -> TranscriptImportInput {
        TranscriptImportInput {
            display_name: display_name.to_string(),
            original_filename: None,
            content: content.to_vec(),
            language: Some("en-AU".to_string()),
            base_started_at: None,
        }
    }

    fn audio(filename: &str, text: &str) -> StructuredAudioTranscript {
        StructuredAudioTranscript {
            display_name: filename.to_string(),
            original_filename: filename.to_string(),
            language: Some("en-AU".to_string()),
            parser_version: "faster-whisper/1.2.1".to_string(),
            segments: vec![crate::data::StructuredAudioSegment {
                start_offset_ms: 0,
                end_offset_ms: 1_000,
                original_text: text.to_string(),
            }],
        }
    }

    fn imported_source(outcome: &TranscriptImportWorkflowOutcome) -> (&str, usize) {
        match &outcome.transcript {
            TranscriptImportOutcome::Imported {
                source,
                segment_count,
            } => (&source.id, *segment_count),
            TranscriptImportOutcome::Duplicate { .. } => {
                panic!("the synthetic source should be newly imported")
            }
        }
    }

    #[test]
    fn atomically_creates_a_draft_meeting_and_transcript() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let workflow = meetings.transcript_imports();

        let outcome = workflow
            .create_meeting_from_transcript(
                create_meeting("Synthetic atomic meeting"),
                TranscriptSourceType::PastedText,
                pasted("Synthetic paste", b"Speaker 1: First\n\nSpeaker 2: Second"),
            )
            .expect("workflow should commit");

        assert_eq!(outcome.meeting.status, MeetingStatus::Draft);
        assert!(outcome.meeting.started_at.is_none());
        assert!(outcome.meeting.ended_at.is_none());
        let (source_id, segment_count) = imported_source(&outcome);
        assert_eq!(segment_count, 2);
        assert!(!outcome.duplicate);
        assert_eq!(outcome.source.source.id, source_id);
        assert_eq!(outcome.source.segment_count, 2);
        assert_eq!(
            meetings
                .transcripts()
                .get_source_overview(&outcome.meeting.id, source_id)
                .expect("overview should load")
                .segment_count,
            2
        );
    }

    #[test]
    fn atomically_creates_a_meeting_from_structured_audio() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let outcome = meetings
            .transcript_imports()
            .create_meeting_from_audio_transcript(
                create_meeting("Synthetic audio meeting"),
                audio("synthetic.mp3", "Synthetic audio transcript"),
            )
            .expect("audio workflow should commit");

        assert_eq!(outcome.meeting.status, MeetingStatus::Draft);
        assert_eq!(
            outcome.source.source.source_type,
            TranscriptSourceType::ImportedAudio
        );
        assert_eq!(outcome.source.segment_count, 1);
        assert!(!outcome.duplicate);
    }

    #[test]
    fn parse_failure_leaves_no_empty_meeting() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let error = meetings
            .transcript_imports()
            .create_meeting_from_transcript(
                create_meeting("Synthetic invalid transcript"),
                TranscriptSourceType::PastedText,
                pasted("Empty paste", b"  \n"),
            )
            .expect_err("empty transcript should fail");

        assert_eq!(
            error,
            TranscriptWorkflowError::Transcript(TranscriptError::Empty)
        );
        assert!(meetings.list().expect("meetings should list").is_empty());
    }

    #[test]
    fn transcript_database_failure_rolls_back_the_meeting_insert() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let workflow = meetings.transcript_imports();
        workflow
            .database
            .connect()
            .expect("database should connect")
            .execute_batch(
                "
                CREATE TRIGGER synthetic_transcript_insert_failure
                BEFORE INSERT ON transcript_sources
                BEGIN
                    SELECT RAISE(ABORT, 'synthetic transcript failure');
                END;
                ",
            )
            .expect("failure trigger should install");

        let error = workflow
            .create_meeting_from_transcript(
                create_meeting("Synthetic rollback meeting"),
                TranscriptSourceType::PastedText,
                pasted("Synthetic paste", b"Synthetic valid content"),
            )
            .expect_err("transcript insert should fail");

        assert_eq!(
            error,
            TranscriptWorkflowError::Transcript(TranscriptError::PersistenceFailed)
        );
        assert!(meetings.list().expect("meetings should list").is_empty());
    }

    #[test]
    fn audio_segment_failure_rolls_back_the_meeting_and_source() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let workflow = meetings.transcript_imports();
        workflow
            .database
            .connect()
            .expect("database should connect")
            .execute_batch(
                "CREATE TRIGGER synthetic_audio_segment_failure
                 BEFORE INSERT ON transcript_segments
                 BEGIN SELECT RAISE(ABORT, 'synthetic audio failure'); END;",
            )
            .expect("failure trigger should install");

        assert_eq!(
            workflow.create_meeting_from_audio_transcript(
                create_meeting("Synthetic audio rollback"),
                audio("synthetic.wav", "Synthetic audio rollback transcript"),
            ),
            Err(TranscriptWorkflowError::Transcript(
                TranscriptError::PersistenceFailed
            ))
        );
        assert!(meetings.list().expect("meetings should list").is_empty());
    }

    #[test]
    fn existing_draft_recording_and_ready_meetings_accept_transcripts() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let workflow = meetings.transcript_imports();

        let draft = meetings
            .create(create_meeting("Synthetic draft"))
            .expect("draft should create");
        let recording = meetings
            .create(create_meeting("Synthetic recording"))
            .and_then(|meeting| meetings.start(&meeting.id))
            .expect("recording should start");
        let ready = meetings
            .create(create_meeting("Synthetic ready"))
            .and_then(|meeting| meetings.start(&meeting.id))
            .and_then(|meeting| meetings.end(&meeting.id))
            .expect("ready meeting should end");

        for (index, meeting) in [draft, recording, ready].iter().enumerate() {
            let outcome = workflow
                .add_transcript_to_meeting(
                    &meeting.id,
                    TranscriptSourceType::PastedText,
                    pasted(
                        &format!("Synthetic state {index}"),
                        format!("Synthetic transcript for state {index}").as_bytes(),
                    ),
                )
                .expect("supported state should import");
            assert_eq!(outcome.meeting.status, meeting.status);
            assert_eq!(imported_source(&outcome).1, 1);
        }
    }

    #[test]
    fn duplicate_outcome_contains_the_existing_overview_in_the_same_transaction() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting = meetings
            .create(create_meeting("Synthetic duplicate"))
            .expect("meeting should create");
        let workflow = meetings.transcript_imports();
        let first = workflow
            .add_transcript_to_meeting(
                &meeting.id,
                TranscriptSourceType::PastedText,
                pasted("First display", b"Synthetic duplicate body"),
            )
            .expect("first import should succeed");
        let duplicate = workflow
            .add_transcript_to_meeting(
                &meeting.id,
                TranscriptSourceType::PastedText,
                pasted("Second display", b"Synthetic duplicate body"),
            )
            .expect("duplicate should return safely");

        assert!(duplicate.duplicate);
        assert_eq!(duplicate.source.source.id, first.source.source.id);
        assert_eq!(duplicate.source.segment_count, 1);
        assert_eq!(
            meetings
                .transcripts()
                .list_source_overviews(&meeting.id)
                .expect("sources should list")
                .len(),
            1
        );
    }

    #[test]
    fn deleting_and_delete_failed_meetings_fail_closed() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let workflow = meetings.transcript_imports();
        let deleting = meetings
            .create(create_meeting("Synthetic deleting"))
            .expect("meeting should create");
        let delete_failed = meetings
            .create(create_meeting("Synthetic delete failed"))
            .expect("meeting should create");

        let mut connection = workflow
            .database
            .connect()
            .expect("database should connect");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction should begin");
        repository::begin_delete(&transaction, &deleting.id, &Uuid::new_v4().to_string())
            .expect("deletion should begin");
        repository::begin_delete(&transaction, &delete_failed.id, &Uuid::new_v4().to_string())
            .expect("deletion should begin");
        repository::mark_delete_failed(&transaction, &delete_failed.id, "synthetic_failure")
            .expect("deletion should fail safely");
        transaction.commit().expect("states should commit");
        drop(connection);

        for meeting in [deleting, delete_failed] {
            assert_eq!(
                workflow.add_transcript_to_meeting(
                    &meeting.id,
                    TranscriptSourceType::PastedText,
                    pasted("Blocked source", b"Synthetic blocked content"),
                ),
                Err(TranscriptWorkflowError::Transcript(
                    TranscriptError::MeetingInvalidState
                ))
            );
        }
    }

    #[test]
    fn source_overviews_are_counted_and_scoped_to_their_meeting() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let first = meetings
            .create(create_meeting("Synthetic first"))
            .expect("meeting should create");
        let second = meetings
            .create(create_meeting("Synthetic second"))
            .expect("meeting should create");
        let outcome = meetings
            .transcript_imports()
            .add_transcript_to_meeting(
                &first.id,
                TranscriptSourceType::PastedText,
                pasted("Counted source", b"First segment\n\nSecond segment"),
            )
            .expect("source should import");
        let (source_id, _) = imported_source(&outcome);
        let transcripts = meetings.transcripts();

        assert_eq!(
            transcripts
                .list_source_overviews(&first.id)
                .expect("overviews should list")
                .into_iter()
                .map(|overview: TranscriptSourceOverview| overview.segment_count)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert!(matches!(
            transcripts.get_source_overview(&second.id, source_id),
            Err(TranscriptError::SourceNotFound)
        ));
        assert_eq!(
            transcripts.list_segments(&second.id, source_id),
            Err(TranscriptError::SourceNotFound)
        );
    }

    #[test]
    fn missing_meeting_uses_the_stable_meeting_error() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let error = meetings
            .transcript_imports()
            .add_transcript_to_meeting(
                &Uuid::new_v4().to_string(),
                TranscriptSourceType::PastedText,
                pasted("Missing meeting", b"Synthetic content"),
            )
            .expect_err("missing meeting should fail");

        assert_eq!(
            error,
            TranscriptWorkflowError::Transcript(TranscriptError::MeetingNotFound)
        );
        assert_eq!(error.code(), "meeting_not_found");
    }
}
