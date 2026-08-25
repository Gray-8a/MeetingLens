use std::collections::{HashMap, HashSet};
use std::fmt;

use super::summary_error::{SummaryError, SummaryResult};
use super::summary_model::{
    SUMMARY_MARKDOWN_MAX_BYTES, SummaryActionItem, SummaryClaim, SummaryDecision, SummaryEvidence,
    SummaryOutputLanguage, SummaryRun, SummaryStatus, summary_content_version,
};
use super::summary_repository::{self, SummaryMarkdownMetadata};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RenderedSummaryMarkdown {
    pub(crate) summary_id: String,
    pub(crate) content_version: String,
    pub(crate) markdown: String,
    pub(crate) bytes: usize,
    pub(crate) suggested_file_name: String,
}

impl fmt::Debug for RenderedSummaryMarkdown {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderedSummaryMarkdown")
            .field("summary_id", &self.summary_id)
            .field("content_version", &"[REDACTED]")
            .field("markdown", &"[REDACTED]")
            .field("bytes", &self.bytes)
            .field("suggested_file_name", &"[REDACTED]")
            .finish()
    }
}

struct Labels {
    title: &'static str,
    revision: &'static str,
    template: &'static str,
    generated: &'static str,
    edited_notice: &'static str,
    stale_revised_notice: &'static str,
    stale_deleted_notice: &'static str,
    overview: &'static str,
    decisions: &'static str,
    actions: &'static str,
    risks: &'static str,
    questions: &'static str,
    evidence: &'static str,
    status: &'static str,
    decision_confirmed: &'static str,
    decision_proposed: &'static str,
    decision_superseded: &'static str,
    action_assigned: &'static str,
    action_suggested: &'static str,
    action_unconfirmed: &'static str,
    owner: &'static str,
    due: &'static str,
    unassigned: &'static str,
    not_set: &'static str,
    source_deleted: &'static str,
    empty: &'static str,
}

const EN: Labels = Labels {
    title: "Meeting Minutes",
    revision: "Revision",
    template: "Template",
    generated: "Generated",
    edited_notice: "This revision contains user edits.",
    stale_revised_notice: "The source transcript changed after this revision was generated.",
    stale_deleted_notice: "A source transcript was deleted after this revision was generated.",
    overview: "Overview",
    decisions: "Decisions",
    actions: "Action items",
    risks: "Risks",
    questions: "Open questions",
    evidence: "Evidence",
    status: "Status",
    decision_confirmed: "confirmed",
    decision_proposed: "proposed",
    decision_superseded: "superseded",
    action_assigned: "assigned",
    action_suggested: "suggested",
    action_unconfirmed: "unconfirmed",
    owner: "Owner",
    due: "Due",
    unassigned: "Unassigned",
    not_set: "Not set",
    source_deleted: "source deleted",
    empty: "None recorded.",
};

const ZH: Labels = Labels {
    title: "会议纪要",
    revision: "修订版本",
    template: "模板",
    generated: "生成时间",
    edited_notice: "此修订版本包含用户修改。",
    stale_revised_notice: "此修订版本生成后，来源转录已被修改。",
    stale_deleted_notice: "此修订版本生成后，一个来源转录已被删除。",
    overview: "概述",
    decisions: "决定",
    actions: "行动项",
    risks: "风险",
    questions: "待确认问题",
    evidence: "证据",
    status: "状态",
    decision_confirmed: "已确认",
    decision_proposed: "建议",
    decision_superseded: "已取代",
    action_assigned: "已分配",
    action_suggested: "建议",
    action_unconfirmed: "未确认",
    owner: "负责人",
    due: "截止时间",
    unassigned: "未分配",
    not_set: "未设置",
    source_deleted: "来源已删除",
    empty: "暂无记录。",
};

