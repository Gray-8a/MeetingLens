use sha2::{Digest, Sha256};

use super::transcript_error::{TranscriptError, TranscriptResult};
use super::transcript_model::{StructuredAudioSegment, TranscriptPolicy, TranscriptSourceType};

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const HASH_DOMAIN_CUES: &[u8] = b"meetinglens-transcript-cues-v1\0";
const MAX_SPEAKER_LABEL_BYTES: usize = 200;
const MAX_SPEAKER_LABEL_CHARS: usize = 80;

#[path = "transcript_text_export.rs"]
mod text_export;

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ParsedTranscript {
    pub(crate) content_sha256: String,
    pub(crate) parser_name: String,
    pub(crate) parser_version: String,
    pub(crate) segments: Vec<ParsedSegment>,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ParsedSegment {
    pub(crate) speaker_label: Option<String>,
    pub(crate) start_offset_ms: Option<u64>,
    pub(crate) end_offset_ms: Option<u64>,
    pub(crate) original_text: String,
}

pub(crate) fn parse(
    source_type: TranscriptSourceType,
    input: &[u8],
    policy: TranscriptPolicy,
) -> TranscriptResult<ParsedTranscript> {
    let max_bytes = match source_type {
        TranscriptSourceType::PastedText => policy.max_pasted_bytes,
        TranscriptSourceType::ImportedTxt
        | TranscriptSourceType::ImportedSrt
        | TranscriptSourceType::ImportedVtt => policy.max_file_bytes,
        TranscriptSourceType::ImportedAudio
        | TranscriptSourceType::LiveAsr
        | TranscriptSourceType::PlatformExport => {
            return Err(TranscriptError::InvalidInput);
        }
    };
    if input.len() > max_bytes {
        return Err(TranscriptError::TooLarge);
    }

    let normalized = decode_and_normalize(input)?;
    if normalized.trim().is_empty() {
        return Err(TranscriptError::Empty);
    }

    match source_type {
        TranscriptSourceType::PastedText | TranscriptSourceType::ImportedTxt => {
            let exported = text_export::parse_export(&normalized, policy)?;
            let is_export = exported.is_some();
            let segments = match exported {
                Some(segments) => segments,
                None => parse_plain_text(&normalized, policy)?,
            };
            Ok(ParsedTranscript {
                content_sha256: hash_plain(&normalized),
                parser_name: if is_export {
                    "speaker_time_text"
                } else {
                    "plain_text"
                }
                .to_string(),
                parser_version: "1".to_string(),
                segments,
            })
        }
        TranscriptSourceType::ImportedSrt => {
            let segments = parse_srt(&normalized, policy)?;
            Ok(ParsedTranscript {
                content_sha256: hash_cues(&segments),
                parser_name: "srt".to_string(),
                parser_version: "1".to_string(),
                segments,
            })
        }
        TranscriptSourceType::ImportedVtt => {
            let segments = parse_vtt(&normalized, policy)?;
            Ok(ParsedTranscript {
                content_sha256: hash_cues(&segments),
                parser_name: "vtt".to_string(),
                parser_version: "1".to_string(),
                segments,
            })
        }
        TranscriptSourceType::ImportedAudio
        | TranscriptSourceType::LiveAsr
        | TranscriptSourceType::PlatformExport => Err(TranscriptError::InvalidInput),
    }
}

pub(crate) fn from_structured_audio(
    segments: Vec<StructuredAudioSegment>,
    parser_version: String,
    policy: TranscriptPolicy,
) -> TranscriptResult<ParsedTranscript> {
    if segments.is_empty() {
        return Err(TranscriptError::Empty);
    }
    if segments.len() > policy.max_segments {
        return Err(TranscriptError::SegmentLimitExceeded);
    }

    let mut parsed = Vec::with_capacity(segments.len());
    let mut total_text_bytes = 0_usize;
    let mut previous_end = None;
    for segment in segments {
        if segment.start_offset_ms > i64::MAX as u64
            || segment.end_offset_ms > i64::MAX as u64
            || segment.end_offset_ms <= segment.start_offset_ms
            || previous_end.is_some_and(|end| segment.start_offset_ms < end)
        {
            return Err(TranscriptError::InvalidTimestamp);
        }
        let original_text =
            normalize_structured_segment_text(&segment.original_text, policy.max_segment_bytes)?;
        total_text_bytes = total_text_bytes
            .checked_add(original_text.len())
            .ok_or(TranscriptError::TooLarge)?;
        if total_text_bytes > policy.max_file_bytes {
            return Err(TranscriptError::TooLarge);
        }
        previous_end = Some(segment.end_offset_ms);
        parsed.push(ParsedSegment {
            speaker_label: None,
            start_offset_ms: Some(segment.start_offset_ms),
            end_offset_ms: Some(segment.end_offset_ms),
            original_text,
        });
    }

    Ok(ParsedTranscript {
        content_sha256: hash_cues(&parsed),
        parser_name: "faster_whisper".to_string(),
        parser_version,
        segments: parsed,
    })
}

fn normalize_structured_segment_text(value: &str, max_bytes: usize) -> TranscriptResult<String> {
    let normalized = value.replace("\r\n", "\n");
    if normalized.contains('\r') || normalized.chars().any(is_forbidden_control) {
        return Err(TranscriptError::InvalidEncoding);
    }
    let normalized = normalized.trim();
    validate_segment_text(
        normalized,
        TranscriptPolicy {
            max_segment_bytes: max_bytes,
            ..TranscriptPolicy::default()
        },
    )?;
    Ok(normalized.to_string())
}

fn decode_and_normalize(input: &[u8]) -> TranscriptResult<String> {
    if input.starts_with(&[0xff, 0xfe])
        || input.starts_with(&[0xfe, 0xff])
        || input.starts_with(&[0xff, 0xfe, 0x00, 0x00])
        || input.starts_with(&[0x00, 0x00, 0xfe, 0xff])
    {
        return Err(TranscriptError::InvalidEncoding);
    }

    let input = input.strip_prefix(UTF8_BOM).unwrap_or(input);
    let text = std::str::from_utf8(input).map_err(|_| TranscriptError::InvalidEncoding)?;
    if text.chars().any(is_forbidden_control) {
        return Err(TranscriptError::InvalidEncoding);
    }

    let normalized = text.replace("\r\n", "\n");
    if normalized.contains('\r') {
        return Err(TranscriptError::InvalidEncoding);
    }
    Ok(normalized)
}

fn is_forbidden_control(character: char) -> bool {
    (character < ' ' && !matches!(character, '\t' | '\n' | '\r')) || character == '\u{7f}'
}

fn parse_plain_text(
    normalized: &str,
    policy: TranscriptPolicy,
) -> TranscriptResult<Vec<ParsedSegment>> {
    let mut segments = Vec::new();
    let mut paragraph = Vec::new();

    for line in normalized.split('\n') {
        if line.trim().is_empty() {
            flush_plain_paragraph(&mut paragraph, &mut segments, policy)?;
        } else {
            paragraph.push(line);
        }
    }
    flush_plain_paragraph(&mut paragraph, &mut segments, policy)?;

    if segments.is_empty() {
        return Err(TranscriptError::Empty);
    }
    Ok(segments)
}

fn flush_plain_paragraph(
    paragraph: &mut Vec<&str>,
    segments: &mut Vec<ParsedSegment>,
    policy: TranscriptPolicy,
) -> TranscriptResult<()> {
    if paragraph.is_empty() {
        return Ok(());
    }
    if segments.len() >= policy.max_segments {
        return Err(TranscriptError::SegmentLimitExceeded);
    }

    let text = paragraph.join("\n");
    paragraph.clear();
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    let (speaker_label, original_text) = extract_plain_speaker(text);
    validate_segment_text(&original_text, policy)?;
    segments.push(ParsedSegment {
        speaker_label,
        start_offset_ms: None,
        end_offset_ms: None,
        original_text,
    });
    Ok(())
}

fn extract_plain_speaker(text: &str) -> (Option<String>, String) {
    let first_line_end = text.find('\n').unwrap_or(text.len());
    let first_line = &text[..first_line_end];
    let delimiter = first_line
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '：'));
    let Some((delimiter_index, delimiter_char)) = delimiter else {
        return (None, text.to_string());
    };

    let candidate = first_line[..delimiter_index].trim();
    if !is_conservative_speaker_candidate(candidate) {
        return (None, text.to_string());
    }

    let body_start = delimiter_index + delimiter_char.len_utf8();
    let mut body = first_line[body_start..].trim_start().to_string();
    if first_line_end < text.len() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(text[first_line_end + 1..].trim_end());
    }
    let body = body.trim_end().to_string();
    if body.is_empty() {
        return (None, text.to_string());
    }

    (Some(candidate.to_string()), body)
}

