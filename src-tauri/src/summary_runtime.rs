use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::data::{
    CreateSummaryRevision, CreateSummaryRun, DEEPSEEK_CONSENT_DATA_CATEGORIES,
    DEEPSEEK_CONSENT_ENDPOINT_HOST, DEEPSEEK_CONSENT_MODEL, DEEPSEEK_CONSENT_POLICY_VERSION,
    DEEPSEEK_CONSENT_PROVIDER, MeetingService, ProviderConsentPolicy, ProviderConsentService,
    RenderedSummaryMarkdown, SummaryError, SummaryEvidence, SummaryEvidencePage, SummaryPolicy,
    SummaryProvider, SummaryResult, SummaryRun, SummaryRunPage, UpdateSummaryContent,
};
use crate::secret_store::{SecretKind, SecretStoreService};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeepSeekProviderStatus {
    pub(crate) key_configured: bool,
    pub(crate) consent_remembered: bool,
    pub(crate) consented_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SummaryExternalDisclosure {
    pub(crate) source_count: u64,
    pub(crate) segment_count: u64,
    pub(crate) input_bytes: u64,
    pub(crate) document_count: u64,
    pub(crate) document_chunk_count: u64,
    pub(crate) document_input_bytes: u64,
    pub(crate) template: crate::data::SummaryTemplate,
    pub(crate) output_language: crate::data::SummaryOutputLanguage,
    pub(crate) remembered_consent_valid: bool,
    pub(crate) requires_confirmation: bool,
}

#[derive(Clone)]
pub(crate) struct SummaryRuntimeService {
    meetings: MeetingService,
    secrets: SecretStoreService,
    consents: ProviderConsentService,
    provider: Arc<dyn SummaryProvider>,
}

impl fmt::Debug for SummaryRuntimeService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryRuntimeService")
            .field("meetings", &"[CONFIGURED]")
            .field("secrets", &"[REDACTED]")
            .field("consents", &"[CONFIGURED]")
            .field("provider", &self.provider.provider_name())
            .finish()
    }
}

impl SummaryRuntimeService {
    pub(crate) fn with_provider(
        meetings: MeetingService,
        secrets: SecretStoreService,
        provider: Arc<dyn SummaryProvider>,
    ) -> Self {
        let consents = meetings.provider_consents();
        Self {
            meetings,
            secrets,
            consents,
            provider,
        }
    }

    pub(crate) fn policy(&self) -> SummaryPolicy {
        if self.provider.provider_name() == "deepseek" {
            self.meetings.summaries().production_policy()
        } else {
            self.meetings.summaries().policy()
        }
    }

    pub(crate) fn create_run(&self, input: CreateSummaryRun) -> SummaryResult<SummaryRun> {
        self.meetings
            .summaries()
            .create_run_with_provider(input, self.provider.as_ref())
    }

    pub(crate) fn create_revision(
        &self,
        input: CreateSummaryRevision,
    ) -> SummaryResult<SummaryRun> {
        self.meetings
            .summaries()
            .create_revision_with_provider(input, self.provider.as_ref())
    }

    pub(crate) async fn execute(
        &self,
        meeting_id: &str,
        summary_id: &str,
    ) -> SummaryResult<SummaryRun> {
        if self.provider.provider_name() == "deepseek" {
            self.meetings
                .summaries()
                .execute_with_provider_consent_async(
                    meeting_id,
                    summary_id,
                    Arc::clone(&self.provider),
                    &self.consents,
                    &ProviderConsentPolicy::deepseek_summary_input(),
                )
                .await
        } else {
            self.meetings
                .summaries()
                .execute_with_provider_async(meeting_id, summary_id, Arc::clone(&self.provider))
                .await
        }
    }

    pub(crate) fn get(&self, meeting_id: &str, summary_id: &str) -> SummaryResult<SummaryRun> {
        self.meetings.summaries().get(meeting_id, summary_id)
    }

    pub(crate) fn list(
        &self,
        meeting_id: &str,
        cursor: Option<&str>,
        limit: Option<u64>,
    ) -> SummaryResult<SummaryRunPage> {
        self.meetings.summaries().list(meeting_id, cursor, limit)
    }

