use rusqlite::{Transaction, TransactionBehavior};
use uuid::Uuid;

use super::database::Database;
use super::error::DataError;
use super::model::MeetingStatus;
use super::repository;
use super::transcript_error::{TranscriptError, TranscriptResult};
use super::transcript_model::{
    NewTranscriptSegment, NewTranscriptSource, SummaryInputSegment, TRANSCRIPT_MAX_SEGMENTS,
    TRANSCRIPT_PAGE_DEFAULT_LIMIT, TRANSCRIPT_PAGE_MAX_LIMIT, TranscriptImportInput,
    TranscriptImportOutcome, TranscriptPolicy, TranscriptSegment, TranscriptSegmentPage,
    TranscriptSource, TranscriptSourceOverview, TranscriptSourceType, TranscriptSpeakerGroupPage,
    UpdateTranscriptSegment,
};
use super::transcript_parser::ParsedTranscript;
use super::{transcript_parser, transcript_repository};

const MAX_DISPLAY_NAME_BYTES: usize = 255;
const MAX_FILENAME_BYTES: usize = 255;
const MAX_LANGUAGE_BYTES: usize = 35;
const MAX_SPEAKER_NAME_BYTES: usize = 255;

#[derive(Clone, Debug)]
pub struct TranscriptService {
    database: Database,
    policy: TranscriptPolicy,
}

#[derive(Clone, PartialEq)]
pub(crate) struct PreparedTranscriptImport {
    source_type: TranscriptSourceType,
    display_name: String,
    original_filename: Option<String>,
    language: Option<String>,
    base_started_at: Option<String>,
    parsed: ParsedTranscript,
}

