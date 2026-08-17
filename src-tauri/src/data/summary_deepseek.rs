use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::io::{self, Read};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::{Zeroize, Zeroizing};

use super::summary_prompt::build_prompt;
use super::{
    CancellationSignal, SummaryProvider, SummaryProviderFuture, SummaryProviderOutput,
    SummaryProviderRequest, cancellable,
};
use crate::data::summary_error::{SummaryError, SummaryResult};
use crate::data::summary_model::{
    DEEPSEEK_SUMMARY_MAX_INPUT_BYTES, SUMMARY_RESULT_MAX_BYTES, SummaryContent,
};
use crate::secret_store::{SecretKind, SecretStoreService, SecretValue};

pub const DEEPSEEK_SUMMARY_PROVIDER: &str = "deepseek";
pub const DEEPSEEK_SUMMARY_PROVIDER_MODE: &str = "external";
pub const DEEPSEEK_SUMMARY_MODEL: &str = "deepseek-v4-pro";
pub const DEEPSEEK_SUMMARY_PROMPT_VERSION: &str = "summary-deepseek-v1";
pub const DEEPSEEK_ENDPOINT_HOST: &str = "api.deepseek.com";
pub const DEEPSEEK_CONSENT_POLICY_VERSION: &str = "deepseek-transcript-external-v1";
pub const DEEPSEEK_CONSENT_DATA_CATEGORIES: [&str; 6] = [
    "transcript_text",
    "speaker_labels_and_names",
    "time_offsets",
    "evidence_ids",
    "summary_template",
    "output_language",
];

pub const DEEPSEEK_MAX_REQUEST_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const DEEPSEEK_MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
pub const DEEPSEEK_MAX_OUTPUT_TOKENS: u32 = 16_384;
const DEEPSEEK_TEMPERATURE: f32 = 0.2;
const DEEPSEEK_CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
const DEEPSEEK_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEEPSEEK_TOTAL_TIMEOUT: Duration = Duration::from_secs(180);
const CANCELLATION_PARSE_CHUNK_BYTES: usize = 4 * 1024;

pub trait DeepSeekApiKeySource: Send + Sync {
    fn read_deepseek_api_key(&self) -> SummaryResult<Option<SecretValue>>;
}

impl DeepSeekApiKeySource for SecretStoreService {
    fn read_deepseek_api_key(&self) -> SummaryResult<Option<SecretValue>> {
        self.get_for_backend(SecretKind::DeepSeekApiKey)
            .map_err(|_| SummaryError::ProviderFailed)
    }
}

pub type DeepSeekTransportFuture<'a> =
    Pin<Box<dyn Future<Output = SummaryResult<DeepSeekTransportResponse>> + Send + 'a>>;

pub trait DeepSeekTransport: Send + Sync {
    fn send<'a>(&'a self, request: DeepSeekTransportRequest<'a>) -> DeepSeekTransportFuture<'a>;
}

pub struct DeepSeekTransportRequest<'a> {
    api_key: &'a SecretValue,
    serialized_body: &'a [u8],
}

impl DeepSeekTransportRequest<'_> {
    #[cfg(any(test, debug_assertions))]
    pub fn serialized_body(&self) -> &[u8] {
        self.serialized_body
    }
}

impl fmt::Debug for DeepSeekTransportRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekTransportRequest")
            .field("api_key", &"[REDACTED]")
            .field("serialized_body", &"[REDACTED]")
            .field("serialized_body_bytes", &self.serialized_body.len())
            .finish()
    }
}

pub struct DeepSeekTransportResponse {
    status: u16,
    body: Zeroizing<Vec<u8>>,
}

impl DeepSeekTransportResponse {
    #[cfg(any(test, debug_assertions))]
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            body: Zeroizing::new(body),
        }
    }
}

