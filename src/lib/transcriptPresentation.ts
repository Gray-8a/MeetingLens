import type { TFunction } from "i18next";
import {
  TranscriptApiError,
  type TranscriptErrorCode,
  type TranscriptSegmentResponse,
} from "../api/transcript";
import { currentUiLocale } from "../i18n";

export const FALLBACK_PASTE_LIMIT_BYTES = 2 * 1024 * 1024;
export const TRANSCRIPT_PAGE_SIZE = 100;
export const SPEAKER_GROUP_PAGE_SIZE = 100;

export function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function defaultSummaryMeetingTitle(
  label: string,
  now = new Date(),
): string {
  const dateTime = new Intl.DateTimeFormat(currentUiLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(now);
  return `${label} · ${dateTime}`;
}

export function defaultPrimaryLanguage(): string {
  return "en-AU";
}

export function formatTranscriptOffset(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds]
    .map((part) => part.toString().padStart(2, "0"))
    .join(":");
}

export function segmentSpeaker(
  segment: Pick<TranscriptSegmentResponse, "speakerLabel" | "speakerName">,
  t: TFunction,
): string {
  const name = segment.speakerName?.trim();
  if (name) return name;
  const label = segment.speakerLabel?.trim();
  return label || t("transcriptWorkspace.unknownSpeaker");
}

export function transcriptErrorCode(reason: unknown): TranscriptErrorCode {
  return reason instanceof TranscriptApiError ? reason.code : "internal_error";
}