impl TranscriptService {
    pub(crate) fn new(database: Database) -> Self {
        Self {
            database,
            policy: TranscriptPolicy::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_policy(database: Database, policy: TranscriptPolicy) -> Self {
        Self { database, policy }
    }

    pub const fn policy(&self) -> TranscriptPolicy {
        self.policy
    }

    pub fn import_pasted_text(
        &self,
        meeting_id: &str,
        mut input: TranscriptImportInput,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        if input.original_filename.is_some() {
            return Err(TranscriptError::InvalidInput);
        }
        input.original_filename = None;
        self.import(meeting_id, TranscriptSourceType::PastedText, input)
    }

    pub fn import_txt(
        &self,
        meeting_id: &str,
        input: TranscriptImportInput,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        self.import(meeting_id, TranscriptSourceType::ImportedTxt, input)
    }

    pub fn import_srt(
        &self,
        meeting_id: &str,
        input: TranscriptImportInput,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        self.import(meeting_id, TranscriptSourceType::ImportedSrt, input)
    }

    pub fn import_vtt(
        &self,
        meeting_id: &str,
        input: TranscriptImportInput,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        self.import(meeting_id, TranscriptSourceType::ImportedVtt, input)
    }

    fn import(
        &self,
        meeting_id: &str,
        source_type: TranscriptSourceType,
        input: TranscriptImportInput,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        validate_uuid(meeting_id)?;
        let prepared = self.prepare_import(source_type, input)?;

        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TranscriptError::from)?;
        let outcome = self.import_prepared(&transaction, meeting_id, prepared)?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(outcome)
    }

    pub(crate) fn prepare_import(
        &self,
        source_type: TranscriptSourceType,
        mut input: TranscriptImportInput,
    ) -> TranscriptResult<PreparedTranscriptImport> {
        if !source_type.is_currently_importable() {
            return Err(TranscriptError::InvalidInput);
        }
        if source_type == TranscriptSourceType::PastedText {
            if input.original_filename.is_some() {
                return Err(TranscriptError::InvalidInput);
            }
            input.original_filename = None;
        }
        let display_name = normalize_display_name(&input.display_name)?;
        let original_filename = validate_original_filename(source_type, input.original_filename)?;
        let language = normalize_optional_language(input.language)?;
        let base_started_at = normalize_optional_utc(input.base_started_at)?;
        let parsed = transcript_parser::parse(source_type, &input.content, self.policy)?;

        Ok(PreparedTranscriptImport {
            source_type,
            display_name,
            original_filename,
            language,
            base_started_at,
            parsed,
        })
    }

    pub(crate) fn import_prepared(
        &self,
        transaction: &Transaction<'_>,
        meeting_id: &str,
        prepared: PreparedTranscriptImport,
    ) -> TranscriptResult<TranscriptImportOutcome> {
        validate_uuid(meeting_id)?;
        ensure_meeting_accepts_mutation(transaction, meeting_id)?;

        if let Some(existing) = transcript_repository::find_source_by_hash(
            transaction,
            meeting_id,
            &prepared.parsed.content_sha256,
        )? {
            return Ok(TranscriptImportOutcome::Duplicate {
                source_id: existing.id,
            });
        }

        let source_id = Uuid::new_v4().to_string();
        let source = NewTranscriptSource {
            id: source_id.clone(),
            meeting_id: meeting_id.to_string(),
            source_type: prepared.source_type,
            display_name: prepared.display_name,
            original_filename: prepared.original_filename,
            content_sha256: prepared.parsed.content_sha256.clone(),
            language: prepared.language.clone(),
            base_started_at: prepared.base_started_at,
            parser_name: prepared.parsed.parser_name.to_string(),
            parser_version: prepared.parsed.parser_version.to_string(),
        };
        if let Err(error) = transcript_repository::insert_source(transaction, &source) {
            if let Some(existing) = transcript_repository::find_source_by_hash(
                transaction,
                meeting_id,
                &prepared.parsed.content_sha256,
            )? {
                return Ok(TranscriptImportOutcome::Duplicate {
                    source_id: existing.id,
                });
            }
            return Err(error);
        }

        let segments = prepared
            .parsed
            .segments
            .into_iter()
            .enumerate()
            .map(|(ordinal, segment)| NewTranscriptSegment {
                id: Uuid::new_v4().to_string(),
                meeting_id: meeting_id.to_string(),
                source_id: source_id.clone(),
                ordinal: ordinal as u64,
                speaker_label: segment.speaker_label,
                speaker_name: None,
                start_offset_ms: segment.start_offset_ms,
                end_offset_ms: segment.end_offset_ms,
                original_text: segment.original_text,
                translated_text: None,
                language: prepared.language.clone(),
                confidence: None,
                is_final: true,
            })
            .collect::<Vec<_>>();
        transcript_repository::insert_segments(transaction, &segments)?;
        let persisted = transcript_repository::get_source(transaction, meeting_id, &source_id)?;
        Ok(TranscriptImportOutcome::Imported {
            source: Box::new(persisted),
            segment_count: segments.len(),
        })
    }

    pub fn get_source(
        &self,
        meeting_id: &str,
        source_id: &str,
    ) -> TranscriptResult<TranscriptSource> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::get_source(&connection, meeting_id, source_id)
    }

    pub fn list_sources(&self, meeting_id: &str) -> TranscriptResult<Vec<TranscriptSource>> {
        validate_uuid(meeting_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::list_sources(&connection, meeting_id)
    }

    pub fn get_source_overview(
        &self,
        meeting_id: &str,
        source_id: &str,
    ) -> TranscriptResult<TranscriptSourceOverview> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::get_source_overview(&connection, meeting_id, source_id)
    }

    pub fn list_source_overviews(
        &self,
        meeting_id: &str,
    ) -> TranscriptResult<Vec<TranscriptSourceOverview>> {
        validate_uuid(meeting_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::list_source_overviews(&connection, meeting_id)
    }

    pub fn get_segment(
        &self,
        meeting_id: &str,
        segment_id: &str,
    ) -> TranscriptResult<TranscriptSegment> {
        validate_uuid(meeting_id)?;
        validate_uuid(segment_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::get_segment(&connection, meeting_id, segment_id)
    }

    pub fn list_segments(
        &self,
        meeting_id: &str,
        source_id: &str,
    ) -> TranscriptResult<Vec<TranscriptSegment>> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        ensure_meeting_is_readable(&connection, meeting_id)?;
        transcript_repository::get_source(&connection, meeting_id, source_id)?;
        transcript_repository::list_segments(&connection, meeting_id, source_id)
    }

    pub fn list_segment_page(
        &self,
        meeting_id: &str,
        source_id: &str,
        cursor: Option<u64>,
        limit: Option<u64>,
    ) -> TranscriptResult<TranscriptSegmentPage> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let limit = validate_page_limit(limit)?;
        if cursor.is_some_and(|cursor| cursor >= TRANSCRIPT_MAX_SEGMENTS as u64) {
            return Err(TranscriptError::InvalidInput);
        }
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(TranscriptError::from)?;
        ensure_meeting_is_readable(&transaction, meeting_id)?;
        transcript_repository::get_source(&transaction, meeting_id, source_id)?;
        let page = transcript_repository::list_segment_page(
            &transaction,
            meeting_id,
            source_id,
            cursor,
            limit,
        )?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(page)
    }