const BILINGUAL: Labels = Labels {
    title: "Meeting Minutes / 会议纪要",
    revision: "Revision / 修订版本",
    template: "Template / 模板",
    generated: "Generated / 生成时间",
    edited_notice: "This revision contains user edits. / 此修订版本包含用户修改。",
    stale_revised_notice: "The source transcript changed after this revision was generated. / 此修订版本生成后，来源转录已被修改。",
    stale_deleted_notice: "A source transcript was deleted after this revision was generated. / 此修订版本生成后，一个来源转录已被删除。",
    overview: "Overview / 概述",
    decisions: "Decisions / 决定",
    actions: "Action items / 行动项",
    risks: "Risks / 风险",
    questions: "Open questions / 待确认问题",
    evidence: "Evidence / 证据",
    status: "Status / 状态",
    decision_confirmed: "confirmed / 已确认",
    decision_proposed: "proposed / 建议",
    decision_superseded: "superseded / 已取代",
    action_assigned: "assigned / 已分配",
    action_suggested: "suggested / 建议",
    action_unconfirmed: "unconfirmed / 未确认",
    owner: "Owner / 负责人",
    due: "Due / 截止时间",
    unassigned: "Unassigned / 未分配",
    not_set: "Not set / 未设置",
    source_deleted: "source deleted / 来源已删除",
    empty: "None recorded. / 暂无记录。",
};

pub(crate) fn render(
    run: &SummaryRun,
    metadata: &SummaryMarkdownMetadata,
    evidence: &[SummaryEvidence],
) -> SummaryResult<RenderedSummaryMarkdown> {
    if run.status != SummaryStatus::Completed {
        return Err(SummaryError::InvalidState);
    }
    let content = run.content.as_ref().ok_or(SummaryError::InvalidState)?;
    if content.schema_version != 1 {
        return Err(SummaryError::PersistenceFailed);
    }
    let content_version = summary_content_version(content)?;
    let evidence_ids = summary_repository::referenced_evidence_ids(content);
    let evidence_by_id = validate_evidence(run, &evidence_ids, evidence)?;
    let citation_labels = evidence_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), format!("E{}", index + 1)))
        .collect::<HashMap<_, _>>();
    let labels = labels(run.output_language);

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(labels.title);
    output.push_str(": ");
    output.push_str(&escape_markdown(&metadata.meeting_title));
    output.push_str("\n\n");
    output.push_str("- **");
    output.push_str(labels.revision);
    output.push_str(":** ");
    output.push_str(&run.revision.to_string());
    output.push_str("\n- **");
    output.push_str(labels.template);
    output.push_str(":** ");
    output.push_str(run.template.as_str());
    if let Some(generated) = &run.finished_at {
        output.push_str("\n- **");
        output.push_str(labels.generated);
        output.push_str(":** ");
        output.push_str(&escape_markdown(generated));
    }
    output.push('\n');
    if run.is_user_edited {
        output.push_str("\n> ");
        output.push_str(labels.edited_notice);
        output.push('\n');
    }
    if run.is_stale {
        output.push_str("\n> ");
        output.push_str(if run.stale_reason.as_deref() == Some("source_deleted") {
            labels.stale_deleted_notice
        } else {
            labels.stale_revised_notice
        });
        output.push('\n');
    }

    push_heading(&mut output, labels.overview);
    push_claim(&mut output, &content.overview, &citation_labels);
    push_heading(&mut output, labels.decisions);
    push_decisions(&mut output, &content.decisions, &citation_labels, labels);
    push_heading(&mut output, labels.actions);
    push_actions(&mut output, &content.action_items, &citation_labels, labels);
    push_heading(&mut output, labels.risks);
    push_claims(&mut output, &content.risks, &citation_labels, labels);
    push_heading(&mut output, labels.questions);
    push_claims(
        &mut output,
        &content.open_questions,
        &citation_labels,
        labels,
    );
    push_heading(&mut output, labels.evidence);
    for evidence_id in &evidence_ids {
        let item = evidence_by_id
            .get(evidence_id.as_str())
            .copied()
            .ok_or(SummaryError::EvidenceInvalid)?;
        let citation = citation_labels
            .get(evidence_id.as_str())
            .ok_or(SummaryError::EvidenceInvalid)?;
        output.push_str("- **[");
        output.push_str(citation);
        output.push_str("]** ");
        if let Some(speaker) = &item.speaker_snapshot {
            output.push_str(&escape_markdown(speaker));
            output.push_str(" — ");
        }
        if let Some(offset) = item.start_offset_ms {
            output.push_str(&format_offset(offset));
            output.push_str(": ");
        }
        output.push_str(&escape_markdown(&item.excerpt));
        if item.source_deleted {
            output.push_str(" _(");
            output.push_str(labels.source_deleted);
            output.push_str(")_");
        }
        output.push('\n');
    }
    if evidence_ids.is_empty() {
        output.push_str(labels.empty);
        output.push('\n');
    }
    while output.ends_with("\n\n") {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    let bytes = output.len();
    if bytes > SUMMARY_MARKDOWN_MAX_BYTES {
        return Err(SummaryError::MarkdownTooLarge);
    }

    Ok(RenderedSummaryMarkdown {
        summary_id: run.id.clone(),
        content_version,
        markdown: output,
        bytes,
        suggested_file_name: suggested_file_name(&metadata.meeting_title, run.revision),
    })
}

