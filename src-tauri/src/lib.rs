pub mod data;
mod meeting_commands;
mod sidecar;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let meeting_service = data::MeetingService::initialize(app_data_dir)?;
            app.manage(meeting_service);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running MeetingLens");
}
