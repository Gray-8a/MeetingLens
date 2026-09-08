#![cfg(debug_assertions)]

use std::path::{Path, PathBuf};

use meetinglens_lib::data::{
    MeetingService, TranscriptError, TranscriptImportInput, TranscriptImportOutcome,
    TranscriptSource, TranscriptSourceType, UpdateTranscriptSegment, seed_transcript_smoke_v2,
};
use rusqlite::Connection;
use uuid::Uuid;

struct SmokeWorkspace {
    app_data: PathBuf,
    sources: PathBuf,
    cleaned: bool,
}

impl SmokeWorkspace {
    fn new() -> Self {
        let suffix = Uuid::new_v4();
        Self {
            app_data: std::env::temp_dir()
                .join(format!("meetinglens-native-transcript-smoke-{suffix}")),
            sources: std::env::temp_dir()
                .join(format!("meetinglens-native-transcript-source-{suffix}")),
            cleaned: false,
        }
    }

    fn write_source(&self, name: &str, content: &[u8]) -> PathBuf {
        std::fs::create_dir_all(&self.sources).expect("smoke source directory should create");
        let path = self.sources.join(name);
        std::fs::write(&path, content).expect("smoke source should write");
        path
    }

    fn cleanup(mut self) {
        self.cleaned = true;
        remove_smoke_directory(&self.app_data, "meetinglens-native-transcript-smoke-");
        remove_smoke_directory(&self.sources, "meetinglens-native-transcript-source-");
        assert!(!self.app_data.exists());
        assert!(!self.sources.exists());
    }
}

impl Drop for SmokeWorkspace {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        remove_smoke_directory(&self.app_data, "meetinglens-native-transcript-smoke-");
        remove_smoke_directory(&self.sources, "meetinglens-native-transcript-source-");
    }
}