    pub fn list_speaker_group_page(
        &self,
        meeting_id: &str,
        source_id: &str,
        cursor: Option<String>,
        limit: Option<u64>,
    ) -> TranscriptResult<TranscriptSpeakerGroupPage> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let cursor = cursor
            .map(|cursor| normalize_required(&cursor, MAX_SPEAKER_NAME_BYTES))
            .transpose()?;
        let limit = validate_page_limit(limit)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(TranscriptError::from)?;
        ensure_meeting_is_readable(&transaction, meeting_id)?;
        transcript_repository::get_source(&transaction, meeting_id, source_id)?;
        let page = transcript_repository::list_speaker_group_page(
            &transaction,
            meeting_id,
            source_id,
            cursor.as_deref(),
            limit,
        )?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(page)
    }

    pub fn update_segment(
        &self,
        meeting_id: &str,
        segment_id: &str,
        update: UpdateTranscriptSegment,
    ) -> TranscriptResult<TranscriptSegment> {
        validate_uuid(meeting_id)?;
        validate_uuid(segment_id)?;
        if update.expected_revision == 0
            || (update.speaker_name.is_none() && update.original_text.is_none())
        {
            return Err(TranscriptError::InvalidInput);
        }

        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TranscriptError::from)?;
        ensure_meeting_accepts_mutation(&transaction, meeting_id)?;
        let current = transcript_repository::get_segment(&transaction, meeting_id, segment_id)?;
        let speaker_name = match update.speaker_name {
            Some(Some(value)) => Some(normalize_required(&value, MAX_SPEAKER_NAME_BYTES)?),
            Some(None) => None,
            None => current.speaker_name,
        };
        let original_text = match update.original_text {
            Some(value) => normalize_transcript_text(&value, self.policy.max_segment_bytes)?,
            None => current.original_text,
        };
        let updated = transcript_repository::update_segment(
            &transaction,
            meeting_id,
            segment_id,
            speaker_name.as_deref(),
            &original_text,
            update.expected_revision,
        )?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(updated)
    }

    pub fn rename_speaker_in_source(
        &self,
        meeting_id: &str,
        source_id: &str,
        speaker_label: &str,
        speaker_name: Option<String>,
    ) -> TranscriptResult<u64> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let speaker_label = normalize_required(speaker_label, MAX_SPEAKER_NAME_BYTES)?;
        let speaker_name = speaker_name
            .map(|speaker_name| normalize_required(&speaker_name, MAX_SPEAKER_NAME_BYTES))
            .transpose()?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TranscriptError::from)?;
        ensure_meeting_accepts_mutation(&transaction, meeting_id)?;
        transcript_repository::get_source(&transaction, meeting_id, source_id)?;
        let updated_count = transcript_repository::rename_speaker_in_source(
            &transaction,
            meeting_id,
            source_id,
            &speaker_label,
            speaker_name.as_deref(),
        )?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(updated_count)
    }

    pub fn delete_source(&self, meeting_id: &str, source_id: &str) -> TranscriptResult<()> {
        validate_uuid(meeting_id)?;
        validate_uuid(source_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(TranscriptError::from)?;
        ensure_meeting_accepts_mutation(&transaction, meeting_id)?;
        transcript_repository::get_source(&transaction, meeting_id, source_id)?;

        // Before Summary runs are introduced, their migration must add an active-snapshot
        // guard here. A source must never disappear from a run that has not frozen evidence.
        transcript_repository::delete_source(&transaction, meeting_id, source_id)?;
        transaction.commit().map_err(TranscriptError::from)
    }

    pub fn build_summary_input(
        &self,
        meeting_id: &str,
        source_ids: &[String],
    ) -> TranscriptResult<Vec<SummaryInputSegment>> {
        validate_uuid(meeting_id)?;
        for source_id in source_ids {
            validate_uuid(source_id)?;
        }
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(TranscriptError::from)?;
        ensure_meeting_is_readable(&transaction, meeting_id)?;
        let input =
            transcript_repository::build_summary_input(&transaction, meeting_id, source_ids)?;
        transaction.commit().map_err(TranscriptError::from)?;
        Ok(input)
    }
}