impl fmt::Debug for DeepSeekTransportResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekTransportResponse")
            .field("status", &self.status)
            .field("body", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct DeepSeekSummaryProvider {
    key_source: Arc<dyn DeepSeekApiKeySource>,
    transport: Arc<dyn DeepSeekTransport>,
    cancellation_signals: Arc<Mutex<HashMap<String, CancellationSignal>>>,
}

impl DeepSeekSummaryProvider {
    pub fn production(key_source: Arc<dyn DeepSeekApiKeySource>) -> SummaryResult<Self> {
        Ok(Self {
            key_source,
            transport: Arc::new(ReqwestDeepSeekTransport::new()?),
            cancellation_signals: Arc::default(),
        })
    }

    #[cfg(any(test, debug_assertions))]
    pub fn with_transport(
        key_source: Arc<dyn DeepSeekApiKeySource>,
        transport: Arc<dyn DeepSeekTransport>,
    ) -> Self {
        Self {
            key_source,
            transport,
            cancellation_signals: Arc::default(),
        }
    }

    fn cancellation_signal(&self, run_id: &str) -> CancellationSignal {
        let mut signals = self
            .cancellation_signals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        signals.entry(run_id.to_string()).or_default().clone()
    }

    async fn generate_inner(
        &self,
        request: &SummaryProviderRequest,
        signal: &CancellationSignal,
    ) -> SummaryResult<SummaryProviderOutput> {
        if signal.is_cancelled() {
            return Err(SummaryError::InvalidState);
        }
        let prompt = build_prompt(request)?;
        let chat_request = DeepSeekChatRequest {
            model: DEEPSEEK_SUMMARY_MODEL,
            thinking: ThinkingControl { kind: "disabled" },
            messages: [
                ChatMessage {
                    role: "system",
                    content: prompt.system(),
                },
                ChatMessage {
                    role: "user",
                    content: prompt.user(),
                },
            ],
            stream: false,
            response_format: ResponseFormat {
                kind: "json_object",
            },
            temperature: DEEPSEEK_TEMPERATURE,
            max_tokens: DEEPSEEK_MAX_OUTPUT_TOKENS,
        };
        let serialized_body = Zeroizing::new(
            serde_json::to_vec(&chat_request).map_err(|_| SummaryError::ProviderFailed)?,
        );
        if serialized_body.len() > DEEPSEEK_MAX_REQUEST_BODY_BYTES {
            return Err(SummaryError::ProviderInputTooLarge);
        }
        if signal.is_cancelled() {
            return Err(SummaryError::InvalidState);
        }

        let api_key = self
            .key_source
            .read_deepseek_api_key()?
            .ok_or(SummaryError::DeepSeekApiKeyMissing)?;
        if signal.is_cancelled() {
            return Err(SummaryError::InvalidState);
        }
        let transport_request = DeepSeekTransportRequest {
            api_key: &api_key,
            serialized_body: serialized_body.as_slice(),
        };
        let response = cancellable(signal, self.transport.send(transport_request)).await;
        drop(api_key);
        let response = response?;
        if signal.is_cancelled() {
            return Err(SummaryError::InvalidState);
        }
        if !(200..=299).contains(&response.status) {
            return Err(map_http_status(response.status));
        }
        if response.body.len() > DEEPSEEK_MAX_RESPONSE_BODY_BYTES {
            return Err(SummaryError::DeepSeekResponseInvalid);
        }
        let content = parse_success_response(response.body.as_slice(), signal)?;
        if signal.is_cancelled() {
            return Err(SummaryError::InvalidState);
        }
        Ok(SummaryProviderOutput {
            run_id: request.run_id.clone(),
            content,
        })
    }
}

impl fmt::Debug for DeepSeekSummaryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeepSeekSummaryProvider")
            .field("key_source", &"[CONFIGURED]")
            .field("transport", &"[CONFIGURED]")
            .finish_non_exhaustive()
    }
}

impl SummaryProvider for DeepSeekSummaryProvider {
    fn provider_name(&self) -> &'static str {
        DEEPSEEK_SUMMARY_PROVIDER
    }

    fn provider_mode(&self) -> &'static str {
        DEEPSEEK_SUMMARY_PROVIDER_MODE
    }

    fn model_name(&self) -> &'static str {
        DEEPSEEK_SUMMARY_MODEL
    }

    fn prompt_version(&self) -> &'static str {
        DEEPSEEK_SUMMARY_PROMPT_VERSION
    }

    fn max_input_bytes(&self) -> usize {
        DEEPSEEK_SUMMARY_MAX_INPUT_BYTES
    }

    fn generate<'a>(&'a self, request: &'a SummaryProviderRequest) -> SummaryProviderFuture<'a> {
        let signal = self.cancellation_signal(&request.run_id);
        Box::pin(async move { self.generate_inner(request, &signal).await })
    }

    fn cancel(&self, run_id: &str) -> SummaryResult<()> {
        self.cancellation_signal(run_id).cancel();
        Ok(())
    }

    fn is_cancelled(&self, run_id: &str) -> bool {
        self.cancellation_signals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(run_id)
            .cloned()
            .is_some_and(|signal| signal.is_cancelled())
    }

    fn release_run(&self, run_id: &str) {
        self.cancellation_signals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(run_id);
    }
}

