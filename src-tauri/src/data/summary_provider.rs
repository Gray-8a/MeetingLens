use std::fmt;

use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    SummaryActionItem, SummaryActionStatus, SummaryClaim, SummaryContent, SummaryDecision,
    SummaryDecisionStatus, SummaryOutputLanguage, SummarySnapshotSegment, SummaryTemplate,
};

pub const MOCK_SUMMARY_PROVIDER: &str = "deterministic_mock";
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
    fn model_name(&self) -> &'static str;
    fn prompt_version(&self) -> &'static str;
    fn generate(&self, request: &SummaryProviderRequest) -> SummaryResult<SummaryProviderOutput>;

    fn cancel(&self, _run_id: &str) -> SummaryResult<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicMockSummaryProvider;

impl SummaryProvider for DeterministicMockSummaryProvider {
    fn provider_name(&self) -> &'static str {
        MOCK_SUMMARY_PROVIDER
    }

    fn model_name(&self) -> &'static str {
        MOCK_SUMMARY_MODEL
    }

    fn prompt_version(&self) -> &'static str {
        MOCK_SUMMARY_PROMPT_VERSION
    }

    fn generate(&self, request: &SummaryProviderRequest) -> SummaryResult<SummaryProviderOutput> {
        pause_for_development_interruption_smoke(request);
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
}

#[cfg(debug_assertions)]
fn pause_for_development_interruption_smoke(request: &SummaryProviderRequest) {
    if request
        .meeting_title
        .starts_with(INTERRUPTION_SMOKE_TITLE_PREFIX)
    {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}

#[cfg(not(debug_assertions))]
fn pause_for_development_interruption_smoke(_request: &SummaryProviderRequest) {}

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

        let first = DeterministicMockSummaryProvider
            .generate(&request)
            .expect("mock should generate");
        let second = DeterministicMockSummaryProvider
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
            DeterministicMockSummaryProvider.generate(&request),
            Err(SummaryError::SnapshotMissing)
        );
    }
}
