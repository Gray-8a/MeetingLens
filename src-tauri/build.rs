const APP_COMMANDS: &[&str] = &[
    "check_sidecar_health",
    "create_meeting",
    "get_meeting",
    "list_meetings",
    "update_meeting",
    "start_meeting",
    "end_meeting",
    "delete_meeting",
    "get_secret_status",
    "set_secret",
    "delete_secret",
    "create_meeting_from_pasted_transcript",
    "select_transcript_and_create_meeting",
    "add_pasted_transcript_to_meeting",
    "select_and_add_transcript_to_meeting",
    "list_transcript_sources",
    "get_transcript_source",
    "list_transcript_segments",
    "list_transcript_speaker_groups",
    "rename_speaker_in_source",
    "update_transcript_segment",
    "delete_transcript_source",
    "get_transcript_import_policy",
    "get_summary_policy",
    "create_summary_run",
    "create_summary_revision",
    "execute_summary_run",
    "get_summary_run",
    "list_summary_runs",
    "cancel_summary_run",
    "list_summary_evidence",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("failed to build MeetingLens Tauri configuration");
}