fn ensure_meeting_accepts_mutation(
    connection: &rusqlite::Connection,
    meeting_id: &str,
) -> TranscriptResult<()> {
    let meeting =
        repository::get_including_deleted(connection, meeting_id).map_err(map_data_error)?;
    if meeting.deletion_error_code.is_none()
        && matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        )
    {
        Ok(())
    } else {
        Err(TranscriptError::MeetingInvalidState)
    }
}

fn ensure_meeting_is_readable(
    connection: &rusqlite::Connection,
    meeting_id: &str,
) -> TranscriptResult<()> {
    repository::get(connection, meeting_id)
        .map(|_| ())
        .map_err(map_data_error)
}

fn map_data_error(error: DataError) -> TranscriptError {
    match error {
        DataError::Database | DataError::MigrationConflict | DataError::StorageUnavailable => {
            TranscriptError::PersistenceFailed
        }
        DataError::InvalidState
        | DataError::MeetingDeleteFailed
        | DataError::MeetingDeletePending => TranscriptError::MeetingInvalidState,
        DataError::NotFound => TranscriptError::MeetingNotFound,
        DataError::InvalidId | DataError::InvalidInput => TranscriptError::InvalidInput,
        DataError::UnsafePath => TranscriptError::Internal,
    }
}

fn validate_uuid(value: &str) -> TranscriptResult<()> {
    if value.len() != 36 {
        return Err(TranscriptError::InvalidInput);
    }
    let parsed = Uuid::parse_str(value).map_err(|_| TranscriptError::InvalidInput)?;
    if parsed.hyphenated().to_string() != value {
        return Err(TranscriptError::InvalidInput);
    }
    Ok(())
}

fn normalize_required(value: &str, max_bytes: usize) -> TranscriptResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > max_bytes
        || value.chars().any(|character| character.is_control())
    {
        return Err(TranscriptError::InvalidInput);
    }
    Ok(value.to_string())
}

fn validate_page_limit(limit: Option<u64>) -> TranscriptResult<usize> {
    let limit = limit.unwrap_or(TRANSCRIPT_PAGE_DEFAULT_LIMIT);
    if !(1..=TRANSCRIPT_PAGE_MAX_LIMIT).contains(&limit) {
        return Err(TranscriptError::InvalidInput);
    }
    usize::try_from(limit).map_err(|_| TranscriptError::InvalidInput)
}

fn normalize_display_name(value: &str) -> TranscriptResult<String> {
    let value = normalize_required(value, MAX_DISPLAY_NAME_BYTES)?;
    let bytes = value.as_bytes();
    let looks_absolute = value.starts_with('/')
        || value.starts_with("\\\\")
        || value
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'));
    if looks_absolute {
        return Err(TranscriptError::InvalidInput);
    }
    Ok(value)
}

fn normalize_optional_language(value: Option<String>) -> TranscriptResult<Option<String>> {
    value
        .map(|value| {
            let value = normalize_required(&value, MAX_LANGUAGE_BYTES)?;
            if value.starts_with('-')
                || value.ends_with('-')
                || value
                    .chars()
                    .any(|character| !character.is_ascii_alphanumeric() && character != '-')
            {
                return Err(TranscriptError::InvalidInput);
            }
            Ok(value)
        })
        .transpose()
}

