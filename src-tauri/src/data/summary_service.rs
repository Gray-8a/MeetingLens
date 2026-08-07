use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use rusqlite::{Transaction, TransactionBehavior};
use uuid::Uuid;

use super::database::Database;
use super::error::DataError;
use super::model::{Meeting, MeetingStatus};
use super::repository;
use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    CreateSummaryRevision, CreateSummaryRun, SUMMARY_EVIDENCE_DEFAULT_LIMIT,
    SUMMARY_EVIDENCE_EXCERPT_MAX_BYTES, SUMMARY_EVIDENCE_MAX_LIMIT, SUMMARY_LIST_DEFAULT_LIMIT,
    SUMMARY_LIST_MAX_LIMIT, SUMMARY_MAX_INPUT_BYTES, SUMMARY_MAX_SEGMENTS,
    SUMMARY_RESULT_MAX_BYTES, SummaryActionItem, SummaryClaim, SummaryDecision, SummaryEvidence,
    SummaryEvidencePage, SummaryPolicy, SummaryRun, SummaryRunPage, SummarySnapshotSegment,
    SummaryStatus,
};
use super::summary_provider::{
    DeterministicMockSummaryProvider, SummaryProvider, SummaryProviderOutput,
    SummaryProviderRequest,
};
use super::summary_repository::{self, NewSummary};
use super::transcript_error::TranscriptError;
use super::transcript_repository;

const MAX_RESULT_ITEMS: usize = 500;
const MAX_RESULT_TOTAL_ITEMS: usize = 1_000;
const MAX_RESULT_TEXT_BYTES: usize = 16 * 1024;
const MAX_RESULT_METADATA_BYTES: usize = 255;

fn active_providers() -> &'static Mutex<HashMap<String, Arc<dyn SummaryProvider>>> {
    static ACTIVE_PROVIDERS: OnceLock<Mutex<HashMap<String, Arc<dyn SummaryProvider>>>> =
        OnceLock::new();
    ACTIVE_PROVIDERS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct ActiveProviderExecution {
    run_id: String,
}

impl ActiveProviderExecution {
    fn register(run_id: &str, provider: Arc<dyn SummaryProvider>) -> SummaryResult<Self> {
        let mut providers = active_providers()
            .lock()
            .map_err(|_| SummaryError::Internal)?;
        if providers.contains_key(run_id) {
            return Err(SummaryError::InvalidState);
        }
        providers.insert(run_id.to_string(), provider);
        Ok(Self {
            run_id: run_id.to_string(),
        })
    }
}

impl Drop for ActiveProviderExecution {
    fn drop(&mut self) {
        if let Ok(mut providers) = active_providers().lock() {
            providers.remove(&self.run_id);
        }
    }
}

fn cancel_active_provider(run_id: &str) -> SummaryResult<()> {
    let provider = active_providers()
        .lock()
        .map_err(|_| SummaryError::Internal)?
        .get(run_id)
        .cloned();
    if let Some(provider) = provider {
        provider.cancel(run_id)?;
    }
    Ok(())
}

pub(crate) fn cancel_active_providers(run_ids: &[String]) {
    for run_id in run_ids {
        let _ = cancel_active_provider(run_id);
    }
}

#[derive(Clone, Debug)]
pub struct SummaryService {
    database: Database,
}

impl SummaryService {
    pub(crate) fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn policy(&self) -> SummaryPolicy {
        SummaryPolicy::default()
    }

    pub fn create_run(&self, input: CreateSummaryRun) -> SummaryResult<SummaryRun> {
        validate_uuid(&input.meeting_id)?;
        validate_source_ids(&input.source_ids)?;
        let provider = DeterministicMockSummaryProvider::default();
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        let meeting = eligible_meeting(&transaction, &input.meeting_id)?;
        let id = Uuid::new_v4().to_string();
        let run = self.freeze_run(
            &transaction,
            &meeting,
            &id,
            &Uuid::new_v4().to_string(),
            None,
            1,
            &input.source_ids,
            input.template,
            input.output_language,
            &provider,
        )?;
        transaction.commit().map_err(SummaryError::from)?;
        Ok(run)
    }

    pub fn create_revision(&self, input: CreateSummaryRevision) -> SummaryResult<SummaryRun> {
        validate_uuid(&input.meeting_id)?;
        validate_uuid(&input.summary_id)?;
        let provider = DeterministicMockSummaryProvider::default();
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        let meeting = eligible_meeting(&transaction, &input.meeting_id)?;
        let base =
            summary_repository::get_summary(&transaction, &input.meeting_id, &input.summary_id)?;
        if !base.status.is_terminal() {
            return Err(SummaryError::InvalidState);
        }
        let source_ids = summary_repository::source_ids_for_summary(&transaction, &base.id)?;
        let revision = summary_repository::next_revision(&transaction, &base.series_id)?;
        if base.revision.checked_add(1) != Some(revision) {
            return Err(SummaryError::RevisionConflict);
        }
        let id = Uuid::new_v4().to_string();
        let run = self.freeze_run(
            &transaction,
            &meeting,
            &id,
            &base.series_id,
            Some(&base.id),
            revision,
            &source_ids,
            base.template,
            base.output_language,
            &provider,
        )?;
        transaction.commit().map_err(SummaryError::from)?;
        Ok(run)
    }