#[derive(Clone)]
struct ReqwestDeepSeekTransport {
    client: reqwest::Client,
    endpoint: reqwest::Url,
}

impl ReqwestDeepSeekTransport {
    fn new() -> SummaryResult<Self> {
        let endpoint = reqwest::Url::parse(&format!(
            "https://{DEEPSEEK_ENDPOINT_HOST}{DEEPSEEK_CHAT_COMPLETIONS_PATH}"
        ))
        .map_err(|_| SummaryError::ProviderFailed)?;
        let client = reqwest::Client::builder()
            .connect_timeout(DEEPSEEK_CONNECT_TIMEOUT)
            .timeout(DEEPSEEK_TOTAL_TIMEOUT)
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .https_only(true)
            .build()
            .map_err(|_| SummaryError::ProviderFailed)?;
        Ok(Self { client, endpoint })
    }
}

impl DeepSeekTransport for ReqwestDeepSeekTransport {
    fn send<'a>(&'a self, request: DeepSeekTransportRequest<'a>) -> DeepSeekTransportFuture<'a> {
        Box::pin(async move {
            let api_key = request
                .api_key
                .expose_utf8()
                .map_err(|_| SummaryError::ProviderFailed)?;
            let mut bearer = Zeroizing::new(String::with_capacity(7 + api_key.len()));
            bearer.push_str("Bearer ");
            bearer.push_str(api_key);
            let mut authorization =
                HeaderValue::from_str(bearer.as_str()).map_err(|_| SummaryError::ProviderFailed)?;
            authorization.set_sensitive(true);

            let mut response = self
                .client
                .post(self.endpoint.clone())
                .header(AUTHORIZATION, authorization)
                .header(CONTENT_TYPE, "application/json")
                .body(request.serialized_body.to_vec())
                .send()
                .await
                .map_err(map_reqwest_error)?;
            let status = response.status().as_u16();
            if !response.status().is_success() {
                return Ok(DeepSeekTransportResponse {
                    status,
                    body: Zeroizing::new(Vec::new()),
                });
            }
            if response
                .content_length()
                .is_some_and(|length| length > DEEPSEEK_MAX_RESPONSE_BODY_BYTES as u64)
            {
                return Err(SummaryError::DeepSeekResponseInvalid);
            }

            let mut body = Zeroizing::new(Vec::new());
            while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
                let next_length = body
                    .len()
                    .checked_add(chunk.len())
                    .ok_or(SummaryError::DeepSeekResponseInvalid)?;
                if next_length > DEEPSEEK_MAX_RESPONSE_BODY_BYTES {
                    return Err(SummaryError::DeepSeekResponseInvalid);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(DeepSeekTransportResponse { status, body })
        })
    }
}

impl fmt::Debug for ReqwestDeepSeekTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReqwestDeepSeekTransport")
            .field("endpoint", &DEEPSEEK_ENDPOINT_HOST)
            .field("client", &"[CONFIGURED]")
            .finish()
    }
}

fn map_reqwest_error(error: reqwest::Error) -> SummaryError {
    if error.is_timeout() {
        SummaryError::DeepSeekTimeout
    } else {
        SummaryError::DeepSeekNetworkFailed
    }
}

fn map_http_status(status: u16) -> SummaryError {
    match status {
        400 | 422 => SummaryError::DeepSeekRequestInvalid,
        401 => SummaryError::DeepSeekAuthFailed,
        402 => SummaryError::DeepSeekInsufficientBalance,
        403 => SummaryError::DeepSeekAccessDenied,
        404 => SummaryError::DeepSeekModelUnavailable,
        408 => SummaryError::DeepSeekTimeout,
        429 => SummaryError::DeepSeekRateLimited,
        500..=599 => SummaryError::DeepSeekServiceUnavailable,
        _ => SummaryError::ProviderFailed,
    }
}

