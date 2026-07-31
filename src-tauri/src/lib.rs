pub mod data;
mod document_commands;
mod meeting_commands;
mod secret_commands;
pub mod secret_store;
mod sidecar;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let meeting_service = data::MeetingService::initialize(app_data_dir)?;
            app.manage(meeting_service);
            app.manage(secret_store::SecretStoreService::production());
            app.manage(sidecar::SidecarRuntime::start());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.state::<sidecar::SidecarRuntime>().shutdown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            sidecar::check_sidecar_health,
            meeting_commands::create_meeting,
            meeting_commands::get_meeting,
            meeting_commands::list_meetings,
            meeting_commands::update_meeting,
            meeting_commands::start_meeting,
            meeting_commands::end_meeting,
            meeting_commands::delete_meeting,
            document_commands::select_and_import_documents,
            document_commands::list_documents,
            document_commands::get_document,
            document_commands::request_delete_document,
            document_commands::retry_delete_document,
            document_commands::get_document_import_policy,
            secret_commands::get_secret_status,
            secret_commands::set_secret,
            secret_commands::delete_secret,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MeetingLens");
}
