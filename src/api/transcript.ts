import { invoke } from "@tauri-apps/api/core";
import {
  parseMeetingResponse,
  type InvokeAdapter,
  type InvokeArguments,
  type MeetingResponse,
} from "./meeting";

export const transcriptCommandNames = {
  createMeetingFromPaste: "create_meeting_from_pasted_transcript",
  selectTranscriptAndCreateMeeting: "select_transcript_and_create_meeting",
  addPasteToMeeting: "add_pasted_transcript_to_meeting",
  selectAndAddToMeeting: "select_and_add_transcript_to_meeting",
  listSources: "list_transcript_sources",
  getSource: "get_transcript_source",
  listSegments: "list_transcript_segments",
  updateSegment: "update_transcript_segment",
  deleteSource: "delete_transcript_source",
  getImportPolicy: "get_transcript_import_policy",
} as const;

export type TranscriptSourceType =
  | "pasted_text"
  | "imported_txt"
  | "imported_srt"
  | "imported_vtt"
  | "live_asr"
  | "platform_export";

export type TranscriptExtension = "txt" | "srt" | "vtt";

interface TranscriptImportMetadata {
  displayName: string;
  language?: string | null;
  baseStartedAt?: string | null;
}

interface SelectedTranscriptMetadata {
  language?: string | null;
  baseStartedAt?: string | null;
}

export interface CreateMeetingFromPasteRequest
  extends TranscriptImportMetadata {
  title: string;
  primaryLanguage: string;
  pastedText: string;
}

export interface SelectTranscriptAndCreateMeetingRequest
  extends SelectedTranscriptMetadata {
  title: string;
  primaryLanguage: string;
}

export interface AddPasteToMeetingRequest extends TranscriptImportMetadata {
  meetingId: string;
  pastedText: string;
}

export interface SelectAndAddToMeetingRequest
  extends SelectedTranscriptMetadata {
  meetingId: string;
}

export interface UpdateTranscriptSegmentRequest {
  meetingId: string;
  segmentId: string;
  speakerName?: string | null;
  originalText?: string;
  expectedRevision: number;
}

export interface TranscriptSourceResponse {
  id: string;
  meetingId: string;
  sourceType: TranscriptSourceType;
  displayName: string;
  originalFilename: string | null;
  language: string | null;
  baseStartedAt: string | null;
  segmentCount: number;
  importedAt: string;
  createdAt: string;
  updatedAt: string;
}

export interface TranscriptSegmentResponse {
  id: string;
  sourceId: string;
  meetingId: string;
  ordinal: number;
  speakerLabel: string | null;
  speakerName: string | null;
  startOffsetMs: number | null;
  endOffsetMs: number | null;
  originalText: string;
  translatedText: string | null;
  language: string | null;
  confidence: number | null;
  isFinal: boolean;
  revision: number;
  correctedAt: string | null;
}

export interface ListTranscriptSourcesResponse {
  sources: TranscriptSourceResponse[];
}

export interface ListTranscriptSegmentsResponse {
  segments: TranscriptSegmentResponse[];
}

export interface CancelledTranscriptImportResult {
  cancelled: true;
  duplicate: false;
  meeting: null;
  source: null;
  segmentCount: 0;
}

export interface CompletedTranscriptImportResult {
  cancelled: false;
  duplicate: boolean;
  meeting: MeetingResponse;
  source: TranscriptSourceResponse;
  segmentCount: number;
}

export type TranscriptImportResult =
  | CancelledTranscriptImportResult
  | CompletedTranscriptImportResult;

export interface DeleteTranscriptSourceResponse {
  sourceId: string;
  deleted: boolean;
}

export interface TranscriptImportPolicyResponse {
  allowedExtensions: TranscriptExtension[];
  maxPasteBytes: number;
  maxFileBytes: number;
  maxSegmentsPerSource: number;
  maxSegmentBytes: number;
}

export type TranscriptErrorCode =
  | "transcript_already_imported"
  | "transcript_empty"
  | "transcript_input_insufficient"
  | "transcript_invalid_encoding"
  | "transcript_invalid_format"
  | "transcript_invalid_input"
  | "transcript_invalid_state"
  | "transcript_invalid_timestamp"
  | "transcript_persistence_failed"
  | "transcript_revision_conflict"
  | "transcript_segment_limit_exceeded"
  | "transcript_segment_not_found"
  | "transcript_segment_too_large"
  | "transcript_source_in_use"
  | "transcript_source_not_found"
  | "transcript_too_large"
  | "transcript_picker_unavailable"
  | "meeting_not_found"
  | "meeting_invalid_state"
  | "internal_error";

