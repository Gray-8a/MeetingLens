//! Conservative recognition of copied speaker/time transcript exports.
//! A repeated display-name / localized-time+clock / display-name+time header
//! establishes the format. Ordinary prose continues through the plain parser.

use super::{
    ParsedSegment, TranscriptError, TranscriptPolicy, TranscriptResult,
    is_conservative_speaker_candidate, parse_plain_text, validate_segment_text,
};
use std::collections::{BTreeMap, BTreeSet};

struct Header {
    start: usize,
    end: usize,
    speaker: String,
    milliseconds: u64,
}

pub(super) fn parse_export(
    input: &str,
    policy: TranscriptPolicy,
) -> TranscriptResult<Option<Vec<ParsedSegment>>> {
    let lines: Vec<&str> = input.lines().collect();
    let mut anchors = Vec::new();
    for index in 0..lines.len().saturating_sub(2) {
        let speaker = lines[index].trim();
        if !is_conservative_speaker_candidate(speaker) {
            continue;
        }
        let Some(milliseconds) = display_time(lines[index + 1].trim()) else {
            continue;
        };
        if speaker_time(lines[index + 2].trim(), speaker) != Some(milliseconds) {
            continue;
        }
        anchors.push(Header {
            start: index,
            end: index + 3,
            speaker: speaker.to_string(),
            milliseconds,
        });
        if anchors.len() > policy.max_segments.max(1) {
            return Err(TranscriptError::SegmentLimitExceeded);
        }
    }
    // One coincidental header in ordinary notes must not switch parsers.
    if anchors.len() < 2 {
        return Ok(None);
    }
    let speakers: BTreeSet<&str> = anchors
        .iter()
        .map(|header| header.speaker.as_str())
        .collect();
    let by_start: BTreeMap<usize, &Header> = anchors
        .iter()
        .map(|header| (header.start, header))
        .collect();
    let mut headers = Vec::new();
    let mut index = anchors[0].start;
    while index < lines.len() {
        if headers.len() > policy.max_segments {
            return Err(TranscriptError::SegmentLimitExceeded);
        }
        if let Some(anchor) = by_start.get(&index) {
            headers.push(Header {
                start: index,
                end: anchor.end,
                speaker: anchor.speaker.clone(),
                milliseconds: anchor.milliseconds,
            });
            index = anchor.end;
            continue;
        }
        let line = lines[index].trim();
        let inline = line
            .char_indices()
            .filter(|(_, character)| character.is_whitespace())
            .find_map(|(boundary, _)| {
                let speaker = line[..boundary].trim();
                if !speakers.contains(speaker) {
                    return None;
                }
                elapsed_time(line[boundary..].trim()).map(|time| (speaker, time))
            });
        if let Some((speaker, milliseconds)) = inline {
            headers.push(Header {
                start: index,
                end: index + 1,
                speaker: speaker.to_string(),
                milliseconds,
            });
        }
        index += 1;
    }
    // Remove only the redundant avatar initials immediately preceding a fully
    // verified header. Never discard an arbitrary short utterance or preamble.
    for header in &mut headers {
        if header.end - header.start != 3 {
            continue;
        }
        let mut before = header.start;
        while before > 0 && lines[before - 1].trim().is_empty() {
            before -= 1;
        }
        let initials: String = header
            .speaker
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .collect();
        if before > 0 && initials.len() >= 2 && lines[before - 1].trim() == initials {
            header.start = before - 1;
        }
    }
    let prefix = lines[..headers[0].start].join("\n");
    let mut segments = if prefix.trim().is_empty() {
        Vec::new()
    } else {
        parse_plain_text(&prefix, policy)?
    };
    let mut previous = None;
    for (position, header) in headers.iter().enumerate() {
        if previous.is_some_and(|time| header.milliseconds < time) {
            return Err(TranscriptError::InvalidTimestamp);
        }
        previous = Some(header.milliseconds);
        let end = headers
            .get(position + 1)
            .map_or(lines.len(), |next| next.start);
        if end < header.end {
            return Err(TranscriptError::InvalidInput);
        }
        let body = lines[header.end..end].join("\n");
        let body = body.trim();
        validate_segment_text(body, policy)?;
        if segments.len() >= policy.max_segments {
            return Err(TranscriptError::SegmentLimitExceeded);
        }
        segments.push(ParsedSegment {
            speaker_label: Some(header.speaker.clone()),
            start_offset_ms: Some(header.milliseconds),
            // A following speaker's start is not an observed utterance end.
            end_offset_ms: None,
            original_text: body.to_string(),
        });
    }
    Ok(Some(segments))
}

