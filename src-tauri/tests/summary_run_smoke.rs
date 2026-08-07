#![cfg(debug_assertions)]

use std::path::{Path, PathBuf};

use meetinglens_lib::data::{
    CreateSummaryRevision, CreateSummaryRun, DataError, MeetingService, SummaryOutputLanguage,
    SummaryStatus, SummaryTemplate, TranscriptError, TranscriptImportInput,
    TranscriptImportOutcome, UpdateTranscriptSegment, seed_summary_smoke_v3,
};
use rusqlite::Connection;
use uuid::Uuid;

struct SmokeWorkspace {
    app_data: PathBuf,
    cleaned: bool,
}

impl SmokeWorkspace {
    fn new() -> Self {
        Self {
            app_data: std::env::temp_dir().join(format!(
                "meetinglens-native-summary-smoke-{}",
                Uuid::new_v4()
            )),
            cleaned: false,
        }
    }

    fn cleanup(mut self) {
        self.cleaned = true;
        remove_smoke_directory(&self.app_data, "meetinglens-native-summary-smoke-");
        assert!(!self.app_data.exists());
    }
}

impl Drop for SmokeWorkspace {
    fn drop(&mut self) {
        if !self.cleaned {
            remove_smoke_directory(&self.app_data, "meetinglens-native-summary-smoke-");
        }
    }
}