fn validate_original_filename(
    source_type: TranscriptSourceType,
    value: Option<String>,
) -> TranscriptResult<Option<String>> {
    if source_type == TranscriptSourceType::PastedText {
        return if value.is_none() {
            Ok(None)
        } else {
            Err(TranscriptError::InvalidInput)
        };
    }
    let value = value.ok_or(TranscriptError::InvalidInput)?;
    let value = normalize_required(&value, MAX_FILENAME_BYTES)?;
    if value == "."
        || value == ".."
        || value.contains(['/', '\\', ':'])
        || !filename_extension_matches(source_type, &value)
    {
        return Err(TranscriptError::InvalidInput);
    }
    Ok(Some(value))
}

fn filename_extension_matches(source_type: TranscriptSourceType, filename: &str) -> bool {
    let extension = filename.rsplit_once('.').map(|(_, value)| value);
    matches!(
        (
            source_type,
            extension.map(str::to_ascii_lowercase).as_deref(),
        ),
        (TranscriptSourceType::ImportedTxt, Some("txt"))
            | (TranscriptSourceType::ImportedSrt, Some("srt"))
            | (TranscriptSourceType::ImportedVtt, Some("vtt"))
    )
}

fn normalize_transcript_text(value: &str, max_bytes: usize) -> TranscriptResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(TranscriptError::Empty);
    }
    if value.len() > max_bytes {
        return Err(TranscriptError::SegmentTooLarge);
    }
    if value.chars().any(|character| {
        (character < ' ' && !matches!(character, '\t' | '\n')) || character == '\u{7f}'
    }) {
        return Err(TranscriptError::InvalidEncoding);
    }
    Ok(value.to_string())
}

fn normalize_optional_utc(value: Option<String>) -> TranscriptResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim();
            validate_utc_iso8601(value)?;
            Ok(value.to_string())
        })
        .transpose()
}

fn validate_utc_iso8601(value: &str) -> TranscriptResult<()> {
    if !value.is_ascii() || !value.ends_with('Z') || value.len() < 20 {
        return Err(TranscriptError::InvalidTimestamp);
    }
    let core = &value[..value.len() - 1];
    let (date, time) = core
        .split_once('T')
        .ok_or(TranscriptError::InvalidTimestamp)?;
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return Err(TranscriptError::InvalidTimestamp);
    }
    let year = parse_decimal(&date[0..4])?;
    let month = parse_decimal(&date[5..7])?;
    let day = parse_decimal(&date[8..10])?;
    if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return Err(TranscriptError::InvalidTimestamp);
    }

    let (whole_time, fraction) = match time.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (time, None),
    };
    if whole_time.len() != 8 || whole_time.as_bytes()[2] != b':' || whole_time.as_bytes()[5] != b':'
    {
        return Err(TranscriptError::InvalidTimestamp);
    }
    let hour = parse_decimal(&whole_time[0..2])?;
    let minute = parse_decimal(&whole_time[3..5])?;
    let second = parse_decimal(&whole_time[6..8])?;
    if hour > 23 || minute > 59 || second > 59 {
        return Err(TranscriptError::InvalidTimestamp);
    }
    if fraction.is_some_and(|fraction| {
        fraction.is_empty()
            || fraction.len() > 9
            || !fraction.chars().all(|character| character.is_ascii_digit())
    }) {
        return Err(TranscriptError::InvalidTimestamp);
    }
    Ok(())
}

fn parse_decimal(value: &str) -> TranscriptResult<u32> {
    if !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(TranscriptError::InvalidTimestamp);
    }
    value
        .parse::<u32>()
        .map_err(|_| TranscriptError::InvalidTimestamp)
}