    pub(crate) fn cancel(&self, meeting_id: &str, summary_id: &str) -> SummaryResult<SummaryRun> {
        self.meetings.summaries().cancel(meeting_id, summary_id)
    }

    pub(crate) fn list_evidence(
        &self,
        meeting_id: &str,
        summary_id: &str,
        cursor: Option<u64>,
        limit: Option<u64>,
    ) -> SummaryResult<SummaryEvidencePage> {
        self.meetings
            .summaries()
            .list_evidence(meeting_id, summary_id, cursor, limit)
    }

    pub(crate) fn get_evidence(
        &self,
        meeting_id: &str,
        summary_id: &str,
        evidence_id: &str,
    ) -> SummaryResult<SummaryEvidence> {
        self.meetings
            .summaries()
            .get_evidence(meeting_id, summary_id, evidence_id)
    }

    pub(crate) fn update_content(&self, input: UpdateSummaryContent) -> SummaryResult<SummaryRun> {
        self.meetings.summaries().update_content(input)
    }

    pub(crate) fn markdown(
        &self,
        meeting_id: &str,
        summary_id: &str,
        expected_content_version: &str,
    ) -> SummaryResult<RenderedSummaryMarkdown> {
        self.meetings
            .summaries()
            .markdown(meeting_id, summary_id, expected_content_version)
    }

    pub(crate) fn save_markdown_to_path(
        &self,
        meeting_id: &str,
        summary_id: &str,
        expected_content_version: &str,
        path: &Path,
    ) -> SummaryResult<RenderedSummaryMarkdown> {
        self.meetings.summaries().save_markdown_to_path(
            meeting_id,
            summary_id,
            expected_content_version,
            path,
        )
    }

    pub(crate) fn provider_status(&self) -> SummaryResult<DeepSeekProviderStatus> {
        let key_configured = self
            .secrets
            .contains(SecretKind::DeepSeekApiKey)
            .map_err(|_| SummaryError::Internal)?;
        let remembered = self
            .consents
            .remembered(&ProviderConsentPolicy::deepseek_summary_input())
            .map_err(map_consent_error)?;
        Ok(DeepSeekProviderStatus {
            key_configured,
            consent_remembered: remembered.is_some(),
            consented_at: remembered.map(|preference| preference.consented_at),
        })
    }

    pub(crate) fn disclosure(&self, summary_id: &str) -> SummaryResult<SummaryExternalDisclosure> {
        let run = self
            .meetings
            .summaries()
            .disclosure_run(summary_id, self.provider.as_ref())?;
        self.disclosure_for_run(run, None)
    }

    pub(crate) fn confirm_disclosure(
        &self,
        summary_id: &str,
        remember: bool,
    ) -> SummaryResult<SummaryExternalDisclosure> {
        let run = self.meetings.summaries().confirm_external_disclosure(
            summary_id,
            remember,
            self.provider.as_ref(),
            &self.consents,
            &ProviderConsentPolicy::deepseek_summary_input(),
        )?;
        self.disclosure_for_run(run, Some(remember))
    }

    pub(crate) fn revoke_consent(&self) -> SummaryResult<DeepSeekProviderStatus> {
        self.consents.revoke().map_err(map_consent_error)?;
        self.provider_status()
    }

    fn disclosure_for_run(
        &self,
        run: SummaryRun,
        confirmed_remember: Option<bool>,
    ) -> SummaryResult<SummaryExternalDisclosure> {
        let remembered = match confirmed_remember {
            Some(remembered) => remembered,
            None => self
                .consents
                .remembered(&ProviderConsentPolicy::deepseek_summary_input())
                .map_err(map_consent_error)?
                .is_some(),
        };
        let one_time = if confirmed_remember == Some(false) {
            true
        } else {
            self.consents
                .one_time_valid(&run.id, &ProviderConsentPolicy::deepseek_summary_input())
                .map_err(map_consent_error)?
        };
        Ok(SummaryExternalDisclosure {
            source_count: run.source_count,
            segment_count: run.input_segment_count,
            input_bytes: run
                .input_bytes
                .checked_add(run.document_input_bytes)
                .ok_or(SummaryError::InputTooLarge)?,
            document_count: run.document_count,
            document_chunk_count: run.input_document_chunk_count,
            document_input_bytes: run.document_input_bytes,
            template: run.template,
            output_language: run.output_language,
            remembered_consent_valid: remembered,
            requires_confirmation: !remembered && !one_time,
        })
    }
}

