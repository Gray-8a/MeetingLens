#[cfg(any(test, debug_assertions))]
use std::collections::HashMap;
use std::fmt;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    SUMMARY_MAX_INPUT_BYTES, SummaryContent, SummaryOutputLanguage, SummarySnapshotSegment,
    SummaryTemplate,
};
#[cfg(any(test, debug_assertions))]
use super::summary_model::{
    SummaryActionItem, SummaryActionStatus, SummaryClaim, SummaryDecision, SummaryDecisionStatus,
};

#[path = "summary_deepseek.rs"]
mod summary_deepseek;
#[path = "summary_prompt.rs"]
mod summary_prompt;

pub use summary_deepseek::{
    DEEPSEEK_CONSENT_DATA_CATEGORIES, DEEPSEEK_CONSENT_POLICY_VERSION, DEEPSEEK_ENDPOINT_HOST,
    DEEPSEEK_SUMMARY_MODEL, DEEPSEEK_SUMMARY_PROMPT_VERSION, DEEPSEEK_SUMMARY_PROVIDER,
    DEEPSEEK_SUMMARY_PROVIDER_MODE, DeepSeekApiKeySource, DeepSeekSummaryProvider,
};
#[cfg(any(test, debug_assertions))]
pub use summary_deepseek::{
    DeepSeekTransport, DeepSeekTransportFuture, DeepSeekTransportRequest, DeepSeekTransportResponse,
};

#[cfg(any(test, debug_assertions))]
pub const MOCK_SUMMARY_PROVIDER: &str = "deterministic_mock";
#[cfg(any(test, debug_assertions))]
pub const MOCK_SUMMARY_PROVIDER_MODE: &str = "mock";
#[cfg(any(test, debug_assertions))]
pub const MOCK_SUMMARY_MODEL: &str = "meetinglens-deterministic-mock-v1";
#[cfg(any(test, debug_assertions))]
pub const MOCK_SUMMARY_PROMPT_VERSION: &str = "summary-core-mock-v1";

pub type SummaryProviderFuture<'a> =
    Pin<Box<dyn Future<Output = SummaryResult<SummaryProviderOutput>> + Send + 'a>>;

#[derive(Clone, Default)]
pub(crate) struct CancellationSignal {
    inner: Arc<CancellationSignalInner>,
}