#[test]
fn native_transcript_data_lifecycle_smoke() {
    let workspace = SmokeWorkspace::new();
    let v2_seed = seed_transcript_smoke_v2(&workspace.app_data)
        .expect("synthetic schema-v2 app data should seed");

    let txt_content =
        b"\xEF\xBB\xBFAlice: Synthetic TXT point\r\n\r\nhttps://example.test: retained as text\r\n";
    let txt_source_path = workspace.write_source("synthetic-transcript.txt", txt_content);

    let meetings =
        MeetingService::initialize(&workspace.app_data).expect("schema v2 should upgrade to v4");
    assert_eq!(
        meetings
            .get(&v2_seed.meeting_id)
            .expect("v2 meeting should survive the upgrade")
            .title,
        "Synthetic transcript smoke upgrade"
    );
    assert_schema_and_integrity(&meetings, 7);

    let transcripts = meetings.transcripts();
    let (pasted, pasted_count) = imported(
        transcripts
            .import_pasted_text(
                &v2_seed.meeting_id,
                transcript_input(
                    "Synthetic pasted transcript",
                    None,
                    "Alice: First synthetic point\n\n张三：第二个合成要点".as_bytes(),
                ),
            )
            .expect("pasted transcript should import"),
    );
    assert_eq!(pasted.source_type, TranscriptSourceType::PastedText);
    assert_eq!(pasted_count, 2);

    let (txt, txt_count) = imported(
        transcripts
            .import_txt(
                &v2_seed.meeting_id,
                transcript_input(
                    "Synthetic TXT transcript",
                    Some("synthetic-transcript.txt"),
                    &std::fs::read(&txt_source_path).expect("TXT source should read"),
                ),
            )
            .expect("TXT transcript should import"),
    );
    assert_eq!(txt.source_type, TranscriptSourceType::ImportedTxt);
    assert_eq!(txt_count, 2);

    let (srt, srt_count) = imported(
        transcripts
            .import_srt(
                &v2_seed.meeting_id,
                transcript_input(
                    "Synthetic SRT transcript",
                    Some("synthetic-transcript.srt"),
                    b"1\r\n00:00:01,250 --> 00:00:02,500\r\nSpeaker S: Synthetic SRT point\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nSecond SRT point\r\n",
                ),
            )
            .expect("SRT transcript should import"),
    );
    assert_eq!(srt.source_type, TranscriptSourceType::ImportedSrt);
    assert_eq!(srt_count, 2);

    let (vtt, vtt_count) = imported(
        transcripts
            .import_vtt(
                &v2_seed.meeting_id,
                transcript_input(
                    "Synthetic VTT transcript",
                    Some("synthetic-transcript.vtt"),
                    b"WEBVTT\n\n00:01.000 --> 00:02.250 align:start\n<v Speaker V>Synthetic VTT point</v>\n",
                ),
            )
            .expect("VTT transcript should import"),
    );
    assert_eq!(vtt.source_type, TranscriptSourceType::ImportedVtt);
    assert_eq!(vtt_count, 1);

    assert_eq!(
        transcripts
            .import_txt(
                &v2_seed.meeting_id,
                transcript_input(
                    "Synthetic duplicate TXT",
                    Some("synthetic-duplicate.txt"),
                    b"Alice: Synthetic TXT point\n\nhttps://example.test: retained as text\n",
                ),
            )
            .expect("normalized duplicate should return a stable outcome"),
        TranscriptImportOutcome::Duplicate {
            source_id: txt.id.clone(),
        }
    );
    assert_eq!(
        transcripts
            .list_sources(&v2_seed.meeting_id)
            .expect("sources should list")
            .len(),
        4
    );

    let pasted_segments = transcripts
        .list_segments(&v2_seed.meeting_id, &pasted.id)
        .expect("pasted segments should list");
    assert_eq!(pasted_segments[0].speaker_label.as_deref(), Some("Alice"));
    assert_eq!(pasted_segments[1].speaker_label.as_deref(), Some("张三"));
    assert!(pasted_segments.iter().all(|segment| {
        segment.start_offset_ms.is_none()
            && segment.end_offset_ms.is_none()
            && segment.is_final
            && segment.revision == 1
            && segment.translated_text.is_none()
            && segment.confidence.is_none()
    }));

    let txt_segments = transcripts
        .list_segments(&v2_seed.meeting_id, &txt.id)
        .expect("TXT segments should list");
    assert_eq!(txt_segments[0].speaker_label.as_deref(), Some("Alice"));
    assert_eq!(txt_segments[1].speaker_label, None);

    let srt_segments = transcripts
        .list_segments(&v2_seed.meeting_id, &srt.id)
        .expect("SRT segments should list");
    assert_eq!(srt_segments[0].ordinal, 0);
    assert_eq!(srt_segments[0].start_offset_ms, Some(1_250));
    assert_eq!(srt_segments[0].end_offset_ms, Some(2_500));
    assert_eq!(srt_segments[0].speaker_label.as_deref(), Some("Speaker S"));

    let vtt_segments = transcripts
        .list_segments(&v2_seed.meeting_id, &vtt.id)
        .expect("VTT segments should list");
    assert_eq!(vtt_segments[0].start_offset_ms, Some(1_000));
    assert_eq!(vtt_segments[0].end_offset_ms, Some(2_250));
    assert_eq!(vtt_segments[0].speaker_label.as_deref(), Some("Speaker V"));

    let corrected = transcripts
        .update_segment(
            &v2_seed.meeting_id,
            &pasted_segments[0].id,
            UpdateTranscriptSegment {
                speaker_name: Some(Some("Synthetic facilitator".to_string())),
                original_text: Some("Corrected synthetic opening point".to_string()),
                expected_revision: 1,
            },
        )
        .expect("segment correction should persist");
    assert_eq!(corrected.revision, 2);
    assert_eq!(
        corrected.speaker_name.as_deref(),
        Some("Synthetic facilitator")
    );
    assert!(corrected.corrected_at.is_some());

    let selected_source_ids = vec![
        vtt.id.clone(),
        pasted.id.clone(),
        srt.id.clone(),
        txt.id.clone(),
    ];
    let summary = transcripts
        .build_summary_input(&v2_seed.meeting_id, &selected_source_ids)
        .expect("summary input should build");
    let summary_again = transcripts
        .build_summary_input(&v2_seed.meeting_id, &selected_source_ids)
        .expect("summary input ordering should be stable");
    assert_eq!(summary, summary_again);
    assert_eq!(
        summary.len(),
        pasted_count + txt_count + srt_count + vtt_count
    );
    assert!(summary.iter().all(|segment| segment.is_final));
    assert!(summary.iter().any(|segment| {
        segment.segment_id == corrected.id
            && segment.revision == 2
            && segment.original_text == "Corrected synthetic opening point"
    }));
    let serialized_summary =
        serde_json::to_string(&summary).expect("summary DTO should serialize safely");
    assert!(!serialized_summary.contains(&workspace.app_data.to_string_lossy().to_string()));
    assert!(!serialized_summary.contains(&workspace.sources.to_string_lossy().to_string()));
    assert!(!serialized_summary.contains("synthetic-transcript.txt"));

    drop(transcripts);
    drop(meetings);

    let reopened =
        MeetingService::initialize(&workspace.app_data).expect("storage should survive restart");
    let transcripts = reopened.transcripts();
    assert_eq!(
        transcripts
            .get_segment(&v2_seed.meeting_id, &corrected.id)
            .expect("corrected segment should persist")
            .revision,
        2
    );
    assert_eq!(
        transcripts
            .build_summary_input(&v2_seed.meeting_id, &selected_source_ids)
            .expect("summary input should survive restart"),
        summary
    );

    transcripts
        .delete_source(&v2_seed.meeting_id, &txt.id)
        .expect("TXT source should delete with its segments");
    assert_eq!(
        transcripts.get_source(&v2_seed.meeting_id, &txt.id),
        Err(TranscriptError::SourceNotFound)
    );
    assert_eq!(
        transcripts.list_segments(&v2_seed.meeting_id, &txt.id),
        Err(TranscriptError::SourceNotFound)
    );
    assert_eq!(
        std::fs::read(&txt_source_path).expect("original TXT source should remain"),
        txt_content
    );
    assert_schema_and_integrity(&reopened, 7);

    drop(transcripts);
    drop(reopened);
    workspace.cleanup();
}

fn transcript_input(
    display_name: &str,
    original_filename: Option<&str>,
    content: &[u8],
) -> TranscriptImportInput {
    TranscriptImportInput {
        display_name: display_name.to_string(),
        original_filename: original_filename.map(str::to_string),
        content: content.to_vec(),
        language: Some("en-AU".to_string()),
        base_started_at: Some("2026-08-06T00:00:00.000Z".to_string()),
    }
}

fn imported(outcome: TranscriptImportOutcome) -> (TranscriptSource, usize) {
    match outcome {
        TranscriptImportOutcome::Imported {
            source,
            segment_count,
        } => (*source, segment_count),
        TranscriptImportOutcome::Duplicate { .. } => panic!("expected a new transcript source"),
    }
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
    let transcript_tables: i64 = connection
        .query_row(
            "
            SELECT COUNT(*) FROM sqlite_master
            WHERE type = 'table' AND name IN ('transcript_sources', 'transcript_segments')
            ",
            [],
            |row| row.get(0),
        )
        .expect("transcript tables should inspect");

    assert_eq!(schema_version, expected_version);
    assert_eq!(integrity, "ok");
    assert_eq!(transcript_tables, 2);
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