fn is_conservative_speaker_candidate(candidate: &str) -> bool {
    if candidate.is_empty()
        || candidate.len() > MAX_SPEAKER_LABEL_BYTES
        || candidate.chars().count() > MAX_SPEAKER_LABEL_CHARS
        || !candidate.chars().any(char::is_alphabetic)
    {
        return false;
    }

    if !candidate.chars().all(|character| {
        character.is_alphanumeric()
            || character.is_whitespace()
            || matches!(character, '-' | '\'' | '’')
    }) {
        return false;
    }

    let ascii_lower = candidate.to_ascii_lowercase();
    if matches!(
        ascii_lower.as_str(),
        "http"
            | "https"
            | "ftp"
            | "file"
            | "mailto"
            | "if"
            | "else"
            | "for"
            | "while"
            | "fn"
            | "class"
            | "struct"
            | "return"
            | "todo"
            | "note"
            | "warning"
            | "error"
            | "debug"
            | "info"
            | "json"
            | "yaml"
            | "key"
            | "value"
            | "type"
            | "const"
            | "let"
            | "var"
            | "function"
            | "def"
            | "print"
            | "log"
    ) {
        return false;
    }

    if candidate.is_ascii() {
        let letters: Vec<char> = candidate
            .chars()
            .filter(|character| character.is_ascii_alphabetic())
            .collect();
        let all_upper =
            letters.len() > 3 && letters.iter().all(|character| character.is_uppercase());
        if all_upper {
            return false;
        }
    }

    true
}