#[derive(Default)]
struct CancellationSignalInner {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl CancellationSignal {
    pub(crate) fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        let mut waker = self
            .inner
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(waker) = waker.take() {
            waker.wake();
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    #[cfg(any(test, debug_assertions))]
    pub(crate) async fn cancelled(&self) {
        poll_fn(|context| {
            if self.is_cancelled() {
                return Poll::Ready(());
            }
            let mut waker = self
                .inner
                .waker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *waker = Some(context.waker().clone());
            if self.is_cancelled() {
                waker.take();
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }
}

pub(crate) async fn cancellable<T, F>(signal: &CancellationSignal, future: F) -> SummaryResult<T>
where
    F: Future<Output = SummaryResult<T>> + Send,
{
    let mut future = Box::pin(future);
    poll_fn(|context| {
        if signal.is_cancelled() {
            return Poll::Ready(Err(SummaryError::InvalidState));
        }
        if let Poll::Ready(result) = future.as_mut().poll(context) {
            return Poll::Ready(result);
        }
        let mut waker = signal
            .inner
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *waker = Some(context.waker().clone());
        if signal.is_cancelled() {
            waker.take();
            Poll::Ready(Err(SummaryError::InvalidState))
        } else {
            Poll::Pending
        }
    })
    .await
}

#[derive(Clone, PartialEq)]
pub struct SummaryProviderRequest {
    pub run_id: String,
    pub meeting_id: String,
    pub template: SummaryTemplate,
    pub output_language: SummaryOutputLanguage,
    pub segments: Vec<SummarySnapshotSegment>,
}

impl fmt::Debug for SummaryProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryProviderRequest")
            .field("run_id", &self.run_id)
            .field("meeting_id", &self.meeting_id)
            .field("template", &self.template)
            .field("output_language", &self.output_language)
            .field("segment_count", &self.segments.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryProviderOutput {
    pub run_id: String,
    pub content: SummaryContent,
}

pub trait SummaryProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn provider_mode(&self) -> &'static str;
    fn model_name(&self) -> &'static str;
    fn prompt_version(&self) -> &'static str;
    fn max_input_bytes(&self) -> usize {
        SUMMARY_MAX_INPUT_BYTES
    }
    fn generate<'a>(&'a self, request: &'a SummaryProviderRequest) -> SummaryProviderFuture<'a>;

    fn cancel(&self, _run_id: &str) -> SummaryResult<()> {
        Ok(())
    }

    fn is_cancelled(&self, _run_id: &str) -> bool {
        false
    }

    fn release_run(&self, _run_id: &str) {}
}

#[cfg(any(test, debug_assertions))]
#[derive(Clone, Default)]
pub struct DeterministicMockSummaryProvider {
    cancellation_signals: Arc<Mutex<HashMap<String, CancellationSignal>>>,
    fail_generation: bool,
    #[cfg(debug_assertions)]
    block_on_execution: Option<usize>,
    #[cfg(debug_assertions)]
    execution_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(any(test, debug_assertions))]
impl fmt::Debug for DeterministicMockSummaryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("DeterministicMockSummaryProvider");
        debug.field("fail_generation", &self.fail_generation);
        #[cfg(debug_assertions)]
        debug.field("block_on_execution", &self.block_on_execution);
        debug.finish_non_exhaustive()
    }
}

#[cfg(any(test, debug_assertions))]
impl DeterministicMockSummaryProvider {
    #[cfg(test)]
    pub(crate) fn failing() -> Self {
        Self {
            cancellation_signals: Arc::default(),
            fail_generation: true,
            #[cfg(debug_assertions)]
            block_on_execution: None,
            #[cfg(debug_assertions)]
            execution_count: Arc::default(),
        }
    }

    #[cfg(debug_assertions)]
    pub fn blocking_on_nth_execution(execution_number: usize) -> Self {
        Self {
            cancellation_signals: Arc::default(),
            fail_generation: false,
            block_on_execution: (execution_number > 0).then_some(execution_number),
            execution_count: Arc::default(),
        }
    }

    fn cancellation_signal(&self, run_id: &str) -> CancellationSignal {
        let mut signals = self
            .cancellation_signals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        signals.entry(run_id.to_string()).or_default().clone()
    }

    async fn wait_for_interruption_smoke(
        &self,
        signal: &CancellationSignal,
        execution_number: usize,
    ) -> SummaryResult<()> {
        #[cfg(debug_assertions)]
        if self.block_on_execution == Some(execution_number) {
            signal.cancelled().await;
            return Err(SummaryError::InvalidState);
        }
        #[cfg(not(debug_assertions))]
        let _ = execution_number;
        Ok(())
    }
}

#[cfg(any(test, debug_assertions))]
impl SummaryProvider for DeterministicMockSummaryProvider {
    fn provider_name(&self) -> &'static str {
        MOCK_SUMMARY_PROVIDER
    }

    fn provider_mode(&self) -> &'static str {
        MOCK_SUMMARY_PROVIDER_MODE
    }

    fn model_name(&self) -> &'static str {
        MOCK_SUMMARY_MODEL
    }