pub(crate) const fn deepseek_provider_name() -> &'static str {
    DEEPSEEK_CONSENT_PROVIDER
}

pub(crate) const fn deepseek_endpoint_host() -> &'static str {
    DEEPSEEK_CONSENT_ENDPOINT_HOST
}

pub(crate) const fn deepseek_model() -> &'static str {
    DEEPSEEK_CONSENT_MODEL
}

pub(crate) const fn deepseek_policy_version() -> &'static str {
    DEEPSEEK_CONSENT_POLICY_VERSION
}

pub(crate) fn deepseek_data_categories() -> Vec<&'static str> {
    DEEPSEEK_CONSENT_DATA_CATEGORIES.to_vec()
}

fn map_consent_error(error: crate::data::DataError) -> SummaryError {
    match error {
        crate::data::DataError::InvalidId | crate::data::DataError::InvalidInput => {
            SummaryError::DeepSeekConsentInvalid
        }
        crate::data::DataError::Database
        | crate::data::DataError::MigrationConflict
        | crate::data::DataError::StorageUnavailable => SummaryError::PersistenceFailed,
        crate::data::DataError::InvalidState
        | crate::data::DataError::MeetingDeleteFailed
        | crate::data::DataError::MeetingDeletePending
        | crate::data::DataError::NotFound
        | crate::data::DataError::UnsafePath => SummaryError::Internal,
    }
}

#[cfg(debug_assertions)]
pub(crate) mod offline_smoke {
    use std::future::poll_fn;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::task::Poll;
    use std::time::Duration;