fn parse_srt(normalized: &str, policy: TranscriptPolicy) -> TranscriptResult<Vec<ParsedSegment>> {
    let mut segments = Vec::new();
    for block in split_blocks(normalized) {
        if segments.len() >= policy.max_segments {
            return Err(TranscriptError::SegmentLimitExceeded);
        }
        let lines: Vec<&str> = block.lines().collect();
        if lines.is_empty() {
            continue;
        }
        let timing_index = if lines[0].contains("-->") {
            0
        } else if lines.len() >= 2
            && lines[0]
                .trim()
                .chars()
                .all(|character| character.is_ascii_digit())
            && lines[1].contains("-->")
        {
            1
        } else {
            return Err(TranscriptError::InvalidFormat);
        };
        let (start_offset_ms, end_offset_ms) =
            parse_timing_line(lines[timing_index], TimeSyntax::Srt)?;
        let cue_text = lines[timing_index + 1..].join("\n");
        let cue_text = cue_text.trim();
        if cue_text.is_empty() {
            return Err(TranscriptError::InvalidFormat);
        }
        let (speaker_label, original_text) = extract_plain_speaker(cue_text);
        validate_segment_text(&original_text, policy)?;
        segments.push(ParsedSegment {
            speaker_label,
            start_offset_ms: Some(start_offset_ms),
            end_offset_ms: Some(end_offset_ms),
            original_text,
        });
    }
    if segments.is_empty() {
        return Err(TranscriptError::Empty);
    }
    Ok(segments)
}