#[test]
fn native_summary_run_core_lifecycle_smoke() {
    let workspace = SmokeWorkspace::new();
    let v3 = seed_summary_smoke_v3(&workspace.app_data)
        .expect("synthetic schema-v3 app data should seed");

    let meetings =
        MeetingService::initialize(&workspace.app_data).expect("schema v3 should upgrade to v4");
    assert_eq!(
        meetings
            .get(&v3.meeting_id)
            .expect("v3 meeting should survive")
            .status
            .as_str(),
        "draft"
    );
    assert_schema_and_integrity(&meetings, 4);

    let summaries = meetings.summaries();
    let policy = summaries.policy();
    assert_eq!(policy.provider, "deterministic_mock");
    assert_eq!(policy.max_segments, 20_000);
    assert_eq!(policy.max_input_bytes, 10 * 1024 * 1024);

    let first = summaries
        .create_run(CreateSummaryRun {
            meeting_id: v3.meeting_id.clone(),
            source_ids: vec![v3.source_id.clone()],
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
        })
        .expect("first summary should freeze");
    assert_eq!(first.status, SummaryStatus::Queued);
    assert_eq!(first.revision, 1);
    assert_eq!(first.input_segment_count, 1);

    let completed = summaries
        .execute(&v3.meeting_id, &first.id)
        .expect("deterministic provider should complete");
    assert_eq!(completed.status, SummaryStatus::Completed);
    assert!(completed.content.is_some());
    let first_content = completed.content.clone();
    let evidence = summaries
        .list_evidence(&v3.meeting_id, &first.id, None, None)
        .expect("completed evidence should list");
    assert!(!evidence.items.is_empty());
    assert!(evidence.items.iter().all(|item| !item.source_deleted));

    let corrected = meetings
        .transcripts()
        .update_segment(
            &v3.meeting_id,
            &v3.segment_id,
            UpdateTranscriptSegment {
                speaker_name: Some(Some("Synthetic corrected speaker".to_string())),
                original_text: Some("Synthetic corrected transcript point".to_string()),
                expected_revision: 1,
            },
        )
        .expect("segment correction should persist");
    assert_eq!(corrected.revision, 2);
    let stale = summaries
        .get(&v3.meeting_id, &first.id)
        .expect("completed run should remain readable");
    assert!(stale.is_stale);
    assert_eq!(stale.stale_reason.as_deref(), Some("segment_revised"));
    assert_eq!(stale.content, first_content);
    assert_snapshot_revision(
        &meetings,
        &first.id,
        1,
        "Synthetic legacy v3 transcript point",
    );

    let second = summaries
        .create_revision(CreateSummaryRevision {
            meeting_id: v3.meeting_id.clone(),
            summary_id: first.id.clone(),
        })
        .expect("revision should refreeze current transcript state");
    assert_eq!(second.series_id, first.series_id);
    assert_eq!(
        second.previous_summary_id.as_deref(),
        Some(first.id.as_str())
    );
    assert_eq!(second.revision, 2);
    assert_snapshot_revision(
        &meetings,
        &second.id,
        2,
        "Synthetic corrected transcript point",
    );
    let second = summaries
        .execute(&v3.meeting_id, &second.id)
        .expect("second revision should complete");
    assert_eq!(second.status, SummaryStatus::Completed);

    let cancelled_revision = summaries
        .create_revision(CreateSummaryRevision {
            meeting_id: v3.meeting_id.clone(),
            summary_id: second.id.clone(),
        })
        .expect("third revision should queue");
    let cancelled_revision = summaries
        .cancel(&v3.meeting_id, &cancelled_revision.id)
        .expect("queued revision should cancel");
    assert_eq!(cancelled_revision.status, SummaryStatus::Cancelled);

    let active = summaries
        .create_run(CreateSummaryRun {
            meeting_id: v3.meeting_id.clone(),
            source_ids: vec![v3.source_id.clone()],
            template: SummaryTemplate::Concise,
            output_language: SummaryOutputLanguage::Bilingual,
        })
        .expect("active source consumer should queue");
    assert_eq!(
        meetings
            .transcripts()
            .delete_source(&v3.meeting_id, &v3.source_id),
        Err(TranscriptError::SourceInUse)
    );
    summaries
        .cancel(&v3.meeting_id, &active.id)
        .expect("active consumer should cancel");
    meetings
        .transcripts()
        .delete_source(&v3.meeting_id, &v3.source_id)
        .expect("terminal source consumers should be redacted");
    assert_eq!(
        meetings
            .transcripts()
            .get_source(&v3.meeting_id, &v3.source_id),
        Err(TranscriptError::SourceNotFound)
    );
    assert_terminal_source_redaction(&meetings, &v3.source_id);

    let recovery_source = match meetings
        .transcripts()
        .import_pasted_text(
            &v3.meeting_id,
            TranscriptImportInput {
                display_name: "Synthetic recovery source".to_string(),
                original_filename: None,
                content: b"Synthetic recovery transcript point".to_vec(),
                language: Some("en-AU".to_string()),
                base_started_at: None,
            },
        )
        .expect("recovery source should import")
    {
        TranscriptImportOutcome::Imported { source, .. } => source.id,
        TranscriptImportOutcome::Duplicate { .. } => panic!("expected a new synthetic source"),
    };
    let interrupted = summaries
        .create_run(CreateSummaryRun {
            meeting_id: v3.meeting_id.clone(),
            source_ids: vec![recovery_source.clone()],
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
        })
        .expect("interrupted run should queue");
    let queued = summaries
        .create_run(CreateSummaryRun {
            meeting_id: v3.meeting_id.clone(),
            source_ids: vec![recovery_source.clone()],
            template: SummaryTemplate::ActionsDecisions,
            output_language: SummaryOutputLanguage::Zh,
        })
        .expect("queued run should persist");
    mark_running_for_restart(&meetings, &interrupted.id);
    drop(summaries);
    drop(meetings);

    let reopened = MeetingService::initialize(&workspace.app_data)
        .expect("restart should recover interrupted summaries");
    let summaries = reopened.summaries();
    let recovered = summaries
        .get(&v3.meeting_id, &interrupted.id)
        .expect("interrupted run should remain readable");
    assert_eq!(recovered.status, SummaryStatus::Failed);
    assert_eq!(recovered.error_code.as_deref(), Some("summary_interrupted"));
    assert_eq!(
        summaries
            .get(&v3.meeting_id, &queued.id)
            .expect("queued run should remain queued")
            .status,
        SummaryStatus::Queued
    );
    assert_schema_and_integrity(&reopened, 4);

    drop(summaries);
    reopened
        .delete(&v3.meeting_id)
        .expect("meeting deletion should cancel active summaries and cascade");
    assert_eq!(reopened.get(&v3.meeting_id), Err(DataError::NotFound));
    assert_summary_rows(&reopened, 0);
    assert_schema_and_integrity(&reopened, 4);
    drop(reopened);
    workspace.cleanup();
}