    #[allow(clippy::too_many_arguments)]
    fn freeze_run(
        &self,
        transaction: &Transaction<'_>,
        meeting: &Meeting,
        id: &str,
        series_id: &str,
        previous_summary_id: Option<&str>,
        revision: u64,
        source_ids: &[String],
        template: super::summary_model::SummaryTemplate,
        output_language: super::summary_model::SummaryOutputLanguage,
        provider: &dyn SummaryProvider,
    ) -> SummaryResult<SummaryRun> {
        validate_source_ids(source_ids)?;
        let mut sources = Vec::with_capacity(source_ids.len());
        let mut total_segments = 0usize;
        let mut total_bytes = 0usize;
        for source_id in source_ids {
            let source = transcript_repository::get_source(transaction, &meeting.id, source_id)
                .map_err(map_transcript_error)?;
            let (segment_count, input_bytes) =
                summary_repository::source_input_totals(transaction, &meeting.id, source_id)?;
            if segment_count == 0 || input_bytes == 0 {
                return Err(SummaryError::InputInsufficient);
            }
            total_segments = total_segments
                .checked_add(segment_count)
                .ok_or(SummaryError::SegmentLimitExceeded)?;
            total_bytes = total_bytes
                .checked_add(input_bytes)
                .ok_or(SummaryError::InputTooLarge)?;
            if total_segments > SUMMARY_MAX_SEGMENTS {
                return Err(SummaryError::SegmentLimitExceeded);
            }
            if total_bytes > SUMMARY_MAX_INPUT_BYTES {
                return Err(SummaryError::InputTooLarge);
            }
            sources.push(source);
        }
        if total_segments == 0 || total_bytes == 0 {
            return Err(SummaryError::InputInsufficient);
        }

        summary_repository::insert_summary(
            transaction,
            NewSummary {
                id,
                meeting_id: &meeting.id,
                series_id,
                previous_summary_id,
                revision,
                meeting_title_snapshot: &meeting.title,
                meeting_primary_language_snapshot: &meeting.primary_language,
                template,
                output_language,
                provider: provider.provider_name(),
                provider_mode: provider.provider_mode(),
                model: provider.model_name(),
                prompt_version: provider.prompt_version(),
                source_count: sources.len(),
                input_segment_count: total_segments,
                input_bytes: total_bytes,
            },
        )?;
        let created = summary_repository::get_summary(transaction, &meeting.id, id)?;

        let mut global_ordinal = 0u64;
        for (source_ordinal, source) in sources.iter().enumerate() {
            let segments =
                transcript_repository::list_segments(transaction, &meeting.id, &source.id)
                    .map_err(map_transcript_error)?;
            for segment in segments.into_iter().filter(|segment| segment.is_final) {
                let snapshot = SummarySnapshotSegment {
                    evidence_id: Uuid::new_v4().to_string(),
                    summary_id: id.to_string(),
                    meeting_id: meeting.id.clone(),
                    source_id: source.id.clone(),
                    segment_id: segment.id,
                    segment_revision: segment.revision,
                    source_ordinal: u64::try_from(source_ordinal)
                        .map_err(|_| SummaryError::SegmentLimitExceeded)?,
                    segment_ordinal: segment.ordinal,
                    global_ordinal,
                    source_type: source.source_type.as_str().to_string(),
                    source_display_name: source.display_name.clone(),
                    source_language: source.language.clone(),
                    source_base_started_at: source.base_started_at.clone(),
                    speaker_label: segment.speaker_label,
                    speaker_name: segment.speaker_name,
                    start_offset_ms: segment.start_offset_ms,
                    end_offset_ms: segment.end_offset_ms,
                    original_text: segment.original_text,
                    language: segment.language,
                    confidence: segment.confidence,
                    is_final: segment.is_final,
                    snapshot_at: created.snapshot_created_at.clone(),
                };
                summary_repository::insert_snapshot(transaction, &snapshot)?;
                global_ordinal = global_ordinal
                    .checked_add(1)
                    .ok_or(SummaryError::SegmentLimitExceeded)?;
            }
        }
        if usize::try_from(global_ordinal).ok() != Some(total_segments) {
            return Err(SummaryError::PersistenceFailed);
        }
        summary_repository::get_summary(transaction, &meeting.id, id)
    }

    pub fn execute(&self, meeting_id: &str, summary_id: &str) -> SummaryResult<SummaryRun> {
        self.execute_with_provider(
            meeting_id,
            summary_id,
            Arc::new(DeterministicMockSummaryProvider::default()),
        )
    }

    pub fn execute_with_provider(
        &self,
        meeting_id: &str,
        summary_id: &str,
        provider: Arc<dyn SummaryProvider>,
    ) -> SummaryResult<SummaryRun> {
        validate_uuid(meeting_id)?;
        validate_uuid(summary_id)?;
        let (request, _active_provider) = {
            let _guard = self.database.mutation_guard().map_err(map_data_error)?;
            let mut connection = self.database.connect().map_err(map_data_error)?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(SummaryError::from)?;
            eligible_meeting(&transaction, meeting_id)?;
            let run = summary_repository::get_summary(&transaction, meeting_id, summary_id)?;
            if run.provider != provider.provider_name()
                || run.provider_mode != provider.provider_mode()
                || run.model != provider.model_name()
                || run.prompt_version != provider.prompt_version()
            {
                return Err(SummaryError::InvalidState);
            }
            summary_repository::transition_to_running(&transaction, meeting_id, summary_id)?;
            let segments = summary_repository::load_snapshot(&transaction, meeting_id, summary_id)?;
            let (meeting_title, meeting_primary_language) =
                summary_repository::provider_meeting_snapshot(
                    &transaction,
                    meeting_id,
                    summary_id,
                )?;
            let request = SummaryProviderRequest {
                run_id: summary_id.to_string(),
                meeting_id: meeting_id.to_string(),
                meeting_title,
                meeting_primary_language,
                template: run.template,
                output_language: run.output_language,
                segments,
            };
            let active_provider =
                ActiveProviderExecution::register(summary_id, Arc::clone(&provider))?;
            transaction.commit().map_err(SummaryError::from)?;
            (request, active_provider)
        };

        let output = match provider.generate(&request) {
            Ok(output) => output,
            Err(_) => {
                return self.finish_failed(meeting_id, summary_id, SummaryError::ProviderFailed);
            }
        };
        let evidence = match validate_provider_output(&request, &output) {
            Ok(evidence) => evidence,
            Err(error) => return self.finish_failed(meeting_id, summary_id, error),
        };

        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        match summary_repository::complete(
            &transaction,
            meeting_id,
            summary_id,
            &output.content,
            &evidence,
        ) {
            Ok(true) => {
                let completed =
                    summary_repository::get_summary(&transaction, meeting_id, summary_id)?;
                transaction.commit().map_err(SummaryError::from)?;
                Ok(completed)
            }
            Ok(false) => {
                let current = summary_repository::get_summary(&transaction, meeting_id, summary_id);
                transaction.rollback().map_err(SummaryError::from)?;
                let current = current?;
                if current.status == SummaryStatus::Cancelled {
                    Ok(current)
                } else {
                    Err(SummaryError::InvalidState)
                }
            }
            Err(error) => {
                drop(transaction);
                drop(connection);
                drop(_guard);
                self.finish_failed(meeting_id, summary_id, error)
            }
        }
    }

    fn finish_failed(
        &self,
        meeting_id: &str,
        summary_id: &str,
        error: SummaryError,
    ) -> SummaryResult<SummaryRun> {
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        let changed =
            summary_repository::fail_running(&transaction, meeting_id, summary_id, error.code())?;
        let current = summary_repository::get_summary(&transaction, meeting_id, summary_id)?;
        transaction.commit().map_err(SummaryError::from)?;
        if changed {
            Err(error)
        } else if current.status == SummaryStatus::Cancelled {
            Ok(current)
        } else {
            Err(SummaryError::InvalidState)
        }
    }