fn validate_evidence<'a>(
    run: &SummaryRun,
    referenced_ids: &[String],
    evidence: &'a [SummaryEvidence],
) -> SummaryResult<HashMap<&'a str, &'a SummaryEvidence>> {
    if referenced_ids.len() != evidence.len() {
        return Err(SummaryError::EvidenceInvalid);
    }
    let mut by_id = HashMap::with_capacity(evidence.len());
    for item in evidence {
        if item.meeting_id != run.meeting_id
            || item.summary_id != run.id
            || by_id.insert(item.id.as_str(), item).is_some()
        {
            return Err(SummaryError::EvidenceInvalid);
        }
    }
    if referenced_ids.iter().collect::<HashSet<_>>().len() != referenced_ids.len()
        || referenced_ids
            .iter()
            .any(|id| !by_id.contains_key(id.as_str()))
    {
        return Err(SummaryError::EvidenceInvalid);
    }
    Ok(by_id)
}

fn labels(language: SummaryOutputLanguage) -> &'static Labels {
    match language {
        SummaryOutputLanguage::En => &EN,
        SummaryOutputLanguage::Zh => &ZH,
        SummaryOutputLanguage::Bilingual => &BILINGUAL,
    }
}

fn push_heading(output: &mut String, heading: &str) {
    output.push_str("\n## ");
    output.push_str(heading);
    output.push_str("\n\n");
}

fn citations(ids: &[String], labels: &HashMap<&str, String>) -> String {
    ids.iter()
        .filter_map(|id| labels.get(id.as_str()))
        .map(|label| format!("[{label}]"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_claim(output: &mut String, claim: &SummaryClaim, labels: &HashMap<&str, String>) {
    output.push_str(&escape_markdown(&claim.text));
    output.push(' ');
    output.push_str(&citations(&claim.evidence_ids, labels));
    output.push('\n');
}

fn push_claims(
    output: &mut String,
    claims: &[SummaryClaim],
    citation_labels: &HashMap<&str, String>,
    labels: &Labels,
) {
    if claims.is_empty() {
        output.push_str(labels.empty);
        output.push('\n');
        return;
    }
    for (index, claim) in claims.iter().enumerate() {
        output.push_str(&(index + 1).to_string());
        output.push_str(". ");
        push_claim(output, claim, citation_labels);
    }
}

fn push_decisions(
    output: &mut String,
    decisions: &[SummaryDecision],
    citation_labels: &HashMap<&str, String>,
    labels: &Labels,
) {
    if decisions.is_empty() {
        output.push_str(labels.empty);
        output.push('\n');
        return;
    }
    for (index, decision) in decisions.iter().enumerate() {
        output.push_str(&(index + 1).to_string());
        output.push_str(". ");
        output.push_str(&escape_markdown(&decision.text));
        output.push_str(" — **");
        output.push_str(labels.status);
        output.push_str(":** ");
        output.push_str(match decision.status {
            super::summary_model::SummaryDecisionStatus::Confirmed => labels.decision_confirmed,
            super::summary_model::SummaryDecisionStatus::Proposed => labels.decision_proposed,
            super::summary_model::SummaryDecisionStatus::Superseded => labels.decision_superseded,
        });
        output.push(' ');
        output.push_str(&citations(&decision.evidence_ids, citation_labels));
        output.push('\n');
    }
}

fn push_actions(
    output: &mut String,
    actions: &[SummaryActionItem],
    citation_labels: &HashMap<&str, String>,
    labels: &Labels,
) {
    if actions.is_empty() {
        output.push_str(labels.empty);
        output.push('\n');
        return;
    }
    for (index, item) in actions.iter().enumerate() {
        output.push_str(&(index + 1).to_string());
        output.push_str(". ");
        output.push_str(&escape_markdown(&item.text));
        output.push_str(" — **");
        output.push_str(labels.owner);
        output.push_str(":** ");
        output.push_str(
            &item
                .owner
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| labels.unassigned.to_string()),
        );
        output.push_str(" — **");
        output.push_str(labels.due);
        output.push_str(":** ");
        output.push_str(
            &item
                .due_date
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| labels.not_set.to_string()),
        );
        output.push_str(" — **");
        output.push_str(labels.status);
        output.push_str(":** ");
        output.push_str(match item.status {
            super::summary_model::SummaryActionStatus::Assigned => labels.action_assigned,
            super::summary_model::SummaryActionStatus::Suggested => labels.action_suggested,
            super::summary_model::SummaryActionStatus::Unconfirmed => labels.action_unconfirmed,
        });
        output.push(' ');
        output.push_str(&citations(&item.evidence_ids, citation_labels));
        output.push('\n');
    }
}

fn escape_markdown(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '`'
                | '*'
                | '_'
                | '{'
                | '}'
                | '['
                | ']'
                | '<'
                | '>'
                | '('
                | ')'
                | '#'
                | '+'
                | '-'
                | '.'
                | '!'
                | '|'
        ) {
            escaped.push('\\');
        }
        if character == '\t' {
            escaped.push(' ');
        } else if character == '\n' || !character.is_control() {
            escaped.push(character);
        } else {
            escaped.push('\u{fffd}');
        }
    }
    escaped
}