fn speaker_time(line: &str, speaker: &str) -> Option<u64> {
    let suffix = line.strip_prefix(speaker)?;
    if !suffix.starts_with(char::is_whitespace) {
        return None;
    }
    elapsed_time(suffix.trim())
}

fn display_time(line: &str) -> Option<u64> {
    // The accessibility export duplicates elapsed time, e.g. "1 分钟 7 秒1:07".
    let colon = line.rfind(':')?;
    let (boundary, character) = line[..colon]
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_ascii_digit() && *character != ':')?;
    let clock_start = boundary + character.len_utf8();
    let elapsed = elapsed_time(line[..clock_start].trim())?;
    let fields: Vec<&str> = line[clock_start..].split(':').collect();
    if !(2..=3).contains(&fields.len()) {
        return None;
    }
    let mut seconds = 0_u64;
    for (index, field) in fields.iter().enumerate() {
        if field.is_empty() || !field.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let value: u64 = field.parse().ok()?;
        if index > 0 && (field.len() != 2 || value >= 60) {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }
    (seconds.checked_mul(1000)? == elapsed).then_some(elapsed)
}

fn elapsed_time(value: &str) -> Option<u64> {
    let mut remaining = value.trim();
    let mut seconds = 0_u64;
    let mut matched = false;
    for (unit, multiplier) in [("小时", 3600_u64), ("分钟", 60), ("秒", 1)] {
        if let Some((number, rest)) = remaining.split_once(unit) {
            let number = number.trim();
            if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let number: u64 = number.parse().ok()?;
            if multiplier == 1 && number >= 60 {
                return None;
            }
            seconds = seconds.checked_add(number.checked_mul(multiplier)?)?;
            remaining = rest.trim();
            matched = true;
        }
    }
    if !matched || !remaining.is_empty() {
        return None;
    }
    seconds
        .checked_mul(1000)
        .filter(|value| *value <= i64::MAX as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::transcript_model::TranscriptSourceType;
    use crate::data::transcript_parser::parse;

    const EXPORT: &str = "Synthetic meeting export\n\nAS\n\nAlice Sample\n0 分钟 3 秒0:03\nAlice Sample 0 分钟 3 秒\nSynthetic proposal.\nAlice Sample 0 分钟 9 秒\nSynthetic revision.\nBE\n\nBob Example\n1 分钟1:00\nBob Example 1 分钟\nSynthetic decision.\nBob Example 1 分钟 7 秒\nSynthetic task due Friday.";

    #[test]
    fn copied_export_splits_turns_without_blank_lines_and_retains_preamble() {
        let parsed = parse(
            TranscriptSourceType::ImportedTxt,
            EXPORT.as_bytes(),
            TranscriptPolicy::default(),
        )
        .expect("synthetic export should parse");
        assert_eq!(parsed.parser_name, "speaker_time_text");
        assert_eq!(parsed.segments.len(), 5);
        assert_eq!(parsed.segments[0].original_text, "Synthetic meeting export");
        assert_eq!(parsed.segments[0].speaker_label, None);
        for (segment, (speaker, offset, body)) in parsed.segments[1..].iter().zip([
            ("Alice Sample", 3000, "Synthetic proposal."),
            ("Alice Sample", 9000, "Synthetic revision."),
            ("Bob Example", 60000, "Synthetic decision."),
            ("Bob Example", 67000, "Synthetic task due Friday."),
        ]) {
            assert_eq!(segment.speaker_label.as_deref(), Some(speaker));
            assert_eq!(segment.start_offset_ms, Some(offset));
            assert_eq!(segment.end_offset_ms, None);
            assert_eq!(segment.original_text, body);
        }
    }

    #[test]
    fn pasted_and_imported_exports_match_and_newlines_keep_original_hash_contract() {
        let txt = parse(
            TranscriptSourceType::ImportedTxt,
            EXPORT.as_bytes(),
            TranscriptPolicy::default(),
        )
        .unwrap();
        let pasted = parse(
            TranscriptSourceType::PastedText,
            EXPORT.as_bytes(),
            TranscriptPolicy::default(),
        )
        .unwrap();
        let crlf = format!("\u{feff}{}", EXPORT.replace('\n', "\r\n"));
        let windows = parse(
            TranscriptSourceType::ImportedTxt,
            crlf.as_bytes(),
            TranscriptPolicy::default(),
        )
        .unwrap();
        assert_eq!(txt, pasted);
        assert_eq!(txt, windows);
        assert_eq!(txt.content_sha256, super::super::hash_plain(EXPORT));
    }

    #[test]
    fn ordinary_notes_and_single_coincidental_header_use_plain_parser() {
        for input in [
            "Alice: Meet at 10:30\n\nBob: Friday",
            "Alice Sample\n0 分钟 3 秒0:03\nAlice Sample 0 分钟 3 秒\nSynthetic quote.",
        ] {
            let parsed = parse(
                TranscriptSourceType::ImportedTxt,
                input.as_bytes(),
                TranscriptPolicy::default(),
            )
            .unwrap();
            assert_eq!(parsed.parser_name, "plain_text");
            assert!(
                parsed
                    .segments
                    .iter()
                    .all(|segment| segment.start_offset_ms.is_none())
            );
        }
    }

    #[test]
    fn arbitrary_short_text_blank_paragraphs_and_unknown_speakers_are_preserved() {
        let input = EXPORT.replace(
            "Synthetic revision.",
            "Synthetic revision.\n\nOK\nUnknown Person 0 分钟 20 秒\nSynthetic continuation.",
        );
        let parsed = parse_export(&input, TranscriptPolicy::default())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.len(), 5);
        assert!(
            parsed[2]
                .original_text
                .ends_with("OK\nUnknown Person 0 分钟 20 秒\nSynthetic continuation.")
        );
    }

    #[test]
    fn exported_turn_limits_are_enforced_after_splitting() {
        let policy = TranscriptPolicy {
            max_segments: 4,
            ..TranscriptPolicy::default()
        };
        assert!(matches!(
            parse_export(EXPORT, policy),
            Err(TranscriptError::SegmentLimitExceeded)
        ));
        let policy = TranscriptPolicy {
            max_segment_bytes: 24,
            ..TranscriptPolicy::default()
        };
        assert!(parse_export(EXPORT, policy).is_err());
    }

    #[test]
    fn decreasing_times_and_empty_turns_fail_closed_without_partial_import() {
        let backwards = EXPORT.replace("Bob Example 1 分钟 7 秒", "Bob Example 0 分钟 1 秒");
        assert!(matches!(
            parse_export(&backwards, TranscriptPolicy::default()),
            Err(TranscriptError::InvalidTimestamp)
        ));
        let empty = EXPORT.replace("Synthetic proposal.\n", "");
        assert!(parse_export(&empty, TranscriptPolicy::default()).is_err());
    }

    #[test]
    fn simultaneous_speakers_are_allowed_without_inventing_end_times() {
        let input = EXPORT
            .replace("1 分钟1:00", "0 分钟 9 秒0:09")
            .replace("Bob Example 1 分钟\n", "Bob Example 0 分钟 9 秒\n");
        let parsed = parse_export(&input, TranscriptPolicy::default())
            .unwrap()
            .unwrap();
        assert_eq!(parsed[2].start_offset_ms, parsed[3].start_offset_ms);
        assert!(parsed.iter().all(|segment| segment.end_offset_ms.is_none()));
    }

    #[test]
    fn localized_times_require_exact_units_valid_clock_and_checked_arithmetic() {
        for (input, expected) in [
            ("0 秒0:00", 0),
            ("1 分钟1:00", 60000),
            ("1 小时 2 分钟 3 秒1:02:03", 3723000),
            ("72 分钟 3 秒72:03", 4323000),
        ] {
            assert_eq!(display_time(input), Some(expected));
        }
        for input in [
            "1 分钟1:01",
            "1 分钟1:0",
            "1 分钟1:60",
            "1 秒x0:01",
            "18446744073709551615 小时0:00",
            "1 分钟 and text1:00",
            "-1 秒0:01",
        ] {
            assert_eq!(display_time(input), None);
        }
    }

    #[test]
    fn long_unbroken_export_is_split_before_per_segment_limit_is_applied() {
        let mut input =
            "Alice Sample\n0 秒0:00\nAlice Sample 0 秒\nSynthetic opening.\n".to_string();
        for second in 1..600 {
            input.push_str(&format!(
                "Alice Sample {} 分钟 {} 秒\nSynthetic bounded turn number {second}.\n",
                second / 60,
                second % 60
            ));
        }
        input.push_str("Bob Example\n10 分钟10:00\nBob Example 10 分钟\nSynthetic closing.");
        let policy = TranscriptPolicy {
            max_segment_bytes: 128,
            ..TranscriptPolicy::default()
        };
        let segments = parse_export(&input, policy).unwrap().unwrap();
        assert_eq!(segments.len(), 601);
        assert!(
            segments
                .iter()
                .all(|segment| segment.original_text.len() <= 128)
        );
        assert_eq!(segments.last().unwrap().start_offset_ms, Some(600000));
    }
}
