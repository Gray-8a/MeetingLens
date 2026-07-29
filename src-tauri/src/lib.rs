mod sidecar;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![sidecar::check_sidecar_health])
        .run(tauri::generate_context!())
        .expect("error while running MeetingLens");
}