const safeMessages: Record<TranscriptErrorCode, string> = {
  transcript_already_imported:
    "This transcript is already attached to the meeting.",
  transcript_empty: "The transcript is empty.",
  transcript_input_insufficient:
    "The transcript does not contain usable final segments.",
  transcript_invalid_encoding:
    "The transcript must use supported UTF-8 text encoding.",
  transcript_invalid_format: "The transcript format is invalid.",
  transcript_invalid_input: "The transcript details are invalid.",
  transcript_invalid_state:
    "The transcript cannot be changed in its current state.",
  transcript_invalid_timestamp: "The transcript contains an invalid timestamp.",
  transcript_persistence_failed: "The transcript could not be saved.",
  transcript_revision_conflict:
    "The transcript segment changed before this update.",
  transcript_segment_limit_exceeded:
    "The transcript contains too many segments.",
  transcript_segment_not_found: "The transcript segment was not found.",
  transcript_segment_too_large: "A transcript segment is too large.",
  transcript_source_in_use: "The transcript source is currently in use.",
  transcript_source_not_found: "The transcript source was not found.",
  transcript_too_large: "The transcript is too large.",
  transcript_picker_unavailable: "The transcript picker is unavailable.",
  meeting_not_found: "The meeting was not found.",
  meeting_invalid_state: "The meeting cannot be changed in its current state.",
  internal_error: "An internal error occurred.",
};

const errorCodes = new Set<TranscriptErrorCode>(
  Object.keys(safeMessages) as TranscriptErrorCode[],
);
const sourceTypes = new Set<TranscriptSourceType>([
  "pasted_text",
  "imported_txt",
  "imported_srt",
  "imported_vtt",
  "live_asr",
  "platform_export",
]);
const transcriptExtensions = new Set<TranscriptExtension>([
  "txt",
  "srt",
  "vtt",
]);
const frozenPolicyLimits = {
  maxPasteBytes: 2 * 1024 * 1024,
  maxFileBytes: 10 * 1024 * 1024,
  maxSegmentsPerSource: 20_000,
  maxSegmentBytes: 64 * 1024,
} as const;
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const utcIsoPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

export class TranscriptApiError extends Error {
  readonly code: TranscriptErrorCode;
  readonly retryable: boolean;