    pub fn cancel(&self, meeting_id: &str, summary_id: &str) -> SummaryResult<SummaryRun> {
        validate_uuid(meeting_id)?;
        validate_uuid(summary_id)?;
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        let was_running = summary_repository::get_summary(&transaction, meeting_id, summary_id)?
            .status
            == SummaryStatus::Running;
        summary_repository::cancel(&transaction, meeting_id, summary_id)?;
        let cancelled = summary_repository::get_summary(&transaction, meeting_id, summary_id)?;
        transaction.commit().map_err(SummaryError::from)?;
        drop(connection);
        drop(_guard);
        if was_running {
            let _ = cancel_active_provider(summary_id);
        }
        Ok(cancelled)
    }

    pub fn get(&self, meeting_id: &str, summary_id: &str) -> SummaryResult<SummaryRun> {
        validate_uuid(meeting_id)?;
        validate_uuid(summary_id)?;
        summary_repository::get_summary(
            &self.database.connect().map_err(map_data_error)?,
            meeting_id,
            summary_id,
        )
    }

    pub fn list(
        &self,
        meeting_id: &str,
        cursor: Option<&str>,
        limit: Option<u64>,
    ) -> SummaryResult<SummaryRunPage> {
        validate_uuid(meeting_id)?;
        if let Some(cursor) = cursor {
            validate_uuid(cursor)?;
        }
        let limit = validate_limit(limit, SUMMARY_LIST_DEFAULT_LIMIT, SUMMARY_LIST_MAX_LIMIT)?;
        let connection = self.database.connect().map_err(map_data_error)?;
        repository::get(&connection, meeting_id).map_err(map_data_error)?;
        summary_repository::list_summaries(&connection, meeting_id, cursor, limit)
    }

    pub fn list_evidence(
        &self,
        meeting_id: &str,
        summary_id: &str,
        cursor: Option<u64>,
        limit: Option<u64>,
    ) -> SummaryResult<SummaryEvidencePage> {
        validate_uuid(meeting_id)?;
        validate_uuid(summary_id)?;
        let limit = validate_limit(
            limit,
            SUMMARY_EVIDENCE_DEFAULT_LIMIT,
            SUMMARY_EVIDENCE_MAX_LIMIT,
        )?;
        summary_repository::list_evidence(
            &self.database.connect().map_err(map_data_error)?,
            meeting_id,
            summary_id,
            cursor,
            limit,
        )
    }

    pub(crate) fn recover_interrupted(&self) -> SummaryResult<u64> {
        let _guard = self.database.mutation_guard().map_err(map_data_error)?;
        let mut connection = self.database.connect().map_err(map_data_error)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SummaryError::from)?;
        let recovered = summary_repository::recover_interrupted(&transaction)?;
        transaction.commit().map_err(SummaryError::from)?;
        Ok(recovered)
    }
}

fn validate_provider_output(
    request: &SummaryProviderRequest,
    output: &SummaryProviderOutput,
) -> SummaryResult<Vec<SummaryEvidence>> {
    if output.run_id != request.run_id || output.content.schema_version != 1 {
        return Err(SummaryError::ProviderOutputInvalid);
    }
    let serialized =
        serde_json::to_vec(&output.content).map_err(|_| SummaryError::ProviderOutputInvalid)?;
    if serialized.len() > SUMMARY_RESULT_MAX_BYTES {
        return Err(SummaryError::ProviderOutputInvalid);
    }
    let total_items = output
        .content
        .decisions
        .len()
        .checked_add(output.content.action_items.len())
        .and_then(|value| value.checked_add(output.content.risks.len()))
        .and_then(|value| value.checked_add(output.content.open_questions.len()))
        .ok_or(SummaryError::ProviderOutputInvalid)?;
    if output.content.decisions.len() > MAX_RESULT_ITEMS
        || output.content.action_items.len() > MAX_RESULT_ITEMS
        || output.content.risks.len() > MAX_RESULT_ITEMS
        || output.content.open_questions.len() > MAX_RESULT_ITEMS
        || total_items > MAX_RESULT_TOTAL_ITEMS
    {
        return Err(SummaryError::ProviderOutputInvalid);
    }

    let allowlist: HashMap<&str, &SummarySnapshotSegment> = request
        .segments
        .iter()
        .map(|segment| (segment.evidence_id.as_str(), segment))
        .collect();
    validate_claim(&output.content.overview, &allowlist)?;
    for decision in &output.content.decisions {
        validate_decision(decision, &allowlist)?;
    }
    for item in &output.content.action_items {
        validate_action_item(item, &allowlist)?;
    }
    for claim in output
        .content
        .risks
        .iter()
        .chain(output.content.open_questions.iter())
    {
        validate_claim(claim, &allowlist)?;
    }

    let evidence_ids = summary_repository::referenced_evidence_ids(&output.content);
    let mut evidence = Vec::with_capacity(evidence_ids.len());
    for evidence_id in evidence_ids {
        let snapshot = allowlist
            .get(evidence_id.as_str())
            .copied()
            .ok_or(SummaryError::EvidenceInvalid)?;
        evidence.push(SummaryEvidence {
            id: evidence_id,
            summary_id: request.run_id.clone(),
            meeting_id: request.meeting_id.clone(),
            kind: "transcript".to_string(),
            source_id: snapshot.source_id.clone(),
            transcript_segment_id: snapshot.segment_id.clone(),
            segment_revision: snapshot.segment_revision,
            label: format!("Transcript segment {}", snapshot.global_ordinal + 1),
            speaker_snapshot: snapshot
                .speaker_name
                .clone()
                .or_else(|| snapshot.speaker_label.clone()),
            start_offset_ms: snapshot.start_offset_ms,
            end_offset_ms: snapshot.end_offset_ms,
            ordinal: snapshot.segment_ordinal,
            global_ordinal: snapshot.global_ordinal,
            excerpt: truncate_utf8(
                snapshot.original_text.trim(),
                SUMMARY_EVIDENCE_EXCERPT_MAX_BYTES,
            )
            .to_string(),
            source_deleted: false,
            created_at: snapshot.snapshot_at.clone(),
        });
    }
    Ok(evidence)
}

fn validate_claim(
    claim: &SummaryClaim,
    allowlist: &HashMap<&str, &SummarySnapshotSegment>,
) -> SummaryResult<()> {
    validate_result_text(&claim.text, MAX_RESULT_TEXT_BYTES)?;
    validate_evidence_ids(&claim.evidence_ids, allowlist)
}

fn validate_decision(
    decision: &SummaryDecision,
    allowlist: &HashMap<&str, &SummarySnapshotSegment>,
) -> SummaryResult<()> {
    validate_result_text(&decision.text, MAX_RESULT_TEXT_BYTES)?;
    validate_evidence_ids(&decision.evidence_ids, allowlist)
}

