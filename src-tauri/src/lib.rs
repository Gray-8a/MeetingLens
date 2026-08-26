mod audio_transcription_commands;
mod audio_transcription_error;
mod audio_transcription_runtime;
mod audio_transcription_worker;
pub mod data;
pub mod docling_worker;
mod document_commands;
mod meeting_commands;
mod secret_commands;
pub mod secret_store;
mod sidecar;
mod summary_commands;
mod summary_runtime;

#[cfg(debug_assertions)]
pub fn run_deepseek_summary_offline_smoke() {
    summary_runtime::offline_smoke::run();
}

#[cfg(debug_assertions)]
pub fn run_local_audio_transcription_smoke(
    smoke_root: impl AsRef<std::path::Path>,
    input_path: impl AsRef<std::path::Path>,
) -> Result<(), &'static str> {
    audio_transcription_runtime::run_local_audio_smoke(smoke_root.as_ref(), input_path.as_ref())
}
mod transcript_commands;

use tauri::Manager;

#[cfg(debug_assertions)]
const SUMMARY_SMOKE_APP_DATA_ENV: &str = "MEETINGLENS_SUMMARY_RUN_CORE_SMOKE_ROOT";
#[cfg(debug_assertions)]
const SUMMARY_SMOKE_APP_DATA_PREFIX: &str = "meetinglens-summary-run-core-smoke-";
#[cfg(debug_assertions)]
const SUMMARY_SMOKE_ENABLED_ENV: &str = "VITE_SUMMARY_RUN_CORE_SMOKE";
#[cfg(debug_assertions)]
const SUMMARY_SMOKE_IDENTIFIER_PREFIX: &str = "com.gray.meetinglens.summarysmoke.";

#[cfg(debug_assertions)]
fn summary_smoke_app_data_override() -> std::io::Result<Option<std::path::PathBuf>> {
    let enabled = std::env::var_os(SUMMARY_SMOKE_ENABLED_ENV).is_some_and(|value| value == "1");
    let candidate = std::env::var_os(SUMMARY_SMOKE_APP_DATA_ENV);
    match (enabled, candidate) {
        (false, None) => Ok(None),
        (true, Some(candidate)) => {
            validate_summary_smoke_app_data_dir(std::path::Path::new(&candidate)).map(Some)
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "summary smoke mode and its isolated app-data root must be configured together",
        )),
    }
}

#[cfg(debug_assertions)]
fn validate_summary_smoke_app_data_dir(
    candidate: &std::path::Path,
) -> std::io::Result<std::path::PathBuf> {
    let temporary_root = std::fs::canonicalize(std::env::temp_dir())?;
    let candidate = std::fs::canonicalize(candidate)?;
    let smoke_id = candidate
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(SUMMARY_SMOKE_APP_DATA_PREFIX))
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .filter(|parsed| {
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix(SUMMARY_SMOKE_APP_DATA_PREFIX))
                .is_some_and(|value| parsed.to_string() == value)
        });
    if candidate.parent() != Some(temporary_root.as_path()) || smoke_id.is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "summary smoke app-data root is not an approved temporary directory",
        ));
    }
    Ok(candidate)
}