fn parse_vtt(normalized: &str, policy: TranscriptPolicy) -> TranscriptResult<Vec<ParsedSegment>> {
    let mut lines = normalized.lines();
    let header = lines.next().ok_or(TranscriptError::InvalidFormat)?;
    if !is_valid_vtt_header(header) {
        return Err(TranscriptError::InvalidFormat);
    }

    let mut body_started = false;
    let mut body = String::new();
    for line in lines {
        if !body_started {
            if line.trim().is_empty() {
                body_started = true;
            }
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    if !body_started {
        return Err(TranscriptError::InvalidFormat);
    }

    let mut segments = Vec::new();
    for block in split_blocks(&body) {
        let lines: Vec<&str> = block.lines().collect();
        if lines.is_empty() {
            continue;
        }
        let kind = lines[0].trim();
        if kind == "NOTE" || kind.starts_with("NOTE ") || kind == "STYLE" || kind == "REGION" {
            continue;
        }
        if segments.len() >= policy.max_segments {
            return Err(TranscriptError::SegmentLimitExceeded);
        }

        let timing_index = if lines[0].contains("-->") {
            0
        } else if lines.len() >= 2 && lines[1].contains("-->") {
            1
        } else {
            return Err(TranscriptError::InvalidFormat);
        };
        let (start_offset_ms, end_offset_ms) =
            parse_timing_line(lines[timing_index], TimeSyntax::Vtt)?;
        let cue_text = lines[timing_index + 1..].join("\n");
        let cue_text = cue_text.trim();
        if cue_text.is_empty() {
            return Err(TranscriptError::InvalidFormat);
        }
        let (speaker_label, original_text) = extract_vtt_voice(cue_text)?;
        validate_segment_text(&original_text, policy)?;
        segments.push(ParsedSegment {
            speaker_label,
            start_offset_ms: Some(start_offset_ms),
            end_offset_ms: Some(end_offset_ms),
            original_text,
        });
    }
    if segments.is_empty() {
        return Err(TranscriptError::Empty);
    }
    Ok(segments)
}

fn is_valid_vtt_header(header: &str) -> bool {
    if header == "WEBVTT" {
        return true;
    }
    header
        .strip_prefix("WEBVTT")
        .is_some_and(|suffix| suffix.starts_with([' ', '\t']) && !suffix.contains("-->"))
}

fn extract_vtt_voice(text: &str) -> TranscriptResult<(Option<String>, String)> {
    let Some(rest) = text.strip_prefix("<v ") else {
        return Ok((None, text.to_string()));
    };
    let close = rest.find('>').ok_or(TranscriptError::InvalidFormat)?;
    let label = rest[..close].trim();
    if label.is_empty()
        || label.len() > MAX_SPEAKER_LABEL_BYTES
        || label.chars().count() > MAX_SPEAKER_LABEL_CHARS
        || label.chars().any(is_forbidden_control)
        || label.contains(['<', '>'])
    {
        return Err(TranscriptError::InvalidFormat);
    }

    let mut body = rest[close + 1..].trim().to_string();
    if body.contains("</v>") {
        let Some(stripped) = body.strip_suffix("</v>") else {
            return Err(TranscriptError::InvalidFormat);
        };
        body = stripped.trim_end().to_string();
    }
    if body.is_empty() || body.contains("<v ") || body.contains("</v>") {
        return Err(TranscriptError::InvalidFormat);
    }
    Ok((Some(label.to_string()), body))
}

fn validate_segment_text(text: &str, policy: TranscriptPolicy) -> TranscriptResult<()> {
    if text.trim().is_empty() {
        return Err(TranscriptError::Empty);
    }
    if text.len() > policy.max_segment_bytes {
        return Err(TranscriptError::SegmentTooLarge);
    }
    Ok(())
}

fn split_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    blocks
}

#[derive(Clone, Copy)]
enum TimeSyntax {
    Srt,
    Vtt,
}

fn parse_timing_line(line: &str, syntax: TimeSyntax) -> TranscriptResult<(u64, u64)> {
    let (start, end_and_settings) = line
        .split_once("-->")
        .ok_or(TranscriptError::InvalidFormat)?;
    let mut end_parts = end_and_settings.split_whitespace();
    let end = end_parts.next().ok_or(TranscriptError::InvalidTimestamp)?;
    if matches!(syntax, TimeSyntax::Srt) && end_parts.next().is_some() {
        return Err(TranscriptError::InvalidFormat);
    }
    let start = parse_timestamp(start.trim(), syntax)?;
    let end = parse_timestamp(end, syntax)?;
    if end < start {
        return Err(TranscriptError::InvalidTimestamp);
    }
    Ok((start, end))
}

fn parse_timestamp(value: &str, syntax: TimeSyntax) -> TranscriptResult<u64> {
    let (whole, fraction) = match syntax {
        TimeSyntax::Srt => value.split_once(','),
        TimeSyntax::Vtt => value.split_once('.'),
    }
    .ok_or(TranscriptError::InvalidTimestamp)?;
    if fraction.len() != 3 || !fraction.chars().all(|character| character.is_ascii_digit()) {
        return Err(TranscriptError::InvalidTimestamp);
    }
    let milliseconds = fraction
        .parse::<u64>()
        .map_err(|_| TranscriptError::InvalidTimestamp)?;
    let parts: Vec<&str> = whole.split(':').collect();
    let (hours, minutes, seconds) = match (syntax, parts.as_slice()) {
        (TimeSyntax::Srt, [hours, minutes, seconds]) => (
            parse_time_part(hours)?,
            parse_time_part(minutes)?,
            parse_time_part(seconds)?,
        ),
        (TimeSyntax::Vtt, [minutes, seconds]) => {
            (0, parse_time_part(minutes)?, parse_time_part(seconds)?)
        }
        (TimeSyntax::Vtt, [hours, minutes, seconds]) => (
            parse_time_part(hours)?,
            parse_time_part(minutes)?,
            parse_time_part(seconds)?,
        ),
        _ => return Err(TranscriptError::InvalidTimestamp),
    };
    if minutes >= 60 || seconds >= 60 {
        return Err(TranscriptError::InvalidTimestamp);
    }
    hours
        .checked_mul(3_600_000)
        .and_then(|value| value.checked_add(minutes * 60_000))
        .and_then(|value| value.checked_add(seconds * 1_000))
        .and_then(|value| value.checked_add(milliseconds))
        .ok_or(TranscriptError::InvalidTimestamp)
}

fn parse_time_part(value: &str) -> TranscriptResult<u64> {
    if value.len() != 2 || !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(TranscriptError::InvalidTimestamp);
    }
    value
        .parse::<u64>()
        .map_err(|_| TranscriptError::InvalidTimestamp)
}

