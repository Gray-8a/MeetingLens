use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};
#[cfg(debug_assertions)]
use std::time::Duration;

use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    SummaryActionItem, SummaryActionStatus, SummaryClaim, SummaryContent, SummaryDecision,
    SummaryDecisionStatus, SummaryOutputLanguage, SummarySnapshotSegment, SummaryTemplate,
};

pub const MOCK_SUMMARY_PROVIDER: &str = "deterministic_mock";
pub const MOCK_SUMMARY_PROVIDER_MODE: &str = "mock";
pub const MOCK_SUMMARY_MODEL: &str = "meetinglens-deterministic-mock-v1";
pub const MOCK_SUMMARY_PROMPT_VERSION: &str = "summary-core-mock-v1";

#[cfg(debug_assertions)]
const INTERRUPTION_SMOKE_TITLE_PREFIX: &str = "Synthetic summary run interruption smoke";

#[derive(Clone, PartialEq)]
pub struct SummaryProviderRequest {
    pub run_id: String,
    pub meeting_id: String,
    pub meeting_title: String,
    pub meeting_primary_language: String,
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
            .field("meeting_title", &"[REDACTED]")
            .field("meeting_primary_language", &self.meeting_primary_language)
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
    fn generate(&self, request: &SummaryProviderRequest) -> SummaryResult<SummaryProviderOutput>;

    fn cancel(&self, _run_id: &str) -> SummaryResult<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct DeterministicMockSummaryProvider {
    cancelled_runs: Arc<Mutex<HashSet<String>>>,
    fail_generation: bool,
}

impl fmt::Debug for DeterministicMockSummaryProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeterministicMockSummaryProvider")
            .field("fail_generation", &self.fail_generation)
            .finish_non_exhaustive()
    }
}

impl DeterministicMockSummaryProvider {
    #[cfg(test)]
    pub(crate) fn failing() -> Self {
        Self {
            cancelled_runs: Arc::default(),
            fail_generation: true,
        }
    }

    fn take_cancelled(&self, run_id: &str) -> SummaryResult<bool> {
        self.cancelled_runs
            .lock()
            .map(|mut cancelled| cancelled.remove(run_id))
            .map_err(|_| SummaryError::Internal)
    }

    fn wait_for_interruption_smoke(&self, _request: &SummaryProviderRequest) -> SummaryResult<()> {
        #[cfg(debug_assertions)]
        if _request
            .meeting_title
            .starts_with(INTERRUPTION_SMOKE_TITLE_PREFIX)
        {
            for _ in 0..300 {
                if self.take_cancelled(&_request.run_id)? {
                    return Err(SummaryError::InvalidState);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        Ok(())
    }
}

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

    fn generate(&self, request: &SummaryProviderRequest) -> SummaryResult<SummaryProviderOutput> {
        if self.fail_generation {
            return Err(SummaryError::ProviderFailed);
        }
        if self.take_cancelled(&request.run_id)? {
            return Err(SummaryError::InvalidState);
        }
        self.wait_for_interruption_smoke(request)?;
        if self.take_cancelled(&request.run_id)? {
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
                    text: "Synthetic mock decision for Summary Run Core verification.".to_string(),
                    status: SummaryDecisionStatus::Confirmed,
                    evidence_ids: vec![first.evidence_id.clone()],
                }],
                action_items: vec![SummaryActionItem {
                    text:
                        "Review the deterministic mock result before enabling an external provider."
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
    }

    fn cancel(&self, run_id: &str) -> SummaryResult<()> {
        self.cancelled_runs
            .lock()
            .map_err(|_| SummaryError::Internal)?
            .insert(run_id.to_string());
        Ok(())
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
            meeting_title: "Synthetic title".to_string(),
            meeting_primary_language: "en-AU".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![
                segment("00000000-0000-4000-8000-000000000011", 0),
                segment("00000000-0000-4000-8000-000000000012", 1),
            ],
        };

        let provider = DeterministicMockSummaryProvider::default();
        let first = provider.generate(&request).expect("mock should generate");
        let second = provider
            .generate(&request)
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
            meeting_title: "Synthetic title".to_string(),
            meeting_primary_language: "en-AU".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: Vec::new(),
        };

        assert_eq!(
            DeterministicMockSummaryProvider::default().generate(&request),
            Err(SummaryError::SnapshotMissing)
        );
    }

    #[test]
    fn deterministic_mock_supports_test_failure_and_cooperative_cancel() {
        let request = SummaryProviderRequest {
            run_id: "00000000-0000-4000-8000-000000000010".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000001".to_string(),
            meeting_title: "Synthetic title".to_string(),
            meeting_primary_language: "en-AU".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: SummaryOutputLanguage::En,
            segments: vec![segment("00000000-0000-4000-8000-000000000011", 0)],
        };

        assert_eq!(
            DeterministicMockSummaryProvider::failing().generate(&request),
            Err(SummaryError::ProviderFailed)
        );

        let provider = DeterministicMockSummaryProvider::default();
        provider
            .cancel(&request.run_id)
            .expect("mock cancellation should register");
        assert_eq!(provider.generate(&request), Err(SummaryError::InvalidState));
    }
}