fn format_offset(offset_ms: u64) -> String {
    let seconds = offset_ms / 1_000;
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn suggested_file_name(title: &str, revision: u64) -> String {
    let mut safe = String::with_capacity(title.len().min(80));
    let mut previous_space = false;
    for character in title.chars().take(80) {
        let replacement = if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) {
            '-'
        } else {
            character
        };
        if replacement.is_whitespace() {
            if !previous_space {
                safe.push(' ');
            }
            previous_space = true;
        } else {
            safe.push(replacement);
            previous_space = false;
        }
    }
    let safe = safe.trim_matches([' ', '.']);
    let safe = if safe.is_empty() { "meeting" } else { safe };
    format!("{safe}-minutes-r{revision}.md")
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::data::summary_model::{
        SummaryActionStatus, SummaryContent, SummaryDecisionStatus, SummaryTemplate,
    };

    fn run(language: SummaryOutputLanguage, evidence_id: &str) -> SummaryRun {
        SummaryRun {
            id: Uuid::new_v4().to_string(),
            meeting_id: Uuid::new_v4().to_string(),
            series_id: Uuid::new_v4().to_string(),
            previous_summary_id: None,
            revision: 1,
            run_purpose: "final".to_string(),
            trigger_kind: "manual".to_string(),
            template: SummaryTemplate::Detailed,
            output_language: language,
            status: SummaryStatus::Completed,
            provider: "deepseek".to_string(),
            provider_mode: "external".to_string(),
            model: "deepseek-v4-pro".to_string(),
            prompt_version: "summary-deepseek-v1".to_string(),
            content: Some(SummaryContent {
                schema_version: 1,
                overview: SummaryClaim {
                    text: "Synthetic <script>*overview*</script>".to_string(),
                    evidence_ids: vec![evidence_id.to_string()],
                },
                decisions: vec![
                    SummaryDecision {
                        text: "Synthetic confirmed decision".to_string(),
                        status: SummaryDecisionStatus::Confirmed,
                        evidence_ids: vec![evidence_id.to_string()],
                    },
                    SummaryDecision {
                        text: "Synthetic proposed decision".to_string(),
                        status: SummaryDecisionStatus::Proposed,
                        evidence_ids: vec![evidence_id.to_string()],
                    },
                    SummaryDecision {
                        text: "Synthetic superseded decision".to_string(),
                        status: SummaryDecisionStatus::Superseded,
                        evidence_ids: vec![evidence_id.to_string()],
                    },
                ],
                action_items: [
                    SummaryActionStatus::Assigned,
                    SummaryActionStatus::Suggested,
                    SummaryActionStatus::Unconfirmed,
                ]
                .into_iter()
                .map(|status| SummaryActionItem {
                    text: "Synthetic action".to_string(),
                    owner: None,
                    due_date: None,
                    status,
                    evidence_ids: vec![evidence_id.to_string()],
                })
                .collect(),
                risks: Vec::new(),
                open_questions: Vec::new(),
            }),
            error_code: None,
            source_count: 1,
            input_segment_count: 1,
            input_bytes: 32,
            is_stale: false,
            stale_reason: None,
            stale_at: None,
            is_user_edited: false,
            snapshot_created_at: "2026-08-17T00:00:00.000Z".to_string(),
            created_at: "2026-08-17T00:00:00.000Z".to_string(),
            started_at: Some("2026-08-17T00:00:01.000Z".to_string()),
            finished_at: Some("2026-08-17T00:00:02.000Z".to_string()),
            cancel_requested_at: None,
            updated_at: "2026-08-17T00:00:02.000Z".to_string(),
        }
    }

    #[test]
    fn renderer_is_deterministic_escaped_and_localizes_headings_and_statuses() {
        for (language, expected_heading, expected_statuses) in [
            (
                SummaryOutputLanguage::En,
                "## Overview",
                [
                    "confirmed",
                    "proposed",
                    "superseded",
                    "assigned",
                    "suggested",
                    "unconfirmed",
                ],
            ),
            (
                SummaryOutputLanguage::Zh,
                "## 概述",
                ["已确认", "建议", "已取代", "已分配", "建议", "未确认"],
            ),
            (
                SummaryOutputLanguage::Bilingual,
                "## Overview / 概述",
                [
                    "confirmed / 已确认",
                    "proposed / 建议",
                    "superseded / 已取代",
                    "assigned / 已分配",
                    "suggested / 建议",
                    "unconfirmed / 未确认",
                ],
            ),
        ] {
            let evidence_id = Uuid::new_v4().to_string();
            let run = run(language, &evidence_id);
            let localized = labels(language);
            assert_eq!(
                [
                    localized.decision_confirmed,
                    localized.decision_proposed,
                    localized.decision_superseded,
                    localized.action_assigned,
                    localized.action_suggested,
                    localized.action_unconfirmed,
                ],
                expected_statuses
            );
            let evidence = SummaryEvidence {
                id: evidence_id,
                summary_id: run.id.clone(),
                meeting_id: run.meeting_id.clone(),
                kind: "transcript".to_string(),
                source_id: Uuid::new_v4().to_string(),
                transcript_segment_id: Uuid::new_v4().to_string(),
                segment_revision: 1,
                label: "Synthetic label".to_string(),
                speaker_snapshot: Some("Speaker [one]".to_string()),
                start_offset_ms: Some(65_000),
                end_offset_ms: None,
                ordinal: 0,
                global_ordinal: 0,
                excerpt: "Synthetic evidence".to_string(),
                source_deleted: false,
                created_at: "2026-08-17T00:00:00.000Z".to_string(),
            };
            let metadata = SummaryMarkdownMetadata {
                meeting_title: "Synthetic <meeting>".to_string(),
            };
            let first = render(&run, &metadata, std::slice::from_ref(&evidence))
                .expect("Markdown should render");
            let second = render(&run, &metadata, &[evidence]).expect("Markdown should repeat");
            assert_eq!(first, second);
            assert!(first.markdown.contains(expected_heading));
            for expected_status in expected_statuses {
                assert!(first.markdown.contains(expected_status));
            }
            assert!(first.markdown.contains("[E1]"));
            assert!(first.markdown.contains("01:05"));
            assert!(!first.markdown.contains("<script>"));
            assert!(first.markdown.ends_with('\n'));
            assert!(!first.markdown.ends_with("\n\n"));
            assert_eq!(first.bytes, first.markdown.len());
            assert!(!first.markdown.contains(&run.id));
        }
    }
}
