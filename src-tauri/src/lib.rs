pub mod data;
mod sidecar;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let meeting_service = data::MeetingService::initialize(app_data_dir)?;
            app.manage(meeting_service);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![sidecar::check_sidecar_health])
        .run(tauri::generate_context!())
        .expect("error while running MeetingLens");
}