  constructor(code: TranscriptErrorCode, retryable: boolean) {
    super(safeMessages[code]);
    this.name = "TranscriptApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

const tauriInvokeAdapter: InvokeAdapter = {
  invoke: (command, args) => invoke<unknown>(command, args),
};

export interface TranscriptApi {
  createMeetingFromPaste(
    request: CreateMeetingFromPasteRequest,
  ): Promise<TranscriptImportResult>;
  selectTranscriptAndCreateMeeting(
    request: SelectTranscriptAndCreateMeetingRequest,
  ): Promise<TranscriptImportResult>;
  addPasteToMeeting(
    request: AddPasteToMeetingRequest,
  ): Promise<TranscriptImportResult>;
  selectAndAddToMeeting(
    request: SelectAndAddToMeetingRequest,
  ): Promise<TranscriptImportResult>;
  listSources(meetingId: string): Promise<ListTranscriptSourcesResponse>;
  getSource(
    meetingId: string,
    sourceId: string,
  ): Promise<TranscriptSourceResponse>;
  listSegments(
    meetingId: string,
    sourceId: string,
  ): Promise<ListTranscriptSegmentsResponse>;
  updateSegment(
    request: UpdateTranscriptSegmentRequest,
  ): Promise<TranscriptSegmentResponse>;
  deleteSource(
    meetingId: string,
    sourceId: string,
  ): Promise<DeleteTranscriptSourceResponse>;
  getImportPolicy(): Promise<TranscriptImportPolicyResponse>;
}

export function createTranscriptApi(
  adapter: InvokeAdapter = tauriInvokeAdapter,
): TranscriptApi {
  return {
    createMeetingFromPaste: (request) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.createMeetingFromPaste,
        {
          request: withImportMetadata(
            {
              title: request.title,
              primaryLanguage: request.primaryLanguage,
              pastedText: request.pastedText,
            },
            request,
          ),
        },
        parseImportResult,
      ),
    selectTranscriptAndCreateMeeting: (request) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.selectTranscriptAndCreateMeeting,
        {
          request: withSelectedMetadata(
            {
              title: request.title,
              primaryLanguage: request.primaryLanguage,
            },
            request,
          ),
        },
        parseImportResult,
      ),
    addPasteToMeeting: (request) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.addPasteToMeeting,
        {
          request: withImportMetadata(
            {
              meetingId: request.meetingId,
              pastedText: request.pastedText,
            },
            request,
          ),
        },
        (value) => parseImportResult(value, request.meetingId),
      ),
    selectAndAddToMeeting: (request) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.selectAndAddToMeeting,
        {
          request: withSelectedMetadata(
            { meetingId: request.meetingId },
            request,
          ),
        },
        (value) => parseImportResult(value, request.meetingId),
      ),
    listSources: (meetingId) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.listSources,
        { request: { meetingId } },
        (value) => parseSourceList(value, meetingId),
      ),
    getSource: (meetingId, sourceId) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.getSource,
        { request: { meetingId, sourceId } },
        (value) => {
          const source = parseSource(value);
          if (source.meetingId !== meetingId || source.id !== sourceId) {
            throw internalError();
          }
          return source;
        },
      ),
    listSegments: (meetingId, sourceId) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.listSegments,
        { request: { meetingId, sourceId } },
        (value) => parseSegmentList(value, meetingId, sourceId),
      ),
    updateSegment: (request) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.updateSegment,
        {
          request: {
            meetingId: request.meetingId,
            segmentId: request.segmentId,
            expectedRevision: request.expectedRevision,
            ...(request.speakerName !== undefined
              ? { speakerName: request.speakerName }
              : {}),
            ...(request.originalText !== undefined
              ? { originalText: request.originalText }
              : {}),
          },
        },
        (value) => {
          const segment = parseSegment(value);
          if (
            segment.meetingId !== request.meetingId ||
            segment.id !== request.segmentId
          ) {
            throw internalError();
          }
          return segment;
        },
      ),
    deleteSource: (meetingId, sourceId) =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.deleteSource,
        { request: { meetingId, sourceId } },
        (value) => {
          const result = parseDelete(value);
          if (result.sourceId !== sourceId) throw internalError();
          return result;
        },
      ),
    getImportPolicy: () =>
      invokeAndParse(
        adapter,
        transcriptCommandNames.getImportPolicy,
        undefined,
        parsePolicy,
      ),
  };
}

export const transcriptApi = createTranscriptApi();

async function invokeAndParse<T>(
  adapter: InvokeAdapter,
  command: string,
  args: InvokeArguments | undefined,
  parse: (value: unknown) => T,
): Promise<T> {
  try {
    return parse(await adapter.invoke(command, args));
  } catch (error: unknown) {
    if (error instanceof TranscriptApiError) throw error;
    throw parseTranscriptError(error);
  }
}

function withImportMetadata(
  base: Record<string, unknown>,
  metadata: TranscriptImportMetadata,
): Record<string, unknown> {
  return {
    ...base,
    displayName: metadata.displayName,
    ...withSelectedMetadata({}, metadata),
  };
}

function withSelectedMetadata(
  base: Record<string, unknown>,
  metadata: SelectedTranscriptMetadata,
): Record<string, unknown> {
  return {
    ...base,
    ...(metadata.language !== undefined ? { language: metadata.language } : {}),
    ...(metadata.baseStartedAt !== undefined
      ? { baseStartedAt: metadata.baseStartedAt }
      : {}),
  };
}