    use rusqlite::Connection;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::*;
    use crate::data::{
        CreateMeeting, DeepSeekSummaryProvider, DeepSeekTransport, DeepSeekTransportFuture,
        DeepSeekTransportRequest, DeepSeekTransportResponse, EditableSummaryContent,
        SummaryOutputLanguage, SummaryStatus, SummaryTemplate, TranscriptImportInput,
        TranscriptImportOutcome, UpdateSummaryContent, UpdateTranscriptSegment,
    };
    use crate::secret_store::test_support::InMemorySecretStore;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "meetinglens-deepseek-offline-smoke-{}",
                Uuid::new_v4()
            )))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Clone, Copy)]
    enum TransportMode {
        Valid,
        InvalidEvidence,
        Pending,
    }

    struct OfflineTransport {
        calls: AtomicUsize,
        captured_bodies: Mutex<Vec<Vec<u8>>>,
        mode: Mutex<TransportMode>,
        pending_entered: Mutex<Option<mpsc::Sender<()>>>,
    }

    impl OfflineTransport {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                captured_bodies: Mutex::new(Vec::new()),
                mode: Mutex::new(TransportMode::Valid),
                pending_entered: Mutex::new(None),
            }
        }

        fn set_mode(&self, mode: TransportMode) {
            *self.mode.lock().expect("transport mode should lock") = mode;
        }

        fn prepare_pending(&self) -> mpsc::Receiver<()> {
            let (sender, receiver) = mpsc::channel();
            *self
                .pending_entered
                .lock()
                .expect("pending sender should lock") = Some(sender);
            self.set_mode(TransportMode::Pending);
            receiver
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn captured_bodies(&self) -> Vec<Vec<u8>> {
            self.captured_bodies
                .lock()
                .expect("captured request bodies should lock")
                .clone()
        }

        fn response_for(request_body: &[u8], invalid_evidence: bool) -> Vec<u8> {
            let request: Value =
                serde_json::from_slice(request_body).expect("request body should be valid JSON");
            let user_prompt = request["messages"][1]["content"]
                .as_str()
                .expect("user prompt should be a string");
            let input: Value = serde_json::from_str(
                user_prompt
                    .split_once("INPUT JSON:\n")
                    .expect("prompt should contain a JSON input marker")
                    .1,
            )
            .expect("prompt input should be valid JSON");
            let evidence_id = if invalid_evidence {
                Uuid::new_v4().to_string()
            } else {
                input["transcriptSegments"][0]["evidenceId"]
                    .as_str()
                    .expect("snapshot evidence ID should exist")
                    .to_string()
            };
            let content = json!({
                "schema_version": 1,
                "overview": {
                    "text": "Synthetic offline minutes",
                    "evidence_ids": [evidence_id]
                },
                "decisions": [],
                "action_items": [],
                "risks": [],
                "open_questions": []
            });
            serde_json::to_vec(&json!({
                "id": "synthetic-offline-response",
                "choices": [{
                    "finish_reason": "stop",
                    "index": 0,
                    "message": {
                        "content": content.to_string(),
                        "role": "assistant"
                    }
                }],
                "created": 1,
                "model": "deepseek-v4-pro",
                "object": "chat.completion"
            }))
            .expect("synthetic response should serialize")
        }
    }

    impl DeepSeekTransport for OfflineTransport {
        fn send<'a>(
            &'a self,
            request: DeepSeekTransportRequest<'a>,
        ) -> DeepSeekTransportFuture<'a> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let request_body = request.serialized_body().to_vec();
            self.captured_bodies
                .lock()
                .expect("captured request bodies should lock")
                .push(request_body.clone());
            let mode = *self.mode.lock().expect("transport mode should lock");
            match mode {
                TransportMode::Valid | TransportMode::InvalidEvidence => {
                    let body = Self::response_for(
                        &request_body,
                        matches!(mode, TransportMode::InvalidEvidence),
                    );
                    Box::pin(async move { Ok(DeepSeekTransportResponse::new(200, body)) })
                }
                TransportMode::Pending => {
                    let mut sender = self
                        .pending_entered
                        .lock()
                        .expect("pending sender should lock")
                        .take();
                    Box::pin(poll_fn(move |_| {
                        if let Some(sender) = sender.take() {
                            let _ = sender.send(());
                        }
                        Poll::Pending
                    }))
                }
            }
        }
    }

    fn create_runtime(
        meetings: MeetingService,
        secrets: SecretStoreService,
        transport: Arc<OfflineTransport>,
    ) -> SummaryRuntimeService {
        let key_source: Arc<dyn crate::data::DeepSeekApiKeySource> = Arc::new(secrets.clone());
        let provider = Arc::new(DeepSeekSummaryProvider::with_transport(
            key_source, transport,
        ));
        SummaryRuntimeService::with_provider(meetings, secrets, provider)
    }

    fn create_run(
        runtime: &SummaryRuntimeService,
        meeting_id: &str,
        source_id: &str,
        template: SummaryTemplate,
    ) -> SummaryRun {
        runtime
            .create_run(CreateSummaryRun {
                meeting_id: meeting_id.to_string(),
                source_ids: vec![source_id.to_string()],
                document_ids: Vec::new(),
                template,
                output_language: SummaryOutputLanguage::En,
            })
            .expect("DeepSeek run should be created")
    }

    pub(crate) fn run() {
        let directory = TestDirectory::new();
        let meetings = MeetingService::initialize(&directory.0)
            .expect("isolated application data should initialize");
        let database_path = meetings.storage_paths().database().to_path_buf();
        let secret_backend = Arc::new(InMemorySecretStore::default());
        let secrets = SecretStoreService::with_backend(secret_backend);
        let transport = Arc::new(OfflineTransport::new());
        let runtime = create_runtime(meetings.clone(), secrets.clone(), transport.clone());

        let meeting = meetings
            .create(CreateMeeting {
                title: "Synthetic offline DeepSeek smoke".to_string(),
                primary_language: "en-AU".to_string(),
            })
            .expect("synthetic meeting should be created");
        let source = match meetings
            .transcripts()
            .import_pasted_text(
                &meeting.id,
                TranscriptImportInput {
                    display_name: "Synthetic transcript".to_string(),
                    original_filename: None,
                    content: b"Speaker 1: Synthetic frozen point".to_vec(),
                    language: Some("en-AU".to_string()),
                    base_started_at: Some("2026-08-17T00:00:00.000Z".to_string()),
                },
            )
            .expect("synthetic transcript should import")
        {
            TranscriptImportOutcome::Imported { source, .. } => *source,
            TranscriptImportOutcome::Duplicate { .. } => {
                panic!("unique synthetic transcript must not be a duplicate")
            }
        };

        let first = create_run(&runtime, &meeting.id, &source.id, SummaryTemplate::Detailed);
        assert_eq!(first.provider, "deepseek");
        assert_eq!(first.provider_mode, "external");
        assert_eq!(first.model, "deepseek-v4-pro");
        assert_eq!(first.prompt_version, "summary-deepseek-v2");
        assert_eq!(
            tauri::async_runtime::block_on(runtime.execute(&meeting.id, &first.id)),
            Err(SummaryError::DeepSeekConsentRequired)
        );
        assert_eq!(transport.calls(), 0, "Consent must fail before transport");
        assert_eq!(
            runtime
                .get(&meeting.id, &first.id)
                .expect("queued run should remain readable")
                .status,
            SummaryStatus::Queued
        );

        let disclosure = runtime
            .disclosure(&first.id)
            .expect("safe disclosure should load");
        assert!(disclosure.requires_confirmation);
        assert_eq!(disclosure.source_count, 1);
        assert!(disclosure.segment_count >= 1);
        assert!(disclosure.input_bytes > 0);
        let confirmed = runtime
            .confirm_disclosure(&first.id, false)
            .expect("one-time Consent should be granted");
        assert!(!confirmed.requires_confirmation);
        assert!(!confirmed.remembered_consent_valid);

        let segment = meetings
            .transcripts()
            .list_segments(&meeting.id, &source.id)
            .expect("segments should list")
            .into_iter()
            .next()
            .expect("one segment should exist");
        meetings
            .transcripts()
            .update_segment(
                &meeting.id,
                &segment.id,
                UpdateTranscriptSegment {
                    speaker_name: None,
                    original_text: Some("Synthetic live text changed later".to_string()),
                    expected_revision: segment.revision,
                },
            )
            .expect("live transcript should update");
        secrets
            .set(
                SecretKind::DeepSeekApiKey,
                "synthetic-offline-key-material".to_string(),
            )
            .expect("synthetic in-memory key should configure");
        let completed = tauri::async_runtime::block_on(runtime.execute(&meeting.id, &first.id))
            .expect("offline DeepSeek execution should complete");
        assert_eq!(completed.status, SummaryStatus::Completed);
        assert!(completed.content.is_some());
        let evidence_page = runtime
            .list_evidence(&meeting.id, &first.id, None, None)
            .expect("evidence should list");
        assert!(evidence_page.total_count > 0);
        let exact_evidence = runtime
            .get_evidence(&meeting.id, &first.id, &evidence_page.items[0].id)
            .expect("referenced evidence should load by its ownership tuple");
        assert_eq!(exact_evidence.summary_id, first.id);
        assert_eq!(
            runtime.get_evidence(&meeting.id, &first.id, &Uuid::new_v4().to_string()),
            Err(SummaryError::EvidenceInvalid)
        );
        let content_version = crate::data::summary_content_version(
            completed.content.as_ref().expect("content should exist"),
        )
        .expect("content version should derive");
        assert_eq!(
            runtime.update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: content_version.clone(),
                editable_content: EditableSummaryContent {
                    overview_text: "Synthetic invalid shape".to_string(),
                    decision_texts: vec!["Synthetic injected decision".to_string()],
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            }),
            Err(SummaryError::InvalidInput)
        );
        assert_eq!(
            runtime.update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: content_version.clone(),
                editable_content: EditableSummaryContent {
                    overview_text: "x".repeat(16 * 1024 + 1),
                    decision_texts: Vec::new(),
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            }),
            Err(SummaryError::EditTooLarge)
        );
        let edited = runtime
            .update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: content_version,
                editable_content: EditableSummaryContent {
                    overview_text: "Synthetic user-edited offline minutes".to_string(),
                    decision_texts: Vec::new(),
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            })
            .expect("DeepSeek result should accept a bounded user edit");
        assert!(edited.is_user_edited);
        let edited_version = crate::data::summary_content_version(
            edited
                .content
                .as_ref()
                .expect("edited content should exist"),
        )
        .expect("edited version should derive");
        let no_op = runtime
            .update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: edited_version.clone(),
                editable_content: EditableSummaryContent {
                    overview_text: "Synthetic user-edited offline minutes".to_string(),
                    decision_texts: Vec::new(),
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            })
            .expect("an identical save should be a no-op");
        assert_eq!(no_op.updated_at, edited.updated_at);
        assert_eq!(
            runtime.update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: "0".repeat(64),
                editable_content: EditableSummaryContent {
                    overview_text: "Synthetic stale edit".to_string(),
                    decision_texts: Vec::new(),
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            }),
            Err(SummaryError::EditConflict)
        );
        let markdown = runtime
            .markdown(&meeting.id, &first.id, &edited_version)
            .expect("canonical Markdown should render");
        assert!(
            markdown
                .markdown
                .contains(r"Synthetic user\-edited offline minutes")
        );
        assert!(markdown.markdown.contains("[E1]"));
        assert!(!markdown.markdown.contains(&first.id));
        let export_path = std::env::temp_dir().join(format!(
            "meetinglens-summary-offline-export-{}.md",
            Uuid::new_v4()
        ));
        let saved = runtime
            .save_markdown_to_path(&meeting.id, &first.id, &edited_version, &export_path)
            .expect("isolated Markdown export should save atomically");
        assert_eq!(
            std::fs::read(&export_path).expect("saved Markdown should read"),
            saved.markdown.as_bytes()
        );
        std::fs::remove_file(&export_path).expect("isolated export should clean up");
        let first_body = String::from_utf8(
            transport
                .captured_bodies()
                .into_iter()
                .next()
                .expect("one request should be captured"),
        )
        .expect("request should be UTF-8");
        assert!(first_body.contains("Synthetic frozen point"));
        assert!(!first_body.contains("Synthetic live text changed later"));
        assert!(!first_body.contains(&meeting.id));
        assert!(!first_body.contains(&source.id));
        assert!(!first_body.contains("synthetic-offline-key-material"));

        let second = runtime
            .create_revision(CreateSummaryRevision {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
            })
            .expect("revision should capture the updated transcript");
        assert_eq!(
            runtime.update_content(UpdateSummaryContent {
                meeting_id: meeting.id.clone(),
                summary_id: first.id.clone(),
                expected_content_version: edited_version.clone(),
                editable_content: EditableSummaryContent {
                    overview_text: "Synthetic user-edited offline minutes".to_string(),
                    decision_texts: Vec::new(),
                    action_items: Vec::new(),
                    risk_texts: Vec::new(),
                    open_question_texts: Vec::new(),
                },
            }),
            Err(SummaryError::RevisionConflict)
        );
        let remembered = runtime
            .confirm_disclosure(&second.id, true)
            .expect("remembered Consent should persist");
        assert!(remembered.remembered_consent_valid);
        assert!(!remembered.requires_confirmation);
        assert_eq!(
            tauri::async_runtime::block_on(runtime.execute(&meeting.id, &second.id))
                .expect("remembered run should execute")
                .status,
            SummaryStatus::Completed
        );

        drop(runtime);
        drop(meetings);
        let reopened = MeetingService::initialize(&directory.0)
            .expect("isolated application data should reopen");
        let runtime = create_runtime(reopened.clone(), secrets.clone(), transport.clone());
        let status = runtime
            .provider_status()
            .expect("provider status should load after restart");
        assert!(status.key_configured);
        assert!(status.consent_remembered);
        assert!(status.consented_at.is_some());
        let third = runtime
            .create_revision(CreateSummaryRevision {
                meeting_id: meeting.id.clone(),
                summary_id: second.id.clone(),
            })
            .expect("post-restart revision should create");
        assert_eq!(
            tauri::async_runtime::block_on(runtime.execute(&meeting.id, &third.id))
                .expect("remembered Consent should survive restart")
                .status,
            SummaryStatus::Completed
        );

        let revoked = runtime
            .revoke_consent()
            .expect("Consent revoke should succeed");
        assert!(!revoked.consent_remembered);
        assert!(revoked.consented_at.is_none());
        let invalid = runtime
            .create_revision(CreateSummaryRevision {
                meeting_id: meeting.id.clone(),
                summary_id: third.id.clone(),
            })
            .expect("post-revoke revision should create");
        let calls_before_rejected = transport.calls();
        assert_eq!(
            tauri::async_runtime::block_on(runtime.execute(&meeting.id, &invalid.id)),
            Err(SummaryError::DeepSeekConsentRequired)
        );
        assert_eq!(transport.calls(), calls_before_rejected);
        runtime
            .confirm_disclosure(&invalid.id, false)
            .expect("new one-time Consent should be required after revoke");
        transport.set_mode(TransportMode::InvalidEvidence);
        assert_eq!(
            tauri::async_runtime::block_on(runtime.execute(&meeting.id, &invalid.id)),
            Err(SummaryError::EvidenceInvalid)
        );
        let failed = runtime
            .get(&meeting.id, &invalid.id)
            .expect("failed run should remain readable");
        assert_eq!(failed.status, SummaryStatus::Failed);
        assert!(failed.content.is_none());
        assert_eq!(
            runtime
                .list_evidence(&meeting.id, &invalid.id, None, None)
                .expect("failed evidence should list")
                .total_count,
            0
        );

        transport.set_mode(TransportMode::Valid);
        let pending = create_run(&runtime, &meeting.id, &source.id, SummaryTemplate::Concise);
        runtime
            .confirm_disclosure(&pending.id, false)
            .expect("pending run should receive one-time Consent");
        let entered = transport.prepare_pending();
        let execution_runtime = runtime.clone();
        let execution_meeting_id = meeting.id.clone();
        let execution_summary_id = pending.id.clone();
        let execution = std::thread::spawn(move || {
            tauri::async_runtime::block_on(
                execution_runtime.execute(&execution_meeting_id, &execution_summary_id),
            )
        });
        entered
            .recv_timeout(Duration::from_secs(5))
            .expect("pending transport should be entered");
        assert_eq!(
            runtime
                .cancel(&meeting.id, &pending.id)
                .expect("running request should cancel")
                .status,
            SummaryStatus::Cancelled
        );
        assert_eq!(
            execution
                .join()
                .expect("execution thread should join")
                .expect("cancelled execution should return its terminal run")
                .status,
            SummaryStatus::Cancelled
        );
        assert!(
            runtime
                .get(&meeting.id, &pending.id)
                .expect("cancelled run should remain readable")
                .content
                .is_none()
        );
        assert_eq!(
            runtime
                .list_evidence(&meeting.id, &pending.id, None, None)
                .expect("cancelled evidence should list")
                .total_count,
            0
        );

        reopened
            .delete(&meeting.id)
            .expect("synthetic meeting should be deleted");
        assert!(reopened.list().expect("meetings should list").is_empty());
        let connection = Connection::open(&database_path).expect("database should open for checks");
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check should run");
        let foreign_key_violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("foreign key check should run");
        let schema_version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("schema version should be readable");
        assert_eq!(integrity, "ok");
        assert_eq!(foreign_key_violations, 0);
        assert_eq!(schema_version, 7);
        drop(connection);
        drop(runtime);
        drop(reopened);
        drop(secrets);
        drop(transport);
        std::fs::remove_dir_all(&directory.0).expect("isolated app-data should be removable");
        assert!(!directory.0.exists(), "isolated app-data must not remain");
    }

    #[cfg(test)]
    #[test]
    fn windows_offline_deepseek_flow_is_atomic_cancellable_and_isolated() {
        run();
    }
}