#[derive(Serialize)]
struct DeepSeekChatRequest<'a> {
    model: &'a str,
    thinking: ThinkingControl<'a>,
    messages: [ChatMessage<'a>; 2],
    stream: bool,
    response_format: ResponseFormat<'a>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ThinkingControl<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ResponseFormat<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeepSeekChatResponse {
    id: String,
    choices: Vec<DeepSeekChoice>,
    created: u64,
    model: String,
    object: String,
    #[serde(default)]
    system_fingerprint: Option<String>,
    #[serde(default)]
    usage: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeepSeekChoice {
    finish_reason: Option<String>,
    index: u32,
    message: DeepSeekMessage,
    #[serde(default)]
    logprobs: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeepSeekMessage {
    content: Option<String>,
    role: String,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<serde_json::Value>,
}

struct CancellationCheckingReader<'a> {
    body: &'a [u8],
    offset: usize,
    signal: &'a CancellationSignal,
    #[cfg(test)]
    cancel_after_bytes: Option<usize>,
}

impl<'a> CancellationCheckingReader<'a> {
    fn new(body: &'a [u8], signal: &'a CancellationSignal) -> Self {
        Self {
            body,
            offset: 0,
            signal,
            #[cfg(test)]
            cancel_after_bytes: None,
        }
    }

    #[cfg(test)]
    fn cancelling_after(
        body: &'a [u8],
        signal: &'a CancellationSignal,
        cancel_after_bytes: usize,
    ) -> Self {
        Self {
            body,
            offset: 0,
            signal,
            cancel_after_bytes: Some(cancel_after_bytes),
        }
    }

    fn cancelled_error() -> io::Error {
        // `serde_json::from_reader` retries `Interrupted` reads, so cancellation
        // must use a terminal I/O error kind to stop deserialization immediately.
        io::Error::other("summary parsing cancelled")
    }
}

impl Read for CancellationCheckingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.signal.is_cancelled() {
            return Err(Self::cancelled_error());
        }
        #[cfg(test)]
        if self
            .cancel_after_bytes
            .is_some_and(|limit| self.offset >= limit)
        {
            self.signal.cancel();
            return Err(Self::cancelled_error());
        }
        if self.offset == self.body.len() || buffer.is_empty() {
            return Ok(0);
        }
        let remaining = &self.body[self.offset..];
        let count = remaining
            .len()
            .min(buffer.len())
            .min(CANCELLATION_PARSE_CHUNK_BYTES);
        buffer[..count].copy_from_slice(&remaining[..count]);
        self.offset += count;
        Ok(count)
    }
}

fn deserialize_json_cancellable<T>(body: &[u8], signal: &CancellationSignal) -> SummaryResult<T>
where
    T: DeserializeOwned,
{
    serde_json::from_reader(CancellationCheckingReader::new(body, signal)).map_err(|_| {
        if signal.is_cancelled() {
            SummaryError::InvalidState
        } else {
            SummaryError::DeepSeekResponseInvalid
        }
    })
}

#[cfg(test)]
fn deserialize_json_cancelling_after<T>(
    body: &[u8],
    signal: &CancellationSignal,
    cancel_after_bytes: usize,
) -> SummaryResult<T>
where
    T: DeserializeOwned,
{
    serde_json::from_reader(CancellationCheckingReader::cancelling_after(
        body,
        signal,
        cancel_after_bytes,
    ))
    .map_err(|_| {
        if signal.is_cancelled() {
            SummaryError::InvalidState
        } else {
            SummaryError::DeepSeekResponseInvalid
        }
    })
}

fn parse_success_response(
    body: &[u8],
    signal: &CancellationSignal,
) -> SummaryResult<SummaryContent> {
    if signal.is_cancelled() {
        return Err(SummaryError::InvalidState);
    }
    if body.is_empty() {
        return Err(SummaryError::DeepSeekResponseEmpty);
    }
    let response: DeepSeekChatResponse = deserialize_json_cancellable(body, signal)?;
    validate_success_response(response, signal)
}