function parseImportResult(
  value: unknown,
  expectedMeetingId?: string,
): TranscriptImportResult {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "cancelled",
      "duplicate",
      "meeting",
      "source",
      "segmentCount",
    ]) ||
    typeof value.cancelled !== "boolean" ||
    typeof value.duplicate !== "boolean" ||
    !isNonNegativeSafeInteger(value.segmentCount)
  ) {
    throw internalError();
  }

  if (value.cancelled) {
    if (
      value.duplicate ||
      value.meeting !== null ||
      value.source !== null ||
      value.segmentCount !== 0
    ) {
      throw internalError();
    }
    return {
      cancelled: true,
      duplicate: false,
      meeting: null,
      source: null,
      segmentCount: 0,
    };
  }

  if (
    value.meeting === null ||
    value.source === null ||
    value.segmentCount === 0
  ) {
    throw internalError();
  }
  const meeting = parseMeetingResponse(value.meeting);
  const source = parseSource(value.source);
  if (
    source.meetingId !== meeting.id ||
    source.segmentCount !== value.segmentCount ||
    (expectedMeetingId !== undefined && meeting.id !== expectedMeetingId)
  ) {
    throw internalError();
  }
  return {
    cancelled: false,
    duplicate: value.duplicate,
    meeting,
    source,
    segmentCount: value.segmentCount,
  };
}

function parseSource(value: unknown): TranscriptSourceResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "id",
      "meetingId",
      "sourceType",
      "displayName",
      "originalFilename",
      "language",
      "baseStartedAt",
      "segmentCount",
      "importedAt",
      "createdAt",
      "updatedAt",
    ]) ||
    !isUuid(value.id) ||
    !isUuid(value.meetingId) ||
    typeof value.sourceType !== "string" ||
    !sourceTypes.has(value.sourceType as TranscriptSourceType) ||
    !isSafeDisplayString(value.displayName) ||
    !isNullableSafeFilename(value.originalFilename) ||
    !isNullableString(value.language) ||
    !isNullableUtcIso(value.baseStartedAt) ||
    !isNonNegativeSafeInteger(value.segmentCount) ||
    !isUtcIso(value.importedAt) ||
    !isUtcIso(value.createdAt) ||
    !isUtcIso(value.updatedAt)
  ) {
    throw internalError();
  }
  return {
    id: value.id,
    meetingId: value.meetingId,
    sourceType: value.sourceType as TranscriptSourceType,
    displayName: value.displayName,
    originalFilename: value.originalFilename,
    language: value.language,
    baseStartedAt: value.baseStartedAt,
    segmentCount: value.segmentCount,
    importedAt: value.importedAt,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  };
}

function parseSourceList(
  value: unknown,
  expectedMeetingId: string,
): ListTranscriptSourcesResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["sources"]) ||
    !Array.isArray(value.sources)
  ) {
    throw internalError();
  }
  const sources = value.sources.map(parseSource);
  if (sources.some((source) => source.meetingId !== expectedMeetingId)) {
    throw internalError();
  }
  return { sources };
}

function parseSegment(value: unknown): TranscriptSegmentResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "id",
      "sourceId",
      "meetingId",
      "ordinal",
      "speakerLabel",
      "speakerName",
      "startOffsetMs",
      "endOffsetMs",
      "originalText",
      "translatedText",
      "language",
      "confidence",
      "isFinal",
      "revision",
      "correctedAt",
    ]) ||
    !isUuid(value.id) ||
    !isUuid(value.sourceId) ||
    !isUuid(value.meetingId) ||
    !isNonNegativeSafeInteger(value.ordinal) ||
    !isNullableString(value.speakerLabel) ||
    !isNullableString(value.speakerName) ||
    !isNullableNonNegativeSafeInteger(value.startOffsetMs) ||
    !isNullableNonNegativeSafeInteger(value.endOffsetMs) ||
    (value.startOffsetMs !== null &&
      value.endOffsetMs !== null &&
      value.endOffsetMs < value.startOffsetMs) ||
    typeof value.originalText !== "string" ||
    value.originalText.length === 0 ||
    !isNullableString(value.translatedText) ||
    !isNullableString(value.language) ||
    !isNullableConfidence(value.confidence) ||
    typeof value.isFinal !== "boolean" ||
    !isPositiveSafeInteger(value.revision) ||
    !isNullableUtcIso(value.correctedAt)
  ) {
    throw internalError();
  }
  return {
    id: value.id,
    sourceId: value.sourceId,
    meetingId: value.meetingId,
    ordinal: value.ordinal,
    speakerLabel: value.speakerLabel,
    speakerName: value.speakerName,
    startOffsetMs: value.startOffsetMs,
    endOffsetMs: value.endOffsetMs,
    originalText: value.originalText,
    translatedText: value.translatedText,
    language: value.language,
    confidence: value.confidence,
    isFinal: value.isFinal,
    revision: value.revision,
    correctedAt: value.correctedAt,
  };
}