#[cfg(debug_assertions)]
fn validate_summary_smoke_identifier(
    app_data_dir: &std::path::Path,
    identifier: &str,
) -> std::io::Result<()> {
    let root_id = app_data_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(SUMMARY_SMOKE_APP_DATA_PREFIX));
    let identifier_id = identifier.strip_prefix(SUMMARY_SMOKE_IDENTIFIER_PREFIX);
    if root_id.is_none() || root_id != identifier_id {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "summary smoke requires a matching isolated application identifier",
        ));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            let (app_data_dir, summary_smoke_mode) = match summary_smoke_app_data_override()? {
                Some(app_data_dir) => {
                    validate_summary_smoke_identifier(&app_data_dir, &app.config().identifier)?;
                    (app_data_dir, true)
                }
                None => (app.path().app_data_dir()?, false),
            };
            #[cfg(not(debug_assertions))]
            let app_data_dir = app.path().app_data_dir()?;
            let meeting_service = data::MeetingService::initialize(app_data_dir)?;
            let secret_store = secret_store::SecretStoreService::production();
            #[cfg(debug_assertions)]
            let summary_provider: std::sync::Arc<dyn data::SummaryProvider> = if summary_smoke_mode
            {
                std::sync::Arc::new(
                    data::DeterministicMockSummaryProvider::blocking_on_nth_execution(3),
                )
            } else {
                let key_source: std::sync::Arc<dyn data::DeepSeekApiKeySource> =
                    std::sync::Arc::new(secret_store.clone());
                std::sync::Arc::new(data::DeepSeekSummaryProvider::production(key_source)?)
            };
            #[cfg(not(debug_assertions))]
            let summary_provider: std::sync::Arc<dyn data::SummaryProvider> = {
                let key_source: std::sync::Arc<dyn data::DeepSeekApiKeySource> =
                    std::sync::Arc::new(secret_store.clone());
                std::sync::Arc::new(data::DeepSeekSummaryProvider::production(key_source)?)
            };
            let summary_runtime = summary_runtime::SummaryRuntimeService::with_provider(
                meeting_service.clone(),
                secret_store.clone(),
                summary_provider,
            );
            let audio_transcription_runtime =
                audio_transcription_runtime::AudioTranscriptionRuntime::initialize(
                    meeting_service.clone(),
                );
            app.manage(meeting_service);
            app.manage(secret_store);
            app.manage(summary_runtime);
            app.manage(audio_transcription_runtime);
            app.manage(sidecar::SidecarRuntime::start());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window
                    .state::<audio_transcription_runtime::AudioTranscriptionRuntime>()
                    .shutdown();
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
            transcript_commands::create_meeting_from_pasted_transcript,
            transcript_commands::select_transcript_and_create_meeting,
            transcript_commands::add_pasted_transcript_to_meeting,
            transcript_commands::select_and_add_transcript_to_meeting,
            transcript_commands::list_transcript_sources,
            transcript_commands::get_transcript_source,
            transcript_commands::list_transcript_segments,
            transcript_commands::list_transcript_speaker_groups,
            transcript_commands::rename_speaker_in_source,
            transcript_commands::update_transcript_segment,
            transcript_commands::delete_transcript_source,
            transcript_commands::get_transcript_import_policy,
            audio_transcription_commands::get_audio_transcription_policy,
            audio_transcription_commands::get_audio_transcription_status,
            audio_transcription_commands::select_audio_and_create_meeting,
            audio_transcription_commands::select_and_add_audio_to_meeting,
            audio_transcription_commands::cancel_audio_transcription,
            summary_commands::get_summary_policy,
            summary_commands::get_deepseek_provider_status,
            summary_commands::get_summary_external_disclosure,
            summary_commands::confirm_summary_external_disclosure,
            summary_commands::revoke_deepseek_consent,
            summary_commands::create_summary_run,
            summary_commands::create_summary_revision,
            summary_commands::execute_summary_run,
            summary_commands::get_summary_run,
            summary_commands::list_summary_runs,
            summary_commands::cancel_summary_run,
            summary_commands::list_summary_evidence,
            summary_commands::get_summary_evidence,
            summary_commands::update_summary_content,
            summary_commands::get_summary_markdown,
            summary_commands::save_summary_markdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MeetingLens");
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    #[test]
    fn summary_smoke_override_requires_matching_temp_root_and_identifier() {
        let smoke_id = uuid::Uuid::new_v4().to_string();
        let allowed =
            std::env::temp_dir().join(format!("{SUMMARY_SMOKE_APP_DATA_PREFIX}{smoke_id}"));
        std::fs::create_dir_all(&allowed).expect("allowed temp root should create");
        let allowed =
            validate_summary_smoke_app_data_dir(&allowed).expect("allowed root should validate");
        assert!(
            validate_summary_smoke_identifier(
                &allowed,
                &format!("{SUMMARY_SMOKE_IDENTIFIER_PREFIX}{smoke_id}")
            )
            .is_ok()
        );
        assert!(validate_summary_smoke_identifier(&allowed, "com.gray.meetinglens").is_err());
        assert!(
            validate_summary_smoke_identifier(
                &allowed,
                &format!("{SUMMARY_SMOKE_IDENTIFIER_PREFIX}{}", uuid::Uuid::new_v4())
            )
            .is_err()
        );

        let nested = allowed.join(format!(
            "{SUMMARY_SMOKE_APP_DATA_PREFIX}{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&nested).expect("nested root should create");
        assert!(validate_summary_smoke_app_data_dir(&nested).is_err());
        let ordinary = std::env::temp_dir().join(format!("meetinglens-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&ordinary).expect("ordinary root should create");
        assert!(validate_summary_smoke_app_data_dir(&ordinary).is_err());

        std::fs::remove_dir_all(&allowed).expect("allowed tree should clean up");
        std::fs::remove_dir_all(&ordinary).expect("ordinary root should clean up");
    }
}