fn validate_success_response(
    response: DeepSeekChatResponse,
    signal: &CancellationSignal,
) -> SummaryResult<SummaryContent> {
    if signal.is_cancelled() {
        return Err(SummaryError::InvalidState);
    }
    if response.id.trim().is_empty()
        || response.created == 0
        || response.model != DEEPSEEK_SUMMARY_MODEL
        || response.object != "chat.completion"
        || response.choices.len() != 1
    {
        return Err(SummaryError::DeepSeekResponseInvalid);
    }
    let _ = (&response.system_fingerprint, &response.usage);
    let mut choice = response
        .choices
        .into_iter()
        .next()
        .ok_or(SummaryError::DeepSeekResponseInvalid)?;
    if choice.index != 0
        || choice.message.role != "assistant"
        || choice.message.reasoning_content.is_some()
        || choice.message.tool_calls.is_some()
    {
        return Err(SummaryError::DeepSeekResponseInvalid);
    }
    let _ = &choice.logprobs;
    match choice.finish_reason.as_deref() {
        Some("stop") => {}
        Some("length") => return Err(SummaryError::DeepSeekOutputTruncated),
        Some("content_filter") => return Err(SummaryError::DeepSeekContentFiltered),
        Some("tool_calls") => return Err(SummaryError::DeepSeekResponseInvalid),
        Some("insufficient_system_resource") => {
            return Err(SummaryError::DeepSeekServiceUnavailable);
        }
        _ => return Err(SummaryError::DeepSeekResponseInvalid),
    }
    let mut content = choice
        .message
        .content
        .take()
        .ok_or(SummaryError::DeepSeekResponseEmpty)?;
    if content.trim().is_empty() {
        content.zeroize();
        return Err(SummaryError::DeepSeekResponseEmpty);
    }
    if content.len() > SUMMARY_RESULT_MAX_BYTES {
        content.zeroize();
        return Err(SummaryError::DeepSeekResponseInvalid);
    }
    let parsed = deserialize_json_cancellable::<SummaryContent>(content.as_bytes(), signal);
    content.zeroize();
    let parsed = parsed?;
    if parsed.schema_version != 1 {
        return Err(SummaryError::DeepSeekResponseInvalid);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::task::Poll;

    use super::*;
    use crate::data::summary_model::{
        SummaryOutputLanguage, SummarySnapshotSegment, SummaryTemplate,
    };

    struct StaticKeySource {
        configured: bool,
        fail: bool,
    }

    impl DeepSeekApiKeySource for StaticKeySource {
        fn read_deepseek_api_key(&self) -> SummaryResult<Option<SecretValue>> {
            if self.fail {
                return Err(SummaryError::ProviderFailed);
            }
            if self.configured {
                SecretValue::from_user_input("synthetic-deepseek-test-key".to_string())
                    .map(Some)
                    .map_err(|_| SummaryError::ProviderFailed)
            } else {
                Ok(None)
            }
        }
    }

    struct StubTransport {
        status: u16,
        body: Vec<u8>,
        calls: AtomicUsize,
        captured_body: Mutex<Vec<u8>>,
    }

    impl StubTransport {
        fn new(status: u16, body: Vec<u8>) -> Self {
            Self {
                status,
                body,
                calls: AtomicUsize::new(0),
                captured_body: Mutex::new(Vec::new()),
            }
        }
    }

    impl DeepSeekTransport for StubTransport {
        fn send<'a>(
            &'a self,
            request: DeepSeekTransportRequest<'a>,
        ) -> DeepSeekTransportFuture<'a> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut body) = self.captured_body.lock() {
                *body = request.serialized_body().to_vec();
            }
            let status = self.status;
            let body = self.body.clone();
            Box::pin(async move { Ok(DeepSeekTransportResponse::new(status, body)) })
        }
    }

    fn segment(text: String) -> SummarySnapshotSegment {
        SummarySnapshotSegment {
            evidence_id: "00000000-0000-4000-8000-000000000011".to_string(),
            summary_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            source_id: "00000000-0000-4000-8000-000000000002".to_string(),
            segment_id: "00000000-0000-4000-8000-000000000003".to_string(),
            segment_revision: 1,
            source_ordinal: 0,
            segment_ordinal: 0,
            global_ordinal: 0,
            source_type: "pasted_text".to_string(),
            source_display_name: "Private source name".to_string(),
            source_language: Some("en-AU".to_string()),
            source_base_started_at: None,
            speaker_label: Some("Speaker 1".to_string()),
            speaker_name: Some("Synthetic Person".to_string()),
            start_offset_ms: Some(0),
            end_offset_ms: Some(1_000),
            original_text: text,
            language: Some("en-AU".to_string()),
            confidence: None,
            is_final: true,
            snapshot_at: "2026-08-07T00:00:00.000Z".to_string(),
        }
    }

    fn request(text: String) -> SummaryProviderRequest {
        SummaryProviderRequest {
            run_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![segment(text)],
        }
    }

    fn valid_content() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "overview": {
                "text": "Synthetic overview",
                "evidence_ids": ["00000000-0000-4000-8000-000000000011"]
            },
            "decisions": [],
            "action_items": [],
            "risks": [],
            "open_questions": []
        })
    }

    fn envelope(content: Option<String>, finish_reason: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "synthetic-response-id",
            "choices": [{
                "finish_reason": finish_reason,
                "index": 0,
                "message": {
                    "content": content,
                    "role": "assistant"
                }
            }],
            "created": 1,
            "model": DEEPSEEK_SUMMARY_MODEL,
            "object": "chat.completion",
            "usage": {
                "completion_tokens": 1,
                "prompt_tokens": 1,
                "total_tokens": 2
            }
        }))
        .expect("envelope should serialize")
    }

    fn provider_with(
        status: u16,
        body: Vec<u8>,
        configured: bool,
    ) -> (DeepSeekSummaryProvider, Arc<StubTransport>) {
        let transport = Arc::new(StubTransport::new(status, body));
        let provider = DeepSeekSummaryProvider::with_transport(
            Arc::new(StaticKeySource {
                configured,
                fail: false,
            }),
            transport.clone(),
        );
        (provider, transport)
    }

    #[test]
    fn production_metadata_and_consent_contract_are_frozen() {
        assert_eq!(DEEPSEEK_SUMMARY_PROVIDER, "deepseek");
        assert_eq!(DEEPSEEK_SUMMARY_PROVIDER_MODE, "external");
        assert_eq!(DEEPSEEK_SUMMARY_MODEL, "deepseek-v4-pro");
        assert_eq!(DEEPSEEK_SUMMARY_PROMPT_VERSION, "summary-deepseek-v1");
        assert_eq!(DEEPSEEK_ENDPOINT_HOST, "api.deepseek.com");
        let transport = ReqwestDeepSeekTransport::new().expect("fixed endpoint should configure");
        assert_eq!(transport.endpoint.scheme(), "https");
        assert_eq!(transport.endpoint.host_str(), Some(DEEPSEEK_ENDPOINT_HOST));
        assert_eq!(transport.endpoint.path(), DEEPSEEK_CHAT_COMPLETIONS_PATH);
        assert_eq!(
            include_str!("summary_deepseek.rs")
                .matches(".retry(reqwest::retry::never())")
                .count(),
            2,
            "the production builder and this assertion must be the only no-retry markers"
        );
        assert_eq!(
            DEEPSEEK_CONSENT_POLICY_VERSION,
            "deepseek-transcript-external-v1"
        );
        assert_eq!(
            DEEPSEEK_CONSENT_DATA_CATEGORIES,
            [
                "transcript_text",
                "speaker_labels_and_names",
                "time_offsets",
                "evidence_ids",
                "summary_template",
                "output_language"
            ]
        );
    }

    #[test]
    fn request_is_fixed_json_only_non_thinking_and_contains_no_internal_ids_or_key() {
        let body = envelope(Some(valid_content().to_string()), "stop");
        let (provider, transport) = provider_with(200, body, true);
        let result = tauri::async_runtime::block_on(
            provider.generate(&request("Synthetic transcript body".to_string())),
        )
        .expect("provider should parse");
        assert_eq!(result.content.schema_version, 1);
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);

        let body = transport
            .captured_body
            .lock()
            .expect("captured body should lock")
            .clone();
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("request body should parse");
        assert_eq!(value["model"], DEEPSEEK_SUMMARY_MODEL);
        assert_eq!(value["thinking"]["type"], "disabled");
        assert_eq!(value["stream"], false);
        assert_eq!(value["response_format"]["type"], "json_object");
        assert_eq!(value["temperature"], DEEPSEEK_TEMPERATURE);
        assert_eq!(value["max_tokens"], DEEPSEEK_MAX_OUTPUT_TOKENS);
        assert_eq!(value["messages"].as_array().map(Vec::len), Some(2));
        let serialized = String::from_utf8(body).expect("body should be utf-8");
        assert!(serialized.contains("JSON"));
        assert!(serialized.contains("Synthetic transcript body"));
        for forbidden in [
            "synthetic-deepseek-test-key",
            "00000000-0000-4000-8000-000000000010",
            "00000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
            "Private source name",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn missing_or_failed_key_never_calls_transport_and_debug_is_redacted() {
        let response = envelope(Some(valid_content().to_string()), "stop");
        let (provider, transport) = provider_with(200, response.clone(), false);
        assert_eq!(
            tauri::async_runtime::block_on(
                provider.generate(&request("Synthetic input".to_string()))
            ),
            Err(SummaryError::DeepSeekApiKeyMissing)
        );
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

        let transport = Arc::new(StubTransport::new(200, response));
        let provider = DeepSeekSummaryProvider::with_transport(
            Arc::new(StaticKeySource {
                configured: false,
                fail: true,
            }),
            transport.clone(),
        );
        assert_eq!(
            tauri::async_runtime::block_on(
                provider.generate(&request("Synthetic input".to_string()))
            ),
            Err(SummaryError::ProviderFailed)
        );
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        let debug = format!("{provider:?}");
        assert!(!debug.contains("synthetic-deepseek-test-key"));
        assert!(debug.contains("[CONFIGURED]"));
    }

    #[test]
    fn oversized_transcript_and_serialized_request_fail_before_transport() {
        let response = envelope(Some(valid_content().to_string()), "stop");
        let (provider, transport) = provider_with(200, response, true);
        assert_eq!(
            tauri::async_runtime::block_on(provider.generate(&request(
                "x".repeat(crate::data::summary_model::DEEPSEEK_SUMMARY_MAX_INPUT_BYTES + 1)
            ))),
            Err(SummaryError::ProviderInputTooLarge)
        );
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);

        let response = envelope(Some(valid_content().to_string()), "stop");
        let (provider, transport) = provider_with(200, response, true);
        assert_eq!(
            tauri::async_runtime::block_on(provider.generate(&request(
                "\\".repeat(crate::data::summary_model::DEEPSEEK_SUMMARY_MAX_INPUT_BYTES)
            ))),
            Err(SummaryError::ProviderInputTooLarge)
        );
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn http_statuses_map_without_parsing_error_bodies() {
        for (status, expected) in [
            (400, SummaryError::DeepSeekRequestInvalid),
            (401, SummaryError::DeepSeekAuthFailed),
            (402, SummaryError::DeepSeekInsufficientBalance),
            (403, SummaryError::DeepSeekAccessDenied),
            (404, SummaryError::DeepSeekModelUnavailable),
            (408, SummaryError::DeepSeekTimeout),
            (422, SummaryError::DeepSeekRequestInvalid),
            (429, SummaryError::DeepSeekRateLimited),
            (500, SummaryError::DeepSeekServiceUnavailable),
            (502, SummaryError::DeepSeekServiceUnavailable),
            (503, SummaryError::DeepSeekServiceUnavailable),
            (418, SummaryError::ProviderFailed),
        ] {
            let (provider, _) =
                provider_with(status, b"PRIVATE RAW PROVIDER ERROR BODY".to_vec(), true);
            assert_eq!(
                tauri::async_runtime::block_on(
                    provider.generate(&request("Synthetic input".to_string()))
                ),
                Err(expected),
                "status {status}"
            );
        }
    }

    #[test]
    fn response_envelope_and_finish_reason_are_strict() {
        let cases = [
            (Vec::new(), SummaryError::DeepSeekResponseEmpty),
            (b"not json".to_vec(), SummaryError::DeepSeekResponseInvalid),
            (envelope(None, "stop"), SummaryError::DeepSeekResponseEmpty),
            (
                envelope(Some(String::new()), "stop"),
                SummaryError::DeepSeekResponseEmpty,
            ),
            (
                envelope(Some(valid_content().to_string()), "length"),
                SummaryError::DeepSeekOutputTruncated,
            ),
            (
                envelope(Some(valid_content().to_string()), "content_filter"),
                SummaryError::DeepSeekContentFiltered,
            ),
            (
                envelope(Some(valid_content().to_string()), "tool_calls"),
                SummaryError::DeepSeekResponseInvalid,
            ),
            (
                envelope(
                    Some(valid_content().to_string()),
                    "insufficient_system_resource",
                ),
                SummaryError::DeepSeekServiceUnavailable,
            ),
            (
                envelope(Some(valid_content().to_string()), "future_value"),
                SummaryError::DeepSeekResponseInvalid,
            ),
        ];
        for (body, expected) in cases {
            let (provider, _) = provider_with(200, body, true);
            assert_eq!(
                tauri::async_runtime::block_on(
                    provider.generate(&request("Synthetic input".to_string()))
                ),
                Err(expected)
            );
        }
    }

    #[test]
    fn response_rejects_unknown_fields_wrong_schema_multiple_choices_and_oversize() {
        let mut unknown_content = valid_content();
        unknown_content["unexpected"] = serde_json::json!(true);
        let mut wrong_schema = valid_content();
        wrong_schema["schema_version"] = serde_json::json!(2);
        for content in [unknown_content, wrong_schema] {
            let (provider, _) =
                provider_with(200, envelope(Some(content.to_string()), "stop"), true);
            assert_eq!(
                tauri::async_runtime::block_on(
                    provider.generate(&request("Synthetic input".to_string()))
                ),
                Err(SummaryError::DeepSeekResponseInvalid)
            );
        }

        let mut multiple: serde_json::Value =
            serde_json::from_slice(&envelope(Some(valid_content().to_string()), "stop"))
                .expect("envelope should parse");
        let choice = multiple["choices"][0].clone();
        multiple["choices"] = serde_json::json!([choice.clone(), choice]);
        let (provider, _) = provider_with(
            200,
            serde_json::to_vec(&multiple).expect("multiple should serialize"),
            true,
        );
        assert_eq!(
            tauri::async_runtime::block_on(
                provider.generate(&request("Synthetic input".to_string()))
            ),
            Err(SummaryError::DeepSeekResponseInvalid)
        );

        let (provider, _) =
            provider_with(200, vec![b'x'; DEEPSEEK_MAX_RESPONSE_BODY_BYTES + 1], true);
        assert_eq!(
            tauri::async_runtime::block_on(
                provider.generate(&request("Synthetic input".to_string()))
            ),
            Err(SummaryError::DeepSeekResponseInvalid)
        );
    }

    #[test]
    fn response_parser_checks_cancellation_during_deserialization() {
        let body = envelope(Some(valid_content().to_string()), "stop");
        let signal = CancellationSignal::default();
        let parsed = deserialize_json_cancelling_after::<DeepSeekChatResponse>(
            &body,
            &signal,
            body.len() / 2,
        )
        .and_then(|response| validate_success_response(response, &signal));
        assert_eq!(parsed.map(|_| ()), Err(SummaryError::InvalidState));
        assert!(signal.is_cancelled());
    }

    struct PendingTransport {
        entered: Mutex<Option<mpsc::Sender<()>>>,
    }

    impl DeepSeekTransport for PendingTransport {
        fn send<'a>(
            &'a self,
            _request: DeepSeekTransportRequest<'a>,
        ) -> DeepSeekTransportFuture<'a> {
            Box::pin(std::future::poll_fn(move |_| {
                if let Ok(mut entered) = self.entered.lock()
                    && let Some(entered) = entered.take()
                {
                    let _ = entered.send(());
                }
                Poll::Pending
            }))
        }
    }

    #[test]
    fn cancellation_wakes_and_drops_an_in_flight_transport_future() {
        let (entered_sender, entered_receiver) = mpsc::channel();
        let provider = DeepSeekSummaryProvider::with_transport(
            Arc::new(StaticKeySource {
                configured: true,
                fail: false,
            }),
            Arc::new(PendingTransport {
                entered: Mutex::new(Some(entered_sender)),
            }),
        );
        let thread_provider = provider.clone();
        let request = request("Synthetic input".to_string());
        let run_id = request.run_id.clone();
        let handle = std::thread::spawn(move || {
            tauri::async_runtime::block_on(thread_provider.generate(&request))
        });
        let transport_entered = entered_receiver
            .recv_timeout(Duration::from_secs(5))
            .is_ok();
        provider.cancel(&run_id).expect("cancel should signal");
        assert!(provider.is_cancelled(&run_id));
        let result = handle.join().expect("provider thread should join");
        provider.release_run(&run_id);
        assert!(transport_entered, "transport future should be polled");
        assert_eq!(result, Err(SummaryError::InvalidState));
        assert!(!provider.is_cancelled(&run_id));
    }

    #[test]
    fn transport_request_and_response_debug_are_redacted() {
        let key = SecretValue::from_user_input("PRIVATE TEST KEY".to_string())
            .expect("test key should construct");
        let request = DeepSeekTransportRequest {
            api_key: &key,
            serialized_body: b"PRIVATE REQUEST BODY",
        };
        let response = DeepSeekTransportResponse::new(200, b"PRIVATE RESPONSE BODY".to_vec());
        let debug = format!("{request:?} {response:?}");
        for private in [
            "PRIVATE TEST KEY",
            "PRIVATE REQUEST BODY",
            "PRIVATE RESPONSE BODY",
        ] {
            assert!(!debug.contains(private));
        }
        assert!(debug.contains("[REDACTED]"));
    }
}
