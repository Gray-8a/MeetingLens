use std::fmt;

use serde::{Deserialize, Serialize};

use super::transcript_error::{TranscriptError, TranscriptResult};

pub const PASTED_TEXT_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const TRANSCRIPT_FILE_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const TRANSCRIPT_MAX_SEGMENTS: usize = 20_000;
pub const TRANSCRIPT_SEGMENT_MAX_BYTES: usize = 64 * 1024;
pub const TRANSCRIPT_PAGE_DEFAULT_LIMIT: u64 = 100;
pub const TRANSCRIPT_PAGE_MAX_LIMIT: u64 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranscriptPolicy {
    pub max_pasted_bytes: usize,
    pub max_file_bytes: usize,
    pub max_segments: usize,
    pub max_segment_bytes: usize,
}

#[derive(Clone, PartialEq)]
pub struct TranscriptImportInput {
    pub display_name: String,
    pub original_filename: Option<String>,
    pub content: Vec<u8>,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
}

impl Default for TranscriptPolicy {
    fn default() -> Self {
        Self {
            max_pasted_bytes: PASTED_TEXT_MAX_BYTES,
            max_file_bytes: TRANSCRIPT_FILE_MAX_BYTES,
            max_segments: TRANSCRIPT_MAX_SEGMENTS,
            max_segment_bytes: TRANSCRIPT_SEGMENT_MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSourceType {
    ImportedSrt,
    ImportedTxt,
    ImportedVtt,
    LiveAsr,
    PastedText,
    PlatformExport,
}

impl TranscriptSourceType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImportedSrt => "imported_srt",
            Self::ImportedTxt => "imported_txt",
            Self::ImportedVtt => "imported_vtt",
            Self::LiveAsr => "live_asr",
            Self::PastedText => "pasted_text",
            Self::PlatformExport => "platform_export",
        }
    }

    pub const fn is_currently_importable(self) -> bool {
        matches!(
            self,
            Self::ImportedSrt | Self::ImportedTxt | Self::ImportedVtt | Self::PastedText
        )
    }

