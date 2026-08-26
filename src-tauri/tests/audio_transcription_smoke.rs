#[test]
#[ignore = "requires an explicitly provisioned local model and a synthetic Windows WAV"]
fn native_local_audio_transcription_smoke() {
    let smoke_root = std::env::var_os("MEETINGLENS_AUDIO_TRANSCRIPTION_SMOKE_ROOT")
        .expect("isolated smoke root is required");
    let input = std::env::var_os("MEETINGLENS_AUDIO_TRANSCRIPTION_SMOKE_INPUT")
        .expect("synthetic smoke input is required");
    meetinglens_lib::run_local_audio_transcription_smoke(smoke_root, input)
        .expect("local audio transcription smoke should pass");
}
