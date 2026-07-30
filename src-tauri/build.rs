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
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("failed to build MeetingLens Tauri configuration");
}