    pub(crate) fn from_database(value: &str) -> TranscriptResult<Self> {
        match value {
            "imported_srt" => Ok(Self::ImportedSrt),
            "imported_txt" => Ok(Self::ImportedTxt),
            "imported_vtt" => Ok(Self::ImportedVtt),
            "live_asr" => Ok(Self::LiveAsr),
            "pasted_text" => Ok(Self::PastedText),
            "platform_export" => Ok(Self::PlatformExport),
            _ => Err(TranscriptError::PersistenceFailed),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSource {
    pub id: String,
    pub meeting_id: String,
    pub source_type: TranscriptSourceType,
    pub display_name: String,
    pub original_filename: Option<String>,
    pub content_sha256: String,
    pub language: Option<String>,
    pub base_started_at: Option<String>,
    pub parser_name: String,
    pub parser_version: String,
    pub imported_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, PartialEq)]
pub struct TranscriptSourceOverview {
    pub source: TranscriptSource,
    pub segment_count: u64,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub speaker_count: u64,
}

impl fmt::Debug for TranscriptSourceOverview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSourceOverview")
            .field("id", &self.source.id)
            .field("meeting_id", &self.source.meeting_id)
            .field("source_type", &self.source.source_type)
            .field("display_name", &"[REDACTED]")
            .field(
                "original_filename",
                &self.source.original_filename.as_ref().map(|_| "[REDACTED]"),
            )
            .field("segment_count", &self.segment_count)
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("speaker_count", &self.speaker_count)
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub meeting_id: String,
    pub source_id: String,
    pub ordinal: u64,
    pub speaker_label: Option<String>,
    pub speaker_name: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub original_text: String,
    pub translated_text: Option<String>,
    pub language: Option<String>,
    pub confidence: Option<f64>,
    pub is_final: bool,
    pub revision: u64,
    pub created_at: String,
    pub corrected_at: Option<String>,
}

impl fmt::Debug for TranscriptSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSegment")
            .field("id", &self.id)
            .field("meeting_id", &self.meeting_id)
            .field("source_id", &self.source_id)
            .field("ordinal", &self.ordinal)
            .field(
                "speaker_label",
                &self.speaker_label.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("original_text", &"[REDACTED]")
            .field(
                "translated_text",
                &self.translated_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("language", &self.language)
            .field("confidence", &self.confidence)
            .field("is_final", &self.is_final)
            .field("revision", &self.revision)
            .field("created_at", &self.created_at)
            .field("corrected_at", &self.corrected_at)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct TranscriptSegmentPage {
    pub items: Vec<TranscriptSegment>,
    pub next_cursor: Option<u64>,
    pub has_more: bool,
    pub total_count: u64,
}

impl fmt::Debug for TranscriptSegmentPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSegmentPage")
            .field("item_count", &self.items.len())
            .field("next_cursor", &self.next_cursor)
            .field("has_more", &self.has_more)
            .field("total_count", &self.total_count)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct TranscriptSpeakerGroup {
    pub speaker_label: String,
    pub speaker_name: Option<String>,
    pub has_mixed_names: bool,
    pub segment_count: u64,
}

impl fmt::Debug for TranscriptSpeakerGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSpeakerGroup")
            .field("speaker_label", &"[REDACTED]")
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field("has_mixed_names", &self.has_mixed_names)
            .field("segment_count", &self.segment_count)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct TranscriptSpeakerGroupPage {
    pub items: Vec<TranscriptSpeakerGroup>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total_count: u64,
}

impl fmt::Debug for TranscriptSpeakerGroupPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptSpeakerGroupPage")
            .field("item_count", &self.items.len())
            .field(
                "next_cursor",
                &self.next_cursor.as_ref().map(|_| "[REDACTED]"),
            )
            .field("has_more", &self.has_more)
            .field("total_count", &self.total_count)
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranscriptSegment {
    pub speaker_name: Option<Option<String>>,
    pub original_text: Option<String>,
    pub expected_revision: u64,
}

impl fmt::Debug for UpdateTranscriptSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpdateTranscriptSegment")
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "original_text",
                &self.original_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expected_revision", &self.expected_revision)
            .finish()
    }
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryInputSegment {
    pub segment_id: String,
    pub source_id: String,
    pub ordinal: u64,
    pub speaker_label: Option<String>,
    pub speaker_name: Option<String>,
    pub start_offset_ms: Option<u64>,
    pub end_offset_ms: Option<u64>,
    pub original_text: String,
    pub translated_text: Option<String>,
    pub language: Option<String>,
    pub confidence: Option<f64>,
    pub is_final: bool,
    pub revision: u64,
}

impl fmt::Debug for SummaryInputSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SummaryInputSegment")
            .field("segment_id", &self.segment_id)
            .field("source_id", &self.source_id)
            .field("ordinal", &self.ordinal)
            .field(
                "speaker_label",
                &self.speaker_label.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "speaker_name",
                &self.speaker_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field("start_offset_ms", &self.start_offset_ms)
            .field("end_offset_ms", &self.end_offset_ms)
            .field("original_text", &"[REDACTED]")
            .field(
                "translated_text",
                &self.translated_text.as_ref().map(|_| "[REDACTED]"),
            )
            .field("language", &self.language)
            .field("confidence", &self.confidence)
            .field("is_final", &self.is_final)
            .field("revision", &self.revision)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum TranscriptImportOutcome {
    Imported {
        source: Box<TranscriptSource>,
        segment_count: usize,
    },
    Duplicate {
        source_id: String,
    },
}

#[derive(Clone, PartialEq)]
pub(crate) struct NewTranscriptSource {
    pub(crate) id: String,
    pub(crate) meeting_id: String,
    pub(crate) source_type: TranscriptSourceType,
    pub(crate) display_name: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) content_sha256: String,
    pub(crate) language: Option<String>,
    pub(crate) base_started_at: Option<String>,
    pub(crate) parser_name: String,
    pub(crate) parser_version: String,
}

#[derive(Clone, PartialEq)]
pub(crate) struct NewTranscriptSegment {
    pub(crate) id: String,
    pub(crate) meeting_id: String,
    pub(crate) source_id: String,
    pub(crate) ordinal: u64,
    pub(crate) speaker_label: Option<String>,
    pub(crate) speaker_name: Option<String>,
    pub(crate) start_offset_ms: Option<u64>,
    pub(crate) end_offset_ms: Option<u64>,
    pub(crate) original_text: String,
    pub(crate) translated_text: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) is_final: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_limits_match_the_frozen_policy() {
        assert_eq!(
            TranscriptPolicy::default(),
            TranscriptPolicy {
                max_pasted_bytes: 2 * 1024 * 1024,
                max_file_bytes: 10 * 1024 * 1024,
                max_segments: 20_000,
                max_segment_bytes: 64 * 1024,
            }
        );
    }

    #[test]
    fn only_the_first_four_source_types_are_currently_importable() {
        for source_type in [
            TranscriptSourceType::ImportedSrt,
            TranscriptSourceType::ImportedTxt,
            TranscriptSourceType::ImportedVtt,
            TranscriptSourceType::PastedText,
        ] {
            assert!(source_type.is_currently_importable());
        }
        for source_type in [
            TranscriptSourceType::LiveAsr,
            TranscriptSourceType::PlatformExport,
        ] {
            assert!(!source_type.is_currently_importable());
        }
    }

    #[test]
    fn transcript_debug_output_redacts_body_and_speaker_data() {
        let segment = TranscriptSegment {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            meeting_id: "00000000-0000-4000-8000-000000000002".to_string(),
            source_id: "00000000-0000-4000-8000-000000000003".to_string(),
            ordinal: 0,
            speaker_label: Some("Synthetic private label".to_string()),
            speaker_name: Some("Synthetic private name".to_string()),
            start_offset_ms: None,
            end_offset_ms: None,
            original_text: "Synthetic private transcript body".to_string(),
            translated_text: Some("Synthetic private translation".to_string()),
            language: Some("en-AU".to_string()),
            confidence: None,
            is_final: true,
            revision: 1,
            created_at: "2026-08-06T00:00:00.000Z".to_string(),
            corrected_at: None,
        };
        let debug = format!("{segment:?}");
        for private in [
            "Synthetic private label",
            "Synthetic private name",
            "Synthetic private transcript body",
            "Synthetic private translation",
        ] {
            assert!(!debug.contains(private));
        }
        assert!(debug.contains("[REDACTED]"));
    }
}