const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};

    use super::*;
    use crate::data::{CreateMeeting, MeetingService};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!("meetinglens-transcript-service-{}", Uuid::new_v4())),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn setup() -> (TestDirectory, MeetingService, String) {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let meeting_id = meetings
            .create(CreateMeeting {
                title: "Synthetic transcript service".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create")
            .id;
        (directory, meetings, meeting_id)
    }

    fn input(name: &str, filename: Option<&str>, content: &[u8]) -> TranscriptImportInput {
        TranscriptImportInput {
            display_name: name.to_string(),
            original_filename: filename.map(str::to_string),
            content: content.to_vec(),
            language: Some("en-AU".to_string()),
            base_started_at: Some("2026-08-06T00:00:00.000Z".to_string()),
        }
    }

    fn imported_source(outcome: TranscriptImportOutcome) -> TranscriptSource {
        match outcome {
            TranscriptImportOutcome::Imported { source, .. } => *source,
            TranscriptImportOutcome::Duplicate { .. } => panic!("expected a new source"),
        }
    }

    #[test]
    fn imports_reads_updates_summarizes_deletes_and_persists() {
        let (directory, meetings, meeting_id) = setup();
        let transcripts = meetings.transcripts();
        let source = imported_source(
            transcripts
                .import_pasted_text(
                    &meeting_id,
                    input(
                        "Synthetic paste",
                        None,
                        b"Alice: First synthetic point\n\nSecond synthetic point",
                    ),
                )
                .expect("paste should import"),
        );
        let segments = transcripts
            .list_segments(&meeting_id, &source.id)
            .expect("segments should list");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker_label.as_deref(), Some("Alice"));
        assert!(segments.iter().all(|segment| {
            segment.is_final
                && segment.revision == 1
                && segment.translated_text.is_none()
                && segment.confidence.is_none()
        }));

        let corrected = transcripts
            .update_segment(
                &meeting_id,
                &segments[0].id,
                UpdateTranscriptSegment {
                    speaker_name: Some(Some("Alex".to_string())),
                    original_text: Some("Corrected synthetic point".to_string()),
                    expected_revision: 1,
                },
            )
            .expect("segment should update");
        assert_eq!(corrected.revision, 2);
        assert!(corrected.corrected_at.is_some());
        assert_eq!(
            transcripts.update_segment(
                &meeting_id,
                &segments[0].id,
                UpdateTranscriptSegment {
                    speaker_name: None,
                    original_text: Some("Stale update".to_string()),
                    expected_revision: 1,
                }
            ),
            Err(TranscriptError::RevisionConflict)
        );

        let summary = transcripts
            .build_summary_input(&meeting_id, std::slice::from_ref(&source.id))
            .expect("summary input should build");
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].revision, 2);
        assert_eq!(summary[0].original_text, "Corrected synthetic point");
        drop(transcripts);
        drop(meetings);

        let reopened = MeetingService::initialize(&directory.0).expect("service should reopen");
        let transcripts = reopened.transcripts();
        assert_eq!(
            transcripts
                .list_segments(&meeting_id, &source.id)
                .expect("segments should persist")
                .len(),
            2
        );
        transcripts
            .delete_source(&meeting_id, &source.id)
            .expect("source should delete");
        assert_eq!(
            transcripts.get_source(&meeting_id, &source.id),
            Err(TranscriptError::SourceNotFound)
        );
        assert_eq!(
            transcripts.build_summary_input(&meeting_id, &[source.id]),
            Err(TranscriptError::SourceNotFound)
        );
    }

    #[test]
    fn returns_stable_duplicate_and_allows_same_hash_in_another_meeting() {
        let (_directory, meetings, meeting_id) = setup();
        let transcripts = meetings.transcripts();
        let first = imported_source(
            transcripts
                .import_txt(
                    &meeting_id,
                    input("First", Some("first.txt"), b"Synthetic\r\ntext"),
                )
                .expect("first source should import"),
        );
        assert_eq!(
            transcripts
                .import_txt(
                    &meeting_id,
                    input("Duplicate", Some("other.txt"), b"Synthetic\ntext"),
                )
                .expect("duplicate should be stable"),
            TranscriptImportOutcome::Duplicate {
                source_id: first.id.clone()
            }
        );

        let other_meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic second meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("second meeting should create");
        assert!(matches!(
            transcripts.import_txt(
                &other_meeting.id,
                input("Other meeting", Some("same.txt"), b"Synthetic\ntext"),
            ),
            Ok(TranscriptImportOutcome::Imported { .. })
        ));
    }

    #[test]
    fn bounds_pages_and_rolls_back_failed_speaker_renames() {
        let (_directory, meetings, meeting_id) = setup();
        let transcripts = meetings.transcripts();
        let content = (0..205)
            .map(|ordinal| format!("Speaker {ordinal:03}: Synthetic point {ordinal}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let source = imported_source(
            transcripts
                .import_pasted_text(
                    &meeting_id,
                    input("Synthetic paged paste", None, content.as_bytes()),
                )
                .expect("paged paste should import"),
        );

        let first = transcripts
            .list_segment_page(&meeting_id, &source.id, None, None)
            .expect("default page should load");
        assert_eq!(first.items.len(), 100);
        assert_eq!(first.total_count, 205);
        assert_eq!(first.next_cursor, Some(99));
        assert!(first.has_more);
        let second = transcripts
            .list_segment_page(&meeting_id, &source.id, first.next_cursor, Some(200))
            .expect("bounded second page should load");
        assert_eq!(second.items.len(), 105);
        assert_eq!(second.items[0].ordinal, 100);
        assert!(!second.has_more);
        assert_eq!(
            transcripts.list_segment_page(&meeting_id, &source.id, None, Some(0)),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            transcripts.list_segment_page(&meeting_id, &source.id, None, Some(201)),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            transcripts.list_segment_page(
                &meeting_id,
                &source.id,
                Some(TRANSCRIPT_MAX_SEGMENTS as u64),
                None
            ),
            Err(TranscriptError::InvalidInput)
        );

        let first_speakers = transcripts
            .list_speaker_group_page(&meeting_id, &source.id, None, Some(100))
            .expect("speaker page should load");
        assert_eq!(first_speakers.items.len(), 100);
        assert_eq!(first_speakers.total_count, 205);
        assert!(first_speakers.has_more);
        assert_eq!(
            transcripts.list_speaker_group_page(
                &meeting_id,
                &source.id,
                None,
                Some(TRANSCRIPT_PAGE_MAX_LIMIT + 1)
            ),
            Err(TranscriptError::InvalidInput)
        );

        assert_eq!(
            transcripts.rename_speaker_in_source(
                &meeting_id,
                &source.id,
                "Speaker 000",
                Some("   ".to_string())
            ),
            Err(TranscriptError::InvalidInput)
        );
        let connection = transcripts
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_synthetic_source_touch
                BEFORE UPDATE OF updated_at ON transcript_sources
                BEGIN
                    SELECT RAISE(ABORT, 'synthetic rollback');
                END;
                ",
            )
            .expect("rollback trigger should create");
        drop(connection);
        assert_eq!(
            transcripts.rename_speaker_in_source(
                &meeting_id,
                &source.id,
                "Speaker 000",
                Some("Confirmed synthetic name".to_string())
            ),
            Err(TranscriptError::PersistenceFailed)
        );
        let connection = transcripts
            .database
            .connect()
            .expect("database should reconnect");
        connection
            .execute_batch("DROP TRIGGER fail_synthetic_source_touch;")
            .expect("rollback trigger should drop");
        drop(connection);
        let unchanged = transcripts
            .list_segment_page(&meeting_id, &source.id, None, Some(1))
            .expect("rolled back segment should load")
            .items
            .remove(0);
        assert_eq!(unchanged.speaker_name, None);
        assert_eq!(unchanged.revision, 1);
        assert_eq!(unchanged.corrected_at, None);

        let other_meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic scoped meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("scoped meeting should create");
        assert_eq!(
            transcripts.list_segment_page(&other_meeting.id, &source.id, None, None),
            Err(TranscriptError::SourceNotFound)
        );
        assert_eq!(
            transcripts.rename_speaker_in_source(
                &other_meeting.id,
                &source.id,
                "Speaker 000",
                Some("Scoped name".to_string())
            ),
            Err(TranscriptError::SourceNotFound)
        );

        let connection = transcripts
            .database
            .connect()
            .expect("database should connect for state check");
        connection
            .execute(
                "UPDATE meetings SET status = 'processing' WHERE id = ?1",
                [&meeting_id],
            )
            .expect("meeting state should update");
        drop(connection);
        assert_eq!(
            transcripts.rename_speaker_in_source(
                &meeting_id,
                &source.id,
                "Speaker 000",
                Some("Blocked name".to_string())
            ),
            Err(TranscriptError::MeetingInvalidState)
        );
    }

    #[test]
    fn serializes_competing_duplicate_imports() {
        let (_directory, meetings, meeting_id) = setup();
        let transcripts = meetings.transcripts();
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for index in 0..2 {
            let service = transcripts.clone();
            let barrier = barrier.clone();
            let meeting_id = meeting_id.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                service.import_txt(
                    &meeting_id,
                    input(
                        &format!("Concurrent {index}"),
                        Some(&format!("concurrent-{index}.txt")),
                        b"Synthetic concurrent content",
                    ),
                )
            }));
        }
        barrier.wait();
        let outcomes = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker should finish"))
            .collect::<TranscriptResult<Vec<_>>>()
            .expect("both imports should return safely");
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, TranscriptImportOutcome::Imported { .. }))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, TranscriptImportOutcome::Duplicate { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn rejects_bad_metadata_state_and_database_failures_without_details() {
        let (_directory, meetings, meeting_id) = setup();
        let transcripts = meetings.transcripts();
        assert_eq!(
            transcripts.list_sources(&meeting_id.to_ascii_uppercase()),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            transcripts.import_txt(
                &meeting_id,
                input("Bad path", Some("C:\\private\\meeting.txt"), b"Synthetic"),
            ),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            transcripts.import_txt(
                &meeting_id,
                input(
                    "C:\\private\\meeting.txt",
                    Some("meeting.txt"),
                    b"Synthetic",
                ),
            ),
            Err(TranscriptError::InvalidInput)
        );
        let mut bad_time = input("Bad time", Some("bad.txt"), b"Synthetic");
        bad_time.base_started_at = Some("2026-02-30T00:00:00Z".to_string());
        assert_eq!(
            transcripts.import_txt(&meeting_id, bad_time),
            Err(TranscriptError::InvalidTimestamp)
        );

        for status in [
            "processing",
            "failed",
            "archived",
            "deleting",
            "delete_failed",
        ] {
            let connection = transcripts
                .database
                .connect()
                .expect("database should connect");
            let deleted_at = matches!(status, "deleting" | "delete_failed")
                .then_some("2026-08-06T00:00:00.000Z");
            connection
                .execute(
                    "UPDATE meetings SET status = ?2, deleted_at = ?3 WHERE id = ?1",
                    rusqlite::params![&meeting_id, status, deleted_at],
                )
                .expect("state should update");
            drop(connection);
            assert_eq!(
                transcripts.import_txt(
                    &meeting_id,
                    input("Blocked", Some("blocked.txt"), b"Synthetic"),
                ),
                Err(TranscriptError::MeetingInvalidState),
                "{status} must fail closed"
            );
        }

        let connection = transcripts
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "UPDATE meetings SET status = 'draft', deleted_at = NULL WHERE id = ?1",
                [&meeting_id],
            )
            .expect("state should reset for persistence failure test");
        drop(connection);

        let connection = transcripts
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute_batch("DROP TABLE transcript_sources;")
            .expect("test should break persistence");
        drop(connection);
        let error = transcripts
            .list_sources(&meeting_id)
            .expect_err("broken database should fail safely");
        assert_eq!(error, TranscriptError::PersistenceFailed);
        assert_eq!(error.to_string(), "transcript_persistence_failed");
    }

    #[test]
    fn validates_utc_leap_days_and_policy_overrides() {
        assert!(validate_utc_iso8601("2024-02-29T23:59:59.123456789Z").is_ok());
        assert_eq!(
            validate_utc_iso8601("2025-02-29T00:00:00Z"),
            Err(TranscriptError::InvalidTimestamp)
        );

        let directory = TestDirectory::new();
        let database = Database::initialize(&directory.0).expect("database should initialize");
        let service = TranscriptService::with_policy(
            database,
            TranscriptPolicy {
                max_pasted_bytes: 4,
                max_file_bytes: 4,
                max_segments: 1,
                max_segment_bytes: 4,
            },
        );
        assert_eq!(service.policy().max_file_bytes, 4);
    }
}