fn assert_snapshot_revision(
    meetings: &MeetingService,
    summary_id: &str,
    expected_revision: i64,
    expected_text: &str,
) {
    let connection =
        Connection::open(meetings.storage_paths().database()).expect("smoke database should open");
    let (revision, body): (i64, Option<String>) = connection
        .query_row(
            "SELECT segment_revision, original_text_snapshot FROM summary_sources WHERE summary_id = ?1",
            [summary_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("snapshot should inspect");
    assert_eq!(revision, expected_revision);
    assert!(body.as_deref() == Some(expected_text));
}

fn assert_terminal_source_redaction(meetings: &MeetingService, source_id: &str) {
    let connection =
        Connection::open(meetings.storage_paths().database()).expect("smoke database should open");
    let remaining_bodies: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM summary_sources WHERE source_id_snapshot = ?1 AND original_text_snapshot IS NOT NULL",
            [source_id],
            |row| row.get(0),
        )
        .expect("snapshot redaction should inspect");
    let deleted_evidence: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM evidence_items WHERE source_id_snapshot = ?1 AND source_deleted = 1 AND excerpt IS NOT NULL",
            [source_id],
            |row| row.get(0),
        )
        .expect("minimal evidence should inspect");
    assert_eq!(remaining_bodies, 0);
    assert!(deleted_evidence > 0);
}

fn mark_running_for_restart(meetings: &MeetingService, summary_id: &str) {
    let connection =
        Connection::open(meetings.storage_paths().database()).expect("smoke database should open");
    let changed = connection
        .execute(
            "UPDATE summaries SET status = 'running', started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1 AND status = 'queued'",
            [summary_id],
        )
        .expect("synthetic interrupted state should seed");
    assert_eq!(changed, 1);
}

fn assert_summary_rows(meetings: &MeetingService, expected: i64) {
    let connection =
        Connection::open(meetings.storage_paths().database()).expect("smoke database should open");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))
        .expect("summary rows should inspect");
    assert_eq!(count, expected);
}

fn assert_schema_and_integrity(meetings: &MeetingService, expected_version: i64) {
    let connection =
        Connection::open(meetings.storage_paths().database()).expect("smoke database should open");
    let schema_version: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("schema version should read");
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check should run");
    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign keys should check");
    let summary_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('summaries', 'summary_sources', 'evidence_items')",
            [],
            |row| row.get(0),
        )
        .expect("summary tables should inspect");
    assert_eq!(schema_version, expected_version);
    assert_eq!(integrity, "ok");
    assert_eq!(foreign_key_violations, 0);
    assert_eq!(summary_tables, 3);
}

fn remove_smoke_directory(path: &Path, expected_prefix: &str) {
    if !path.exists() {
        return;
    }
    let resolved = path
        .canonicalize()
        .expect("smoke cleanup path should resolve");
    let name = resolved
        .file_name()
        .and_then(|value| value.to_str())
        .expect("smoke cleanup path should have a UTF-8 name");
    assert!(name.starts_with(expected_prefix));
    let temp_root = std::env::temp_dir()
        .canonicalize()
        .expect("system temp directory should resolve");
    assert!(resolved.starts_with(temp_root));
    std::fs::remove_dir_all(&resolved).expect("smoke directory should clean up");
}