fn hash_plain(normalized: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    encode_hex(&hasher.finalize())
}

fn hash_cues(segments: &[ParsedSegment]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN_CUES);
    hash_u64(&mut hasher, segments.len() as u64);
    for segment in segments {
        hash_optional_u64(&mut hasher, segment.start_offset_ms);
        hash_optional_u64(&mut hasher, segment.end_offset_ms);
        hash_optional_text(&mut hasher, segment.speaker_label.as_deref());
        hash_text(&mut hasher, &segment.original_text);
    }
    encode_hex(&hasher.finalize())
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hash_optional_u64(hasher: &mut Sha256, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_u64(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn hash_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_input(
        source_type: TranscriptSourceType,
        input: &[u8],
    ) -> TranscriptResult<ParsedTranscript> {
        parse(source_type, input, TranscriptPolicy::default())
    }

    #[test]
    fn parses_plain_paragraphs_and_only_conservative_speakers() {
        let parsed = parse_input(
            TranscriptSourceType::PastedText,
            "Alice: First line\ncontinued\n\nalice: lowercase name\n\n张三：第二段\n\nhttps://example.test: not a speaker\n\n10:30 next topic\n\nif: code label\n\nfoo(): code label\n\nwww.example.test: URL label"
                .as_bytes(),
        )
        .expect("plain transcript should parse");

        assert_eq!(parsed.segments.len(), 8);
        assert_eq!(parsed.segments[0].speaker_label.as_deref(), Some("Alice"));
        assert_eq!(parsed.segments[0].original_text, "First line\ncontinued");
        assert_eq!(parsed.segments[1].speaker_label.as_deref(), Some("alice"));
        assert_eq!(parsed.segments[2].speaker_label.as_deref(), Some("张三"));
        assert_eq!(parsed.segments[3].speaker_label, None);
        assert_eq!(parsed.segments[4].speaker_label, None);
        assert_eq!(parsed.segments[5].speaker_label, None);
        assert_eq!(parsed.segments[6].speaker_label, None);
        assert_eq!(parsed.segments[7].speaker_label, None);
        assert!(
            parsed
                .segments
                .iter()
                .all(|segment| segment.start_offset_ms.is_none())
        );
    }

    #[test]
    fn accepts_utf8_bom_and_normalizes_newlines_for_plain_hashing() {
        let with_bom = parse_input(
            TranscriptSourceType::ImportedTxt,
            b"\xEF\xBB\xBFAlice: hello\r\n",
        )
        .expect("BOM text should parse");
        let normalized = parse_input(TranscriptSourceType::ImportedTxt, b"Alice: hello\n")
            .expect("normalized text should parse");
        assert_eq!(with_bom.content_sha256, normalized.content_sha256);
    }

    #[test]
    fn preserves_ordinary_whitespace_in_plain_hashing() {
        let first = parse_input(TranscriptSourceType::ImportedTxt, b"Alice: hello")
            .expect("first input should parse");
        let second = parse_input(TranscriptSourceType::ImportedTxt, b"Alice:  hello")
            .expect("second input should parse");
        assert_ne!(first.content_sha256, second.content_sha256);
    }

    #[test]
    fn rejects_invalid_encodings_nul_controls_and_empty_text() {
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedTxt, b"\xff\xfeA\0"),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedTxt,
                b"\xff\xfe\x00\x00A\x00\x00\x00"
            ),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedTxt,
                b"\x00\x00\xfe\xff\x00\x00\x00A"
            ),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedTxt, b"\x80invalid"),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedTxt, b"valid\0text"),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedTxt, b"isolated\rcarriage"),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedTxt, b" \n\t "),
            Err(TranscriptError::Empty)
        );
    }

    #[test]
    fn enforces_input_segment_and_segment_count_limits_without_truncating() {
        let tiny = TranscriptPolicy {
            max_pasted_bytes: 8,
            max_file_bytes: 4,
            max_segments: 1,
            max_segment_bytes: 5,
        };
        assert_eq!(
            parse(TranscriptSourceType::PastedText, b"123456789", tiny),
            Err(TranscriptError::TooLarge)
        );
        assert_eq!(
            parse(TranscriptSourceType::ImportedTxt, b"12345", tiny),
            Err(TranscriptError::TooLarge)
        );
        assert_eq!(
            parse(TranscriptSourceType::PastedText, b"123456", tiny),
            Err(TranscriptError::SegmentTooLarge)
        );
        assert_eq!(
            parse(TranscriptSourceType::PastedText, b"a\n\nb", tiny),
            Err(TranscriptError::SegmentLimitExceeded)
        );
    }

    #[test]
    fn parses_srt_offsets_order_and_explicit_speaker_labels() {
        let parsed = parse_input(
            TranscriptSourceType::ImportedSrt,
            b"1\r\n00:00:01,250 --> 00:00:02,500\r\nAlice: Hello <b>world</b>\r\n\r\n2\r\n00:00:03,000 --> 00:00:03,000\r\nSecond cue\r\n",
        )
        .expect("SRT should parse");
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(parsed.segments[0].start_offset_ms, Some(1_250));
        assert_eq!(parsed.segments[0].end_offset_ms, Some(2_500));
        assert_eq!(parsed.segments[0].speaker_label.as_deref(), Some("Alice"));
        assert_eq!(parsed.segments[0].original_text, "Hello <b>world</b>");
        assert_eq!(parsed.segments[1].start_offset_ms, Some(3_000));
        assert_eq!(parsed.segments[1].end_offset_ms, Some(3_000));
    }

    #[test]
    fn srt_fails_closed_for_malformed_or_reverse_time_cues() {
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedSrt,
                b"1\n00:00:02,000 --> 00:00:01,000\ninvalid"
            ),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedSrt,
                b"1\n00:00:01,000 --> bad\ninvalid"
            ),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedSrt, b"not a cue"),
            Err(TranscriptError::InvalidFormat)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedSrt,
                b"1\n00:00:01,000 --> 00:00:02,000 align:start\ninvalid"
            ),
            Err(TranscriptError::InvalidFormat)
        );
    }

    #[test]
    fn parses_vtt_voice_labels_and_ignores_non_body_blocks() {
        let parsed = parse_input(
            TranscriptSourceType::ImportedVtt,
            b"WEBVTT\n\nSTYLE\n::cue { color: lime; }\n\nNOTE synthetic metadata\nnot body\n\ncue-one\n00:01.000 --> 00:02.250 align:start\n<v Speaker 1>Hello</v>\n\n00:03.000 --> 00:04.000\n<script>stored only as text</script>\n",
        )
        .expect("VTT should parse");
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(
            parsed.segments[0].speaker_label.as_deref(),
            Some("Speaker 1")
        );
        assert_eq!(parsed.segments[0].original_text, "Hello");
        assert_eq!(parsed.segments[0].start_offset_ms, Some(1_000));
        assert_eq!(parsed.segments[0].end_offset_ms, Some(2_250));
        assert_eq!(
            parsed.segments[1].original_text,
            "<script>stored only as text</script>"
        );
    }

    #[test]
    fn vtt_requires_a_header_blank_separator_and_valid_timestamps() {
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedVtt,
                b"00:01.000 --> 00:02.000\ntext"
            ),
            Err(TranscriptError::InvalidFormat)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedVtt,
                b"WEBVTT\n00:01.000 --> 00:02.000\ntext"
            ),
            Err(TranscriptError::InvalidFormat)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedVtt,
                b"WEBVTT\n\n00:61.000 --> 01:02.000\ntext"
            ),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedVtt,
                b"WEBVTT --> invalid header\n\n00:01.000 --> 00:02.000\ntext"
            ),
            Err(TranscriptError::InvalidFormat)
        );
        assert_eq!(
            parse_input(
                TranscriptSourceType::ImportedVtt,
                b"WEBVTT\n\n00:01.000 --> 00:02.000\n<v Alice>Hello</v> <v Bob>World</v>"
            ),
            Err(TranscriptError::InvalidFormat)
        );
    }

    #[test]
    fn equivalent_srt_and_vtt_cues_have_the_same_canonical_hash() {
        let srt = parse_input(
            TranscriptSourceType::ImportedSrt,
            b"1\n00:00:01,000 --> 00:00:02,000\nAlice: Hello\n",
        )
        .expect("SRT should parse");
        let vtt = parse_input(
            TranscriptSourceType::ImportedVtt,
            b"WEBVTT\n\n00:01.000 --> 00:02.000\n<v Alice>Hello</v>\n",
        )
        .expect("VTT should parse");
        assert_eq!(srt.content_sha256, vtt.content_sha256);
    }

    #[test]
    fn future_source_types_cannot_be_created_by_the_parser() {
        assert_eq!(
            parse_input(TranscriptSourceType::ImportedAudio, b"text"),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::LiveAsr, b"text"),
            Err(TranscriptError::InvalidInput)
        );
        assert_eq!(
            parse_input(TranscriptSourceType::PlatformExport, b"text"),
            Err(TranscriptError::InvalidInput)
        );
    }

    #[test]
    fn structured_audio_has_a_canonical_cue_hash_and_no_speaker_metadata() {
        let audio = from_structured_audio(
            vec![StructuredAudioSegment {
                start_offset_ms: 1_000,
                end_offset_ms: 2_000,
                original_text: "  Hello\r\nworld  ".to_string(),
            }],
            "faster-whisper/1.2.1".to_string(),
            TranscriptPolicy::default(),
        )
        .expect("structured audio should adapt");
        let vtt = parse_input(
            TranscriptSourceType::ImportedVtt,
            b"WEBVTT\n\n00:01.000 --> 00:02.000\nHello\nworld\n",
        )
        .expect("equivalent VTT should parse");

        assert_eq!(audio.content_sha256, vtt.content_sha256);
        assert_eq!(audio.parser_name, "faster_whisper");
        assert_eq!(audio.parser_version, "faster-whisper/1.2.1");
        assert_eq!(audio.segments[0].speaker_label, None);
        assert_eq!(audio.segments[0].original_text, "Hello\nworld");
    }

    #[test]
    fn structured_audio_fails_closed_for_invalid_timestamps_and_text() {
        let segment = |start, end, text: &str| StructuredAudioSegment {
            start_offset_ms: start,
            end_offset_ms: end,
            original_text: text.to_string(),
        };
        let parse_audio = |segments| {
            from_structured_audio(
                segments,
                "faster-whisper/1.2.1".to_string(),
                TranscriptPolicy::default(),
            )
        };

        assert_eq!(parse_audio(vec![]), Err(TranscriptError::Empty));
        assert_eq!(
            parse_audio(vec![segment(2, 1, "body")]),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_audio(vec![segment(2, 2, "body")]),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_audio(vec![segment(2, 3, "first"), segment(1, 4, "second")]),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_audio(vec![segment(0, 3, "first"), segment(2, 4, "second")]),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_audio(vec![segment(
                i64::MAX as u64 + 1,
                i64::MAX as u64 + 1,
                "body"
            )]),
            Err(TranscriptError::InvalidTimestamp)
        );
        assert_eq!(
            parse_audio(vec![segment(0, 1, "body\0text")]),
            Err(TranscriptError::InvalidEncoding)
        );
        assert_eq!(
            parse_audio(vec![segment(0, 1, " \n\t ")]),
            Err(TranscriptError::Empty)
        );
    }

    #[test]
    fn structured_audio_enforces_segment_count_per_segment_and_total_limits() {
        let policy = TranscriptPolicy {
            max_pasted_bytes: 100,
            max_file_bytes: 5,
            max_segments: 1,
            max_segment_bytes: 4,
        };
        let segment = |text: &str| StructuredAudioSegment {
            start_offset_ms: 0,
            end_offset_ms: 1,
            original_text: text.to_string(),
        };

        assert_eq!(
            from_structured_audio(vec![segment("a"), segment("b")], "1".to_string(), policy,),
            Err(TranscriptError::SegmentLimitExceeded)
        );
        assert_eq!(
            from_structured_audio(vec![segment("12345")], "1".to_string(), policy),
            Err(TranscriptError::SegmentTooLarge)
        );
        let total_policy = TranscriptPolicy {
            max_segments: 2,
            max_segment_bytes: 4,
            ..policy
        };
        assert_eq!(
            from_structured_audio(
                vec![
                    StructuredAudioSegment {
                        start_offset_ms: 0,
                        end_offset_ms: 1,
                        original_text: "123".to_string(),
                    },
                    StructuredAudioSegment {
                        start_offset_ms: 1,
                        end_offset_ms: 2,
                        original_text: "456".to_string(),
                    },
                ],
                "1".to_string(),
                total_policy,
            ),
            Err(TranscriptError::TooLarge)
        );
    }
}