    fn prompt_version(&self) -> &'static str {
        MOCK_SUMMARY_PROMPT_VERSION
    }

    fn generate<'a>(&'a self, request: &'a SummaryProviderRequest) -> SummaryProviderFuture<'a> {
        let signal = self.cancellation_signal(&request.run_id);
        #[cfg(debug_assertions)]
        let execution_number = self.execution_count.fetch_add(1, Ordering::AcqRel) + 1;
        #[cfg(not(debug_assertions))]
        let execution_number = 0;
        Box::pin(async move {
            if self.fail_generation {
                return Err(SummaryError::ProviderFailed);
            }
            if signal.is_cancelled() {
                return Err(SummaryError::InvalidState);
            }
            self.wait_for_interruption_smoke(&signal, execution_number)
                .await?;
            if signal.is_cancelled() {
                return Err(SummaryError::InvalidState);
            }
            let first = request
                .segments
                .first()
                .ok_or(SummaryError::SnapshotMissing)?;
            let last = request.segments.last().unwrap_or(first);
            let overview_evidence = if first.evidence_id == last.evidence_id {
                vec![first.evidence_id.clone()]
            } else {
                vec![first.evidence_id.clone(), last.evidence_id.clone()]
            };

            Ok(SummaryProviderOutput {
                run_id: request.run_id.clone(),
                content: SummaryContent {
                    schema_version: 1,
                    overview: SummaryClaim {
                        text: format!(
                            "Deterministic mock summary generated from {} frozen transcript segment(s).",
                            request.segments.len()
                        ),
                        evidence_ids: overview_evidence,
                    },
                    decisions: vec![SummaryDecision {
                        text: "Synthetic mock decision for Summary Run Core verification."
                            .to_string(),
                        status: SummaryDecisionStatus::Confirmed,
                        evidence_ids: vec![first.evidence_id.clone()],
                    }],
                    action_items: vec![SummaryActionItem {
                        text: "Review the deterministic mock result before enabling an external provider."
                            .to_string(),
                        owner: None,
                        due_date: None,
                        status: SummaryActionStatus::Unconfirmed,
                        evidence_ids: vec![last.evidence_id.clone()],
                    }],
                    risks: Vec::new(),
                    open_questions: Vec::new(),
                },
            })
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(evidence_id: &str, global_ordinal: u64) -> SummarySnapshotSegment {
        SummarySnapshotSegment {
            evidence_id: evidence_id.to_string(),
            summary_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            source_id: "00000000-0000-4000-8000-000000000002".to_string(),
            segment_id: "00000000-0000-4000-8000-000000000003".to_string(),
            segment_revision: 1,
            source_ordinal: 0,
            segment_ordinal: global_ordinal,
            global_ordinal,
            source_type: "pasted_text".to_string(),
            source_display_name: "Synthetic source".to_string(),
            source_language: Some("en-AU".to_string()),
            source_base_started_at: None,
            speaker_label: Some("Speaker 1".to_string()),
            speaker_name: None,
            start_offset_ms: None,
            end_offset_ms: None,
            original_text: "Synthetic transcript input".to_string(),
            language: Some("en-AU".to_string()),
            confidence: None,
            is_final: true,
            snapshot_at: "2026-08-07T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn deterministic_mock_uses_only_allowlisted_snapshot_ids() {
        let request = SummaryProviderRequest {
            run_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![
                segment("00000000-0000-4000-8000-000000000011", 0),
                segment("00000000-0000-4000-8000-000000000012", 1),
            ],
        };

        let provider = DeterministicMockSummaryProvider::default();
        let first = tauri::async_runtime::block_on(provider.generate(&request))
            .expect("mock should generate");
        let second = tauri::async_runtime::block_on(provider.generate(&request))
            .expect("mock should remain deterministic");
        assert_eq!(first, second);
        assert_eq!(first.run_id, request.run_id);
        assert_eq!(first.content.schema_version, 1);
        assert_eq!(
            first.content.decisions[0].evidence_ids[0],
            request.segments[0].evidence_id
        );
    }

    #[test]
    fn deterministic_mock_rejects_an_empty_snapshot() {
        let request = SummaryProviderRequest {
            run_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: Vec::new(),
        };

        assert_eq!(
            tauri::async_runtime::block_on(
                DeterministicMockSummaryProvider::default().generate(&request),
            ),
            Err(SummaryError::SnapshotMissing)
        );
    }

    #[test]
    fn deterministic_mock_supports_test_failure_and_cooperative_cancel() {
        let request = SummaryProviderRequest {
            run_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![segment("00000000-0000-4000-8000-000000000011", 0)],
        };

        assert_eq!(
            tauri::async_runtime::block_on(
                DeterministicMockSummaryProvider::failing().generate(&request),
            ),
            Err(SummaryError::ProviderFailed)
        );

        let provider = DeterministicMockSummaryProvider::default();
        provider
            .cancel(&request.run_id)
            .expect("mock cancellation should register");
        assert!(provider.is_cancelled(&request.run_id));
        assert_eq!(
            tauri::async_runtime::block_on(provider.generate(&request)),
            Err(SummaryError::InvalidState)
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn deterministic_mock_blocks_only_on_the_configured_execution_number() {
        let provider = DeterministicMockSummaryProvider::blocking_on_nth_execution(3);
        let mut request = SummaryProviderRequest {
            run_id: "synthetic-run-1".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![segment("00000000-0000-4000-8000-000000000011", 0)],
        };

        for execution in 1..=2 {
            request.run_id = format!("synthetic-run-{execution}");
            tauri::async_runtime::block_on(provider.generate(&request))
                .expect("the first two smoke executions should complete");
            provider.release_run(&request.run_id);
        }

        request.run_id = "synthetic-run-3".to_string();
        let mut future = provider.generate(&request);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        provider
            .cancel(&request.run_id)
            .expect("the blocked execution should remain cancellable");
        assert_eq!(
            tauri::async_runtime::block_on(future),
            Err(SummaryError::InvalidState)
        );
        provider.release_run(&request.run_id);
    }

    #[test]
    fn cancellation_signal_wakes_a_pending_future() {
        use std::sync::Barrier;

        let signal = CancellationSignal::default();
        let thread_signal = signal.clone();
        let entered = Arc::new(Barrier::new(2));
        let thread_entered = Arc::clone(&entered);
        let handle = std::thread::spawn(move || {
            tauri::async_runtime::block_on(cancellable(&thread_signal, async move {
                poll_fn::<SummaryResult<()>, _>(|_| {
                    thread_entered.wait();
                    Poll::Pending
                })
                .await
            }))
        });
        entered.wait();
        signal.cancel();
        assert_eq!(
            handle.join().expect("cancellation thread should join"),
            Err(SummaryError::InvalidState)
        );
    }
}