fn validate_action_item(
    item: &SummaryActionItem,
    allowlist: &HashMap<&str, &SummarySnapshotSegment>,
) -> SummaryResult<()> {
    validate_result_text(&item.text, MAX_RESULT_TEXT_BYTES)?;
    if let Some(owner) = &item.owner {
        validate_result_text(owner, MAX_RESULT_METADATA_BYTES)?;
    }
    if let Some(due_date) = &item.due_date {
        validate_result_text(due_date, MAX_RESULT_METADATA_BYTES)?;
    }
    validate_evidence_ids(&item.evidence_ids, allowlist)
}

fn validate_evidence_ids(
    evidence_ids: &[String],
    allowlist: &HashMap<&str, &SummarySnapshotSegment>,
) -> SummaryResult<()> {
    if evidence_ids.is_empty() || evidence_ids.len() > SUMMARY_MAX_SEGMENTS {
        return Err(SummaryError::EvidenceInvalid);
    }
    let mut seen = HashSet::with_capacity(evidence_ids.len());
    for evidence_id in evidence_ids {
        if !seen.insert(evidence_id.as_str()) {
            return Err(SummaryError::EvidenceInvalid);
        }
        if !allowlist.contains_key(evidence_id.as_str()) {
            return Err(SummaryError::EvidenceInvalid);
        }
    }
    Ok(())
}