function parseSegmentList(
  value: unknown,
  expectedMeetingId: string,
  expectedSourceId: string,
): ListTranscriptSegmentsResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["segments"]) ||
    !Array.isArray(value.segments)
  ) {
    throw internalError();
  }
  const segments = value.segments.map(parseSegment);
  if (
    segments.some(
      (segment) =>
        segment.meetingId !== expectedMeetingId ||
        segment.sourceId !== expectedSourceId,
    )
  ) {
    throw internalError();
  }
  return { segments };
}

function parseDelete(value: unknown): DeleteTranscriptSourceResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["sourceId", "deleted"]) ||
    !isUuid(value.sourceId) ||
    typeof value.deleted !== "boolean"
  ) {
    throw internalError();
  }
  return { sourceId: value.sourceId, deleted: value.deleted };
}

function parsePolicy(value: unknown): TranscriptImportPolicyResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "allowedExtensions",
      "maxPasteBytes",
      "maxFileBytes",
      "maxSegmentsPerSource",
      "maxSegmentBytes",
    ]) ||
    !Array.isArray(value.allowedExtensions) ||
    value.allowedExtensions.length !== transcriptExtensions.size ||
    !value.allowedExtensions.every(
      (extension) =>
        typeof extension === "string" &&
        transcriptExtensions.has(extension as TranscriptExtension),
    ) ||
    new Set(value.allowedExtensions).size !== transcriptExtensions.size ||
    !isPositiveSafeInteger(value.maxPasteBytes) ||
    value.maxPasteBytes !== frozenPolicyLimits.maxPasteBytes ||
    !isPositiveSafeInteger(value.maxFileBytes) ||
    value.maxFileBytes !== frozenPolicyLimits.maxFileBytes ||
    !isPositiveSafeInteger(value.maxSegmentsPerSource) ||
    value.maxSegmentsPerSource !== frozenPolicyLimits.maxSegmentsPerSource ||
    !isPositiveSafeInteger(value.maxSegmentBytes) ||
    value.maxSegmentBytes !== frozenPolicyLimits.maxSegmentBytes
  ) {
    throw internalError();
  }
  return {
    allowedExtensions: [...value.allowedExtensions] as TranscriptExtension[],
    maxPasteBytes: value.maxPasteBytes,
    maxFileBytes: value.maxFileBytes,
    maxSegmentsPerSource: value.maxSegmentsPerSource,
    maxSegmentBytes: value.maxSegmentBytes,
  };
}

function parseTranscriptError(value: unknown): TranscriptApiError {
  if (
    isRecord(value) &&
    hasOnlyKeys(value, ["code", "message", "retryable"]) &&
    typeof value.code === "string" &&
    errorCodes.has(value.code as TranscriptErrorCode) &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  ) {
    return new TranscriptApiError(
      value.code as TranscriptErrorCode,
      value.retryable,
    );
  }
  return internalError();
}

function internalError(): TranscriptApiError {
  return new TranscriptApiError("internal_error", false);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const allowed = new Set(keys);
  return Object.keys(value).every((key) => allowed.has(key));
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isSafeDisplayString(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    !Array.from(value).some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
    })
  );
}

function isNullableSafeFilename(value: unknown): value is string | null {
  return (
    value === null ||
    (isSafeDisplayString(value) &&
      value !== "." &&
      value !== ".." &&
      !/[\\/:*?"<>|]/u.test(value))
  );
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return isNonNegativeSafeInteger(value) && value > 0;
}

function isNullableNonNegativeSafeInteger(
  value: unknown,
): value is number | null {
  return value === null || isNonNegativeSafeInteger(value);
}

function isNullableConfidence(value: unknown): value is number | null {
  return (
    value === null ||
    (typeof value === "number" &&
      Number.isFinite(value) &&
      value >= 0 &&
      value <= 1)
  );
}

function isUtcIso(value: unknown): value is string {
  return (
    typeof value === "string" &&
    utcIsoPattern.test(value) &&
    Number.isFinite(Date.parse(value))
  );
}

function isNullableUtcIso(value: unknown): value is string | null {
  return value === null || isUtcIso(value);
}