fn validate_result_text(value: &str, max_bytes: usize) -> SummaryResult<()> {
    if value.trim().is_empty()
        || value.len() > max_bytes
        || value.chars().any(|character| {
            let code = u32::from(character);
            (code <= 0x1f && character != '\n' && character != '\t')
                || (0x7f..=0x9f).contains(&code)
        })
    {
        return Err(SummaryError::ProviderOutputInvalid);
    }
    Ok(())
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn eligible_meeting(connection: &rusqlite::Connection, meeting_id: &str) -> SummaryResult<Meeting> {
    let meeting =
        repository::get_including_deleted(connection, meeting_id).map_err(map_data_error)?;
    if meeting.deletion_error_code.is_none()
        && matches!(
            meeting.status,
            MeetingStatus::Draft | MeetingStatus::Recording | MeetingStatus::Ready
        )
    {
        Ok(meeting)
    } else {
        Err(SummaryError::MeetingInvalidState)
    }
}

fn validate_source_ids(source_ids: &[String]) -> SummaryResult<()> {
    if source_ids.is_empty() || source_ids.len() > SUMMARY_MAX_SEGMENTS {
        return Err(SummaryError::InvalidInput);
    }
    let mut seen = HashSet::with_capacity(source_ids.len());
    for source_id in source_ids {
        validate_uuid(source_id)?;
        if !seen.insert(source_id.as_str()) {
            return Err(SummaryError::InvalidInput);
        }
    }
    Ok(())
}

fn validate_uuid(value: &str) -> SummaryResult<()> {
    let parsed = Uuid::parse_str(value).map_err(|_| SummaryError::InvalidInput)?;
    if parsed.to_string() != value {
        return Err(SummaryError::InvalidInput);
    }
    Ok(())
}

fn validate_limit(limit: Option<u64>, default: u64, maximum: u64) -> SummaryResult<u64> {
    let limit = limit.unwrap_or(default);
    if limit == 0 || limit > maximum {
        return Err(SummaryError::InvalidInput);
    }
    Ok(limit)
}

fn map_data_error(error: DataError) -> SummaryError {
    match error {
        DataError::NotFound => SummaryError::MeetingNotFound,
        DataError::InvalidState
        | DataError::MeetingDeleteFailed
        | DataError::MeetingDeletePending => SummaryError::MeetingInvalidState,
        DataError::InvalidId | DataError::InvalidInput => SummaryError::InvalidInput,
        DataError::Database | DataError::MigrationConflict | DataError::StorageUnavailable => {
            SummaryError::PersistenceFailed
        }
        DataError::UnsafePath => SummaryError::Internal,
    }
}

fn map_transcript_error(error: TranscriptError) -> SummaryError {
    match error {
        TranscriptError::SourceNotFound => SummaryError::SourceNotFound,
        TranscriptError::MeetingNotFound => SummaryError::MeetingNotFound,
        TranscriptError::MeetingInvalidState | TranscriptError::InvalidState => {
            SummaryError::MeetingInvalidState
        }
        TranscriptError::InputInsufficient | TranscriptError::Empty => {
            SummaryError::InputInsufficient
        }
        TranscriptError::SegmentLimitExceeded => SummaryError::SegmentLimitExceeded,
        TranscriptError::TooLarge | TranscriptError::SegmentTooLarge => SummaryError::InputTooLarge,
        TranscriptError::PersistenceFailed => SummaryError::PersistenceFailed,
        TranscriptError::Internal => SummaryError::Internal,
        _ => SummaryError::InvalidInput,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::data::service::MeetingService;
    use crate::data::summary_model::{
        SummaryContent, SummaryDecisionStatus, SummaryOutputLanguage, SummaryTemplate,
    };
    use crate::data::summary_provider::{
        MOCK_SUMMARY_MODEL, MOCK_SUMMARY_PROMPT_VERSION, MOCK_SUMMARY_PROVIDER,
    };
    use crate::data::{
        CreateMeeting, TranscriptImportInput, TranscriptImportOutcome, UpdateTranscriptSegment,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(
                std::env::temp_dir()
                    .join(format!("meetinglens-summary-service-{}", Uuid::new_v4())),
            )
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_meeting_and_source(
        meetings: &MeetingService,
    ) -> (String, String, Vec<crate::data::TranscriptSegment>) {
        let meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic summary service meeting".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("meeting should create");
        let outcome = meetings
            .transcripts()
            .import_pasted_text(
                &meeting.id,
                TranscriptImportInput {
                    display_name: "Synthetic transcript".to_string(),
                    original_filename: None,
                    content:
                        b"Speaker 1: Synthetic first point\n\nSpeaker 2: Synthetic second point"
                            .to_vec(),
                    language: Some("en-AU".to_string()),
                    base_started_at: None,
                },
            )
            .expect("transcript should import");
        let source = match outcome {
            TranscriptImportOutcome::Imported { source, .. } => *source,
            TranscriptImportOutcome::Duplicate { .. } => panic!("source should be new"),
        };
        let segments = meetings
            .transcripts()
            .list_segments(&meeting.id, &source.id)
            .expect("segments should list");
        (meeting.id, source.id, segments)
    }

    fn create_run(summaries: &SummaryService, meeting_id: &str, source_id: &str) -> SummaryRun {
        summaries
            .create_run(CreateSummaryRun {
                meeting_id: meeting_id.to_string(),
                source_ids: vec![source_id.to_string()],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            })
            .expect("summary run should create")
    }

    #[test]
    fn completes_from_an_immutable_snapshot_marks_stale_and_preserves_revisions() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, segments) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        assert_eq!(run.status, SummaryStatus::Queued);
        assert_eq!(run.revision, 1);
        assert_eq!(run.input_segment_count, 2);

        let completed = summaries
            .execute(&meeting_id, &run.id)
            .expect("mock should complete");
        assert_eq!(completed.status, SummaryStatus::Completed);
        let original_content = completed.content.clone().expect("content should exist");
        let evidence = summaries
            .list_evidence(&meeting_id, &run.id, None, None)
            .expect("evidence should list");
        assert_eq!(evidence.items.len(), 2);
        assert!(evidence.items.iter().all(|item| !item.source_deleted));

        let first = &segments[0];
        let corrected = meetings
            .transcripts()
            .update_segment(
                &meeting_id,
                &first.id,
                UpdateTranscriptSegment {
                    speaker_name: Some(Some("Synthetic corrected speaker".to_string())),
                    original_text: Some("Synthetic corrected live point".to_string()),
                    expected_revision: first.revision,
                },
            )
            .expect("live segment should update");
        assert_eq!(corrected.revision, first.revision + 1);
        let stale = summaries
            .get(&meeting_id, &run.id)
            .expect("completed run should remain");
        assert!(stale.is_stale);
        assert_eq!(stale.stale_reason.as_deref(), Some("segment_revised"));
        assert_eq!(stale.content.as_ref(), Some(&original_content));

        let revision = summaries
            .create_revision(CreateSummaryRevision {
                meeting_id: meeting_id.clone(),
                summary_id: run.id.clone(),
            })
            .expect("revision should create");
        assert_eq!(revision.series_id, run.series_id);
        assert_eq!(
            revision.previous_summary_id.as_deref(),
            Some(run.id.as_str())
        );
        assert_eq!(revision.revision, 2);
        assert_eq!(revision.status, SummaryStatus::Queued);
        assert_eq!(
            summaries.create_revision(CreateSummaryRevision {
                meeting_id: meeting_id.clone(),
                summary_id: run.id.clone(),
            }),
            Err(SummaryError::RevisionConflict)
        );
        assert_eq!(
            summaries
                .get(&meeting_id, &run.id)
                .expect("revision one should remain")
                .content,
            Some(original_content)
        );
    }

    #[test]
    fn active_source_delete_is_rejected_then_terminal_delete_keeps_minimal_evidence() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        assert_eq!(
            meetings
                .transcripts()
                .delete_source(&meeting_id, &source_id),
            Err(TranscriptError::SourceInUse)
        );

        summaries
            .execute(&meeting_id, &run.id)
            .expect("mock should complete");
        meetings
            .transcripts()
            .delete_source(&meeting_id, &source_id)
            .expect("terminal source should delete");
        let evidence = summaries
            .list_evidence(&meeting_id, &run.id, None, None)
            .expect("minimal evidence should remain");
        assert!(!evidence.items.is_empty());
        assert!(evidence.items.iter().all(|item| item.source_deleted));
        assert!(evidence.items.iter().all(|item| !item.excerpt.is_empty()));
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        let retained_bodies: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM summary_sources \
                 WHERE summary_id = ?1 AND original_text_snapshot IS NOT NULL",
                [&run.id],
                |row| row.get(0),
            )
            .expect("snapshot bodies should count");
        assert_eq!(retained_bodies, 0);
        let stale = summaries
            .get(&meeting_id, &run.id)
            .expect("summary should remain");
        assert!(stale.is_stale);
        assert_eq!(stale.stale_reason.as_deref(), Some("source_deleted"));
    }

    struct UnknownEvidenceProvider;

    impl SummaryProvider for UnknownEvidenceProvider {
        fn provider_name(&self) -> &'static str {
            MOCK_SUMMARY_PROVIDER
        }

        fn provider_mode(&self) -> &'static str {
            crate::data::summary_provider::MOCK_SUMMARY_PROVIDER_MODE
        }

        fn model_name(&self) -> &'static str {
            MOCK_SUMMARY_MODEL
        }

        fn prompt_version(&self) -> &'static str {
            MOCK_SUMMARY_PROMPT_VERSION
        }

        fn generate(
            &self,
            request: &SummaryProviderRequest,
        ) -> SummaryResult<SummaryProviderOutput> {
            Ok(SummaryProviderOutput {
                run_id: request.run_id.clone(),
                content: SummaryContent {
                    schema_version: 1,
                    overview: SummaryClaim {
                        text: "Synthetic invalid evidence output".to_string(),
                        evidence_ids: vec!["00000000-0000-4000-8000-000000000099".to_string()],
                    },
                    decisions: vec![SummaryDecision {
                        text: "Synthetic invalid decision".to_string(),
                        status: SummaryDecisionStatus::Confirmed,
                        evidence_ids: vec!["00000000-0000-4000-8000-000000000099".to_string()],
                    }],
                    action_items: Vec::new(),
                    risks: Vec::new(),
                    open_questions: Vec::new(),
                },
            })
        }
    }

    struct DuplicateEvidenceProvider;

    impl SummaryProvider for DuplicateEvidenceProvider {
        fn provider_name(&self) -> &'static str {
            MOCK_SUMMARY_PROVIDER
        }

        fn provider_mode(&self) -> &'static str {
            crate::data::summary_provider::MOCK_SUMMARY_PROVIDER_MODE
        }

        fn model_name(&self) -> &'static str {
            MOCK_SUMMARY_MODEL
        }

        fn prompt_version(&self) -> &'static str {
            MOCK_SUMMARY_PROMPT_VERSION
        }

        fn generate(
            &self,
            request: &SummaryProviderRequest,
        ) -> SummaryResult<SummaryProviderOutput> {
            let evidence_id = request
                .segments
                .first()
                .expect("test snapshot should not be empty")
                .evidence_id
                .clone();
            Ok(SummaryProviderOutput {
                run_id: request.run_id.clone(),
                content: SummaryContent {
                    schema_version: 1,
                    overview: SummaryClaim {
                        text: "Synthetic duplicate evidence output".to_string(),
                        evidence_ids: vec![evidence_id.clone(), evidence_id],
                    },
                    decisions: Vec::new(),
                    action_items: Vec::new(),
                    risks: Vec::new(),
                    open_questions: Vec::new(),
                },
            })
        }
    }

    #[test]
    fn unknown_evidence_fails_atomically() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        assert_eq!(
            summaries.execute_with_provider(
                &meeting_id,
                &run.id,
                Arc::new(UnknownEvidenceProvider),
            ),
            Err(SummaryError::EvidenceInvalid)
        );
        let failed = summaries
            .get(&meeting_id, &run.id)
            .expect("failed run should persist");
        assert_eq!(failed.status, SummaryStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("summary_evidence_invalid")
        );
        assert!(failed.content.is_none());
        assert!(
            summaries
                .list_evidence(&meeting_id, &run.id, None, None)
                .expect("evidence should list")
                .items
                .is_empty()
        );

        let duplicate_run = create_run(&summaries, &meeting_id, &source_id);
        assert_eq!(
            summaries.execute_with_provider(
                &meeting_id,
                &duplicate_run.id,
                Arc::new(DuplicateEvidenceProvider),
            ),
            Err(SummaryError::EvidenceInvalid)
        );
        let duplicate_failed = summaries
            .get(&meeting_id, &duplicate_run.id)
            .expect("duplicate evidence failure should persist");
        assert_eq!(duplicate_failed.status, SummaryStatus::Failed);
        assert_eq!(
            duplicate_failed.error_code.as_deref(),
            Some("summary_evidence_invalid")
        );
        assert!(duplicate_failed.content.is_none());
    }

    #[test]
    fn evidence_persistence_failure_releases_the_guard_before_marking_failed() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute_batch(
                "
                CREATE TRIGGER synthetic_summary_evidence_failure
                BEFORE INSERT ON evidence_items
                BEGIN
                    SELECT RAISE(ABORT, 'synthetic evidence failure');
                END;
                ",
            )
            .expect("synthetic failure trigger should install");
        drop(connection);

        assert_eq!(
            summaries.execute(&meeting_id, &run.id),
            Err(SummaryError::PersistenceFailed)
        );
        let failed = summaries
            .get(&meeting_id, &run.id)
            .expect("failed run should remain readable");
        assert_eq!(failed.status, SummaryStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("summary_persistence_failed")
        );
        assert!(
            summaries
                .list_evidence(&meeting_id, &run.id, None, None)
                .expect("failed evidence should list")
                .items
                .is_empty()
        );
    }

    #[test]
    fn cancel_is_terminal_and_meeting_delete_cascades_everything() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        let cancelled = summaries
            .cancel(&meeting_id, &run.id)
            .expect("queued run should cancel");
        assert_eq!(cancelled.status, SummaryStatus::Cancelled);
        assert_eq!(
            summaries.execute(&meeting_id, &run.id),
            Err(SummaryError::InvalidState)
        );
        meetings.delete(&meeting_id).expect("meeting should delete");
        assert_eq!(
            summaries.get(&meeting_id, &run.id),
            Err(SummaryError::RunNotFound)
        );
        let connection = rusqlite::Connection::open(meetings.storage_paths().database())
            .expect("database should open");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))
            .expect("summary count should read");
        assert_eq!(count, 0);
    }

    #[test]
    fn eligibility_and_source_validation_cover_all_allowed_and_rejected_states() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();

        assert_eq!(
            create_run(&summaries, &meeting_id, &source_id).status,
            SummaryStatus::Queued
        );
        meetings.start(&meeting_id).expect("meeting should start");
        assert_eq!(
            create_run(&summaries, &meeting_id, &source_id).status,
            SummaryStatus::Queued
        );
        meetings.end(&meeting_id).expect("meeting should end");
        assert_eq!(
            create_run(&summaries, &meeting_id, &source_id).status,
            SummaryStatus::Queued
        );

        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: Vec::new(),
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::InvalidInput)
        );
        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: vec![source_id.clone(), source_id.clone()],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::InvalidInput)
        );

        let (other_meeting_id, other_source_id, _) = create_meeting_and_source(&meetings);
        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: vec![other_source_id.clone()],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::SourceNotFound)
        );
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "UPDATE transcript_segments SET is_final = 0 WHERE meeting_id = ?1",
                [&other_meeting_id],
            )
            .expect("final flags should update");
        drop(connection);
        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id: other_meeting_id,
                source_ids: vec![other_source_id],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::InputInsufficient)
        );

        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "UPDATE meetings SET status = 'deleting', deleted_at = ?2 WHERE id = ?1",
                rusqlite::params![&meeting_id, "2026-08-07T00:00:00.000Z"],
            )
            .expect("meeting should enter deleting");
        drop(connection);
        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: vec![source_id.clone()],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::MeetingInvalidState)
        );
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "UPDATE meetings SET status = 'delete_failed', deletion_error_code = 'synthetic' \
                 WHERE id = ?1",
                [&meeting_id],
            )
            .expect("meeting should enter delete_failed");
        drop(connection);
        assert_eq!(
            summaries.create_run(CreateSummaryRun {
                meeting_id,
                source_ids: vec![source_id],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::MeetingInvalidState)
        );
    }

    #[test]
    fn run_list_uses_bounded_stable_keyset_pagination() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let first = create_run(&summaries, &meeting_id, &source_id);
        let second = summaries
            .create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: vec![source_id.clone()],
                template: SummaryTemplate::Concise,
                output_language: SummaryOutputLanguage::En,
            })
            .expect("second run should create");
        let third = summaries
            .create_run(CreateSummaryRun {
                meeting_id: meeting_id.clone(),
                source_ids: vec![source_id],
                template: SummaryTemplate::ActionsDecisions,
                output_language: SummaryOutputLanguage::En,
            })
            .expect("third run should create");
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        for (id, created_at) in [
            (&first.id, "2026-08-07T00:00:01.000Z"),
            (&second.id, "2026-08-07T00:00:02.000Z"),
            (&third.id, "2026-08-07T00:00:03.000Z"),
        ] {
            connection
                .execute(
                    "UPDATE summaries SET created_at = ?2 WHERE id = ?1",
                    rusqlite::params![id, created_at],
                )
                .expect("created_at should update");
        }
        drop(connection);

        let page_one = summaries
            .list(&meeting_id, None, Some(1))
            .expect("first page should list");
        assert_eq!(page_one.items[0].id, third.id);
        assert_eq!(page_one.next_cursor.as_deref(), Some(third.id.as_str()));
        assert!(page_one.has_more);
        assert_eq!(page_one.total_count, 3);
        let page_two = summaries
            .list(&meeting_id, page_one.next_cursor.as_deref(), Some(1))
            .expect("second page should list");
        assert_eq!(page_two.items[0].id, second.id);
        assert_ne!(page_two.items[0].id, page_one.items[0].id);
        let page_three = summaries
            .list(&meeting_id, page_two.next_cursor.as_deref(), Some(1))
            .expect("third page should list");
        assert_eq!(page_three.items[0].id, first.id);
        assert!(!page_three.has_more);
        assert_eq!(page_three.next_cursor, None);
        assert_eq!(
            summaries.list(
                &meeting_id,
                Some("00000000-0000-4000-8000-000000000099"),
                Some(1)
            ),
            Err(SummaryError::InvalidInput)
        );
        assert_eq!(
            summaries.list(&meeting_id, None, Some(SUMMARY_LIST_MAX_LIMIT + 1)),
            Err(SummaryError::InvalidInput)
        );
    }

    #[test]
    fn aggregate_segment_and_utf8_byte_limits_reject_without_partial_runs() {
        let byte_directory = TestDirectory::new();
        let byte_meetings =
            MeetingService::initialize(&byte_directory.0).expect("service should initialize");
        let (byte_meeting_id, byte_source_id, _) = create_meeting_and_source(&byte_meetings);
        let byte_summaries = byte_meetings.summaries();
        let connection = byte_summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "DELETE FROM transcript_segments WHERE source_id = ?1",
                [&byte_source_id],
            )
            .expect("seed segments should delete");
        connection
            .execute(
                "
                WITH RECURSIVE sequence(value) AS (
                    SELECT 0 UNION ALL SELECT value + 1 FROM sequence WHERE value < 160
                )
                INSERT INTO transcript_segments(
                    id, meeting_id, source_id, ordinal, original_text,
                    is_final, revision, created_at
                )
                SELECT printf('71000000-0000-4000-8000-%012d', value), ?1, ?2, value,
                       lower(hex(zeroblob(32768))), 1, 1, '2026-08-07T00:00:00.000Z'
                FROM sequence
                ",
                rusqlite::params![&byte_meeting_id, &byte_source_id],
            )
            .expect("oversized aggregate should seed");
        drop(connection);
        assert_eq!(
            byte_summaries.create_run(CreateSummaryRun {
                meeting_id: byte_meeting_id.clone(),
                source_ids: vec![byte_source_id],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::InputTooLarge)
        );
        let connection = byte_summaries
            .database
            .connect()
            .expect("database should connect");
        let summary_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM summaries WHERE meeting_id = ?1",
                [&byte_meeting_id],
                |row| row.get(0),
            )
            .expect("summary count should read");
        assert_eq!(summary_count, 0);

        let segment_directory = TestDirectory::new();
        let segment_meetings =
            MeetingService::initialize(&segment_directory.0).expect("service should initialize");
        let (segment_meeting_id, first_source_id, _) = create_meeting_and_source(&segment_meetings);
        let second_source = match segment_meetings
            .transcripts()
            .import_pasted_text(
                &segment_meeting_id,
                TranscriptImportInput {
                    display_name: "Synthetic second cap source".to_string(),
                    original_filename: None,
                    content: b"Synthetic cap seed".to_vec(),
                    language: Some("en-AU".to_string()),
                    base_started_at: None,
                },
            )
            .expect("second source should import")
        {
            TranscriptImportOutcome::Imported { source, .. } => source.id,
            TranscriptImportOutcome::Duplicate { .. } => panic!("source should be new"),
        };
        let segment_summaries = segment_meetings.summaries();
        let connection = segment_summaries
            .database
            .connect()
            .expect("database should connect");
        connection
            .execute(
                "DELETE FROM transcript_segments WHERE meeting_id = ?1",
                [&segment_meeting_id],
            )
            .expect("seed segments should delete");
        for (prefix, source_id) in [("72000000", &first_source_id), ("73000000", &second_source)] {
            let sql = format!(
                "
                WITH RECURSIVE sequence(value) AS (
                    SELECT 0 UNION ALL SELECT value + 1 FROM sequence WHERE value < 10000
                )
                INSERT INTO transcript_segments(
                    id, meeting_id, source_id, ordinal, original_text,
                    is_final, revision, created_at
                )
                SELECT printf('{prefix}-0000-4000-8000-%012d', value), ?1, ?2, value,
                       'x', 1, 1, '2026-08-07T00:00:00.000Z'
                FROM sequence
                "
            );
            connection
                .execute(&sql, rusqlite::params![&segment_meeting_id, source_id])
                .expect("segment cap rows should seed");
        }
        drop(connection);
        assert_eq!(
            segment_summaries.create_run(CreateSummaryRun {
                meeting_id: segment_meeting_id.clone(),
                source_ids: vec![first_source_id, second_source],
                template: SummaryTemplate::Detailed,
                output_language: SummaryOutputLanguage::En,
            }),
            Err(SummaryError::SegmentLimitExceeded)
        );
        let connection = segment_summaries
            .database
            .connect()
            .expect("database should connect");
        let summary_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM summaries WHERE meeting_id = ?1",
                [&segment_meeting_id],
                |row| row.get(0),
            )
            .expect("summary count should read");
        assert_eq!(summary_count, 0);
    }

    #[test]
    fn source_scoped_speaker_rename_marks_stale_without_mutating_the_snapshot() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, segments) = create_meeting_and_source(&meetings);
        let speaker_label = segments[0]
            .speaker_label
            .clone()
            .expect("synthetic speaker label should parse");
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        let completed = summaries
            .execute(&meeting_id, &run.id)
            .expect("summary should complete");
        let original_content = completed.content.clone();
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        let before: (Option<String>, String) = connection
            .query_row(
                "SELECT speaker_name_snapshot, original_text_snapshot FROM summary_sources \
                 WHERE summary_id = ?1 AND global_ordinal = 0",
                [&run.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("snapshot should read");
        drop(connection);
        assert_eq!(before.0, None);
        assert!(
            meetings
                .transcripts()
                .rename_speaker_in_source(
                    &meeting_id,
                    &source_id,
                    &speaker_label,
                    Some("Synthetic confirmed speaker".to_string()),
                )
                .expect("speaker should rename")
                > 0
        );
        let stale = summaries
            .get(&meeting_id, &run.id)
            .expect("stale run should remain");
        assert!(stale.is_stale);
        assert_eq!(stale.stale_reason.as_deref(), Some("segment_revised"));
        assert_eq!(stale.content, original_content);
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        let after: (Option<String>, String) = connection
            .query_row(
                "SELECT speaker_name_snapshot, original_text_snapshot FROM summary_sources \
                 WHERE summary_id = ?1 AND global_ordinal = 0",
                [&run.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("snapshot should remain readable");
        assert_eq!(after, before);
    }

    #[test]
    fn concurrent_revision_allocation_allows_exactly_one_latest_child() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let base = create_run(&summaries, &meeting_id, &source_id);
        summaries
            .execute(&meeting_id, &base.id)
            .expect("base should complete");
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let service = summaries.clone();
            let barrier = Arc::clone(&barrier);
            let meeting_id = meeting_id.clone();
            let summary_id = base.id.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                service.create_revision(CreateSummaryRevision {
                    meeting_id,
                    summary_id,
                })
            }));
        }
        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("revision thread should join"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| { result.as_ref().err() == Some(&SummaryError::RevisionConflict) })
                .count(),
            1
        );
        let created = results
            .into_iter()
            .find_map(Result::ok)
            .expect("one revision");
        assert_eq!(created.revision, 2);
        assert_eq!(
            created.previous_summary_id.as_deref(),
            Some(base.id.as_str())
        );
    }

    #[derive(Clone)]
    struct BlockingProvider {
        entered: Arc<Barrier>,
        cancelled: Arc<AtomicBool>,
    }

    impl SummaryProvider for BlockingProvider {
        fn provider_name(&self) -> &'static str {
            MOCK_SUMMARY_PROVIDER
        }

        fn provider_mode(&self) -> &'static str {
            crate::data::summary_provider::MOCK_SUMMARY_PROVIDER_MODE
        }

        fn model_name(&self) -> &'static str {
            MOCK_SUMMARY_MODEL
        }

        fn prompt_version(&self) -> &'static str {
            MOCK_SUMMARY_PROMPT_VERSION
        }

        fn generate(
            &self,
            _request: &SummaryProviderRequest,
        ) -> SummaryResult<SummaryProviderOutput> {
            self.entered.wait();
            while !self.cancelled.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(SummaryError::ProviderFailed)
        }

        fn cancel(&self, _run_id: &str) -> SummaryResult<()> {
            self.cancelled.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn running_cancel_reaches_the_active_provider_and_is_idempotent() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let run = create_run(&summaries, &meeting_id, &source_id);
        let provider = Arc::new(BlockingProvider {
            entered: Arc::new(Barrier::new(2)),
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        let execute_service = summaries.clone();
        let execute_meeting_id = meeting_id.clone();
        let execute_summary_id = run.id.clone();
        let execute_provider: Arc<dyn SummaryProvider> = provider.clone();
        let handle = std::thread::spawn(move || {
            execute_service.execute_with_provider(
                &execute_meeting_id,
                &execute_summary_id,
                execute_provider,
            )
        });
        provider.entered.wait();
        assert_eq!(
            summaries
                .get(&meeting_id, &run.id)
                .expect("running summary should read")
                .status,
            SummaryStatus::Running
        );
        assert_eq!(
            summaries
                .cancel(&meeting_id, &run.id)
                .expect("running summary should cancel")
                .status,
            SummaryStatus::Cancelled
        );
        assert!(provider.cancelled.load(Ordering::SeqCst));
        assert_eq!(
            summaries
                .cancel(&meeting_id, &run.id)
                .expect("repeated cancellation should be idempotent")
                .status,
            SummaryStatus::Cancelled
        );
        assert_eq!(
            handle
                .join()
                .expect("execution thread should join")
                .expect("cancelled execution should return terminal run")
                .status,
            SummaryStatus::Cancelled
        );
        assert_eq!(
            summaries.execute(&meeting_id, &run.id),
            Err(SummaryError::InvalidState)
        );
    }

    #[test]
    fn provider_failure_is_stable_and_terminal_source_delete_redacts_failed_and_cancelled_inputs() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
        let summaries = meetings.summaries();
        let failed_run = create_run(&summaries, &meeting_id, &source_id);
        assert_eq!(
            summaries.execute_with_provider(
                &meeting_id,
                &failed_run.id,
                Arc::new(DeterministicMockSummaryProvider::failing()),
            ),
            Err(SummaryError::ProviderFailed)
        );
        let failed = summaries
            .get(&meeting_id, &failed_run.id)
            .expect("failed run should persist");
        assert_eq!(failed.status, SummaryStatus::Failed);
        assert_eq!(
            failed.error_code.as_deref(),
            Some("summary_provider_failed")
        );
        assert_eq!(
            summaries.execute(&meeting_id, &failed.id),
            Err(SummaryError::InvalidState)
        );

        let cancelled_run = create_run(&summaries, &meeting_id, &source_id);
        summaries
            .cancel(&meeting_id, &cancelled_run.id)
            .expect("queued run should cancel");
        meetings
            .transcripts()
            .delete_source(&meeting_id, &source_id)
            .expect("terminal source should delete");
        let connection = summaries
            .database
            .connect()
            .expect("database should connect");
        let retained_bodies: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM summary_sources \
                 WHERE summary_id IN (?1, ?2) AND original_text_snapshot IS NOT NULL",
                rusqlite::params![&failed.id, &cancelled_run.id],
                |row| row.get(0),
            )
            .expect("retained bodies should count");
        assert_eq!(retained_bodies, 0);
    }

    #[test]
    fn startup_recovers_running_but_leaves_queued_runs() {
        let directory = TestDirectory::new();
        let (meeting_id, running_id, queued_id) = {
            let meetings =
                MeetingService::initialize(&directory.0).expect("service should initialize");
            let (meeting_id, source_id, _) = create_meeting_and_source(&meetings);
            let summaries = meetings.summaries();
            let running = create_run(&summaries, &meeting_id, &source_id);
            let queued = summaries
                .create_run(CreateSummaryRun {
                    meeting_id: meeting_id.clone(),
                    source_ids: vec![source_id],
                    template: SummaryTemplate::Concise,
                    output_language: SummaryOutputLanguage::En,
                })
                .expect("second series should create");
            let connection = summaries
                .database
                .connect()
                .expect("database should connect");
            connection
                .execute(
                    "UPDATE summaries SET status = 'running', \
                     started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
                    [&running.id],
                )
                .expect("run should simulate interruption");
            (meeting_id, running.id, queued.id)
        };

        let reopened = MeetingService::initialize(&directory.0).expect("service should reopen");
        let summaries = reopened.summaries();
        let recovered = summaries
            .get(&meeting_id, &running_id)
            .expect("running should recover");
        assert_eq!(recovered.status, SummaryStatus::Failed);
        assert_eq!(recovered.error_code.as_deref(), Some("summary_interrupted"));
        assert_eq!(
            summaries
                .get(&meeting_id, &queued_id)
                .expect("queued should remain")
                .status,
            SummaryStatus::Queued
        );
    }

    #[test]
    fn policy_freezes_aggregate_limits_and_zero_network_provider() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0).expect("service should initialize");
        let policy = meetings.summaries().policy();
        assert_eq!(policy.max_segments, 20_000);
        assert_eq!(policy.max_input_bytes, 10 * 1024 * 1024);
        assert_eq!(policy.list_default_limit, 50);
        assert_eq!(policy.list_max_limit, 100);
        assert_eq!(policy.evidence_default_limit, 50);
        assert_eq!(policy.evidence_max_limit, 100);
        assert_eq!(policy.run_purpose, "final");
        assert_eq!(policy.trigger_kind, "manual");
        assert_eq!(policy.provider, MOCK_SUMMARY_PROVIDER);

        let source = include_str!("summary_provider.rs").to_ascii_lowercase();
        for forbidden in [
            "reqwest",
            "tcpstream",
            "std::net",
            "tokio::net",
            "async_std::net",
            "ureq",
            "hyper",
            "http::",
            "websocket",
            "secret_store",
            "sidecar",
            "std::fs",
            "std::env",
            "file::open",
            "openoptions",
            "rusqlite",
            "database",
            "apphandle",
            "std::process",
            "command::new",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
}
