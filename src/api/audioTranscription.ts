import { Channel, invoke } from "@tauri-apps/api/core";
import {
  parseMeetingResponse,
  type InvokeAdapter,
  type InvokeArguments,
  type MeetingResponse,
} from "./meeting";
import {
  parseTranscriptSourceResponse,
  type TranscriptSourceResponse,
} from "./transcript";

export const audioTranscriptionCommandNames = {
  getPolicy: "get_audio_transcription_policy",
  getStatus: "get_audio_transcription_status",
  selectAndCreateMeeting: "select_audio_and_create_meeting",
  selectAndAddToMeeting: "select_and_add_audio_to_meeting",
  cancel: "cancel_audio_transcription",
} as const;

export type AudioTranscriptionExtension = "wav" | "mp3" | "m4a" | "mp4";

export type AudioTranscriptionPhase =
  | "selected"
  | "validating_input"
  | "copying"
  | "probing"
  | "model_loading"
  | "transcribing"
  | "validating_output"
  | "committing"
  | "completed"
  | "failed"
  | "cancelled";

export type AudioTranscriptionStatus =
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export type AudioTranscriptionErrorCode =
  | "audio_picker_unavailable"
  | "audio_invalid_input"
  | "audio_unsupported_type"
  | "audio_type_mismatch"
  | "audio_empty"
  | "audio_too_large"
  | "audio_too_long"
  | "audio_stream_missing"
  | "audio_model_missing"
  | "audio_model_incompatible"
  | "audio_worker_unavailable"
  | "audio_worker_start_failed"
  | "audio_worker_crashed"
  | "audio_protocol_error"
  | "audio_output_too_large"
  | "audio_integrity_failed"
  | "audio_timeout"
  | "audio_cancelled"
  | "audio_transcription_busy"
  | "audio_persistence_failed"
  | "meeting_not_found"
  | "meeting_invalid_state"
  | "internal_error";

export interface AudioTranscriptionPolicyResponse {
  supportedExtensions: AudioTranscriptionExtension[];
  maxFileBytes: number;
  maxDurationMs: number;
  maxSegments: number;
  device: "cpu";
  computeType: "int8";
  model: string;
  modelRevision: string;
  workerAvailable: boolean;
  modelAvailable: boolean;
  busy: boolean;
}

export interface AudioTranscriptionProgress {
  jobId: string;
  phase: AudioTranscriptionPhase;
  status: AudioTranscriptionStatus;
  bytesProcessed: number;
  totalBytes: number | null;
  processedMs: number | null;
  durationMs: number | null;
  errorCode: AudioTranscriptionErrorCode | null;
}

export interface SelectAudioAndCreateMeetingRequest {
  title: string;
  primaryLanguage: string;
}

export interface SelectAndAddAudioToMeetingRequest {
  meetingId: string;
}

export interface CancelledAudioTranscriptionResult {
  cancelled: true;
  jobId: string | null;
  meeting: null;
  source: null;
  segmentCount: 0;
}

export interface CompletedAudioTranscriptionResult {
  cancelled: false;
  jobId: string;
  meeting: MeetingResponse;
  source: TranscriptSourceResponse;
  segmentCount: number;
}

export type AudioTranscriptionResult =
  | CancelledAudioTranscriptionResult
  | CompletedAudioTranscriptionResult;

const safeMessages: Record<AudioTranscriptionErrorCode, string> = {
  audio_picker_unavailable: "The audio file picker is unavailable.",
  audio_invalid_input: "The audio transcription request is invalid.",
  audio_unsupported_type: "This audio file type is not supported.",
  audio_type_mismatch: "This file does not match its filename type.",
  audio_empty: "The selected audio file is empty.",
  audio_too_large: "The selected audio file is too large.",
  audio_too_long: "The selected audio file is too long.",
  audio_stream_missing: "The selected file has no supported audio stream.",
  audio_model_missing: "The local transcription model is not installed.",
  audio_model_incompatible: "The local transcription model is incompatible.",
  audio_worker_unavailable: "The local transcription worker is unavailable.",
  audio_worker_start_failed: "The local transcription worker could not start.",
  audio_worker_crashed: "The local transcription worker stopped unexpectedly.",
  audio_protocol_error: "The local transcription response was invalid.",
  audio_output_too_large: "The local transcription result is too large.",
  audio_integrity_failed: "The selected audio file could not be verified.",
  audio_timeout: "Local audio transcription timed out.",
  audio_cancelled: "Local audio transcription was cancelled.",
  audio_transcription_busy: "Another audio transcription is already running.",
  audio_persistence_failed: "The audio transcription could not be saved.",
  meeting_not_found: "The meeting was not found.",
  meeting_invalid_state: "Audio cannot be added in the current meeting state.",
  internal_error: "An internal error occurred.",
};

const errorCodes = new Set<AudioTranscriptionErrorCode>(
  Object.keys(safeMessages) as AudioTranscriptionErrorCode[],
);
const supportedExtensions = [
  "wav",
  "mp3",
  "m4a",
  "mp4",
] as const satisfies readonly AudioTranscriptionExtension[];
const phases = new Set<AudioTranscriptionPhase>([
  "selected",
  "validating_input",
  "copying",
  "probing",
  "model_loading",
  "transcribing",
  "validating_output",
  "committing",
  "completed",
  "failed",
  "cancelled",
]);
const statuses = new Set<AudioTranscriptionStatus>([
  "running",
  "completed",
  "failed",
  "cancelled",
]);
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export class AudioTranscriptionApiError extends Error {
  readonly code: AudioTranscriptionErrorCode;
  readonly retryable: boolean;

  constructor(code: AudioTranscriptionErrorCode, retryable: boolean) {
    super(safeMessages[code]);
    this.name = "AudioTranscriptionApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

export interface AudioProgressChannelHandle {
  readonly argument: unknown;
  dispose(): void;
}

export interface AudioProgressChannelAdapter {
  create(handler: (value: unknown) => void): AudioProgressChannelHandle;
}

const tauriInvokeAdapter: InvokeAdapter = {
  invoke: (command, args) => invoke<unknown>(command, args),
};

const tauriProgressChannelAdapter: AudioProgressChannelAdapter = {
  create(handler) {
    let active = true;
    const channel = new Channel<unknown>((value) => {
      if (active) handler(value);
    });
    return {
      argument: channel,
      dispose() {
        active = false;
        channel.onmessage = () => undefined;
      },
    };
  },
};

export interface AudioTranscriptionApi {
  getPolicy(): Promise<AudioTranscriptionPolicyResponse>;
  getStatus(jobId: string): Promise<AudioTranscriptionProgress>;
  selectAudioAndCreateMeeting(
    request: SelectAudioAndCreateMeetingRequest,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
  ): Promise<AudioTranscriptionResult>;
  selectAndAddAudioToMeeting(
    request: SelectAndAddAudioToMeetingRequest,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
  ): Promise<AudioTranscriptionResult>;
  cancel(jobId: string): Promise<AudioTranscriptionProgress>;
}

export function createAudioTranscriptionApi(
  invokeAdapter: InvokeAdapter = tauriInvokeAdapter,
  channelAdapter: AudioProgressChannelAdapter = tauriProgressChannelAdapter,
): AudioTranscriptionApi {
  const select = async (
    command: string,
    request: Record<string, unknown>,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
    expectedMeetingId?: string,
  ) => {
    const progressChannel = channelAdapter.create((value) => {
      progressHandler(parseProgress(value));
    });
    try {
      return await invokeAndParse(
        invokeAdapter,
        command,
        { request, onProgress: progressChannel.argument },
        (value) => parseResult(value, expectedMeetingId),
      );
    } finally {
      progressChannel.dispose();
    }
  };

  return {
    getPolicy: () =>
      invokeAndParse(
        invokeAdapter,
        audioTranscriptionCommandNames.getPolicy,
        undefined,
        parsePolicy,
      ),
    getStatus: async (jobId) => {
      requireJobId(jobId);
      return await invokeAndParse(
        invokeAdapter,
        audioTranscriptionCommandNames.getStatus,
        { request: { jobId } },
        parseProgress,
      );
    },
    selectAudioAndCreateMeeting: (request, progressHandler) =>
      select(
        audioTranscriptionCommandNames.selectAndCreateMeeting,
        { title: request.title, primaryLanguage: request.primaryLanguage },
        progressHandler,
      ),
    selectAndAddAudioToMeeting: (request, progressHandler) =>
      select(
        audioTranscriptionCommandNames.selectAndAddToMeeting,
        { meetingId: request.meetingId },
        progressHandler,
        request.meetingId,
      ),
    cancel: async (jobId) => {
      requireJobId(jobId);
      return await invokeAndParse(
        invokeAdapter,
        audioTranscriptionCommandNames.cancel,
        { request: { jobId } },
        parseProgress,
      );
    },
  };
}

export const audioTranscriptionApi = createAudioTranscriptionApi();

async function invokeAndParse<T>(
  adapter: InvokeAdapter,
  command: string,
  args: InvokeArguments | undefined,
  parse: (value: unknown) => T,
): Promise<T> {
  try {
    return parse(await adapter.invoke(command, args));
  } catch (error: unknown) {
    if (error instanceof AudioTranscriptionApiError) throw error;
    throw parseError(error);
  }
}

function parsePolicy(value: unknown): AudioTranscriptionPolicyResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "supportedExtensions",
      "maxFileBytes",
      "maxDurationMs",
      "maxSegments",
      "device",
      "computeType",
      "model",
      "modelRevision",
      "workerAvailable",
      "modelAvailable",
      "busy",
    ]) ||
    !Array.isArray(value.supportedExtensions) ||
    value.supportedExtensions.length !== supportedExtensions.length ||
    !value.supportedExtensions.every(
      (extension, index) => extension === supportedExtensions[index],
    ) ||
    !isPositiveSafeInteger(value.maxFileBytes) ||
    !isPositiveSafeInteger(value.maxDurationMs) ||
    !isPositiveSafeInteger(value.maxSegments) ||
    value.maxSegments > 20_000 ||
    value.device !== "cpu" ||
    value.computeType !== "int8" ||
    !isSafeLabel(value.model) ||
    !isSafeLabel(value.modelRevision) ||
    typeof value.workerAvailable !== "boolean" ||
    typeof value.modelAvailable !== "boolean" ||
    typeof value.busy !== "boolean"
  ) {
    throw internalError();
  }
  return {
    supportedExtensions: [...supportedExtensions],
    maxFileBytes: value.maxFileBytes,
    maxDurationMs: value.maxDurationMs,
    maxSegments: value.maxSegments,
    device: "cpu",
    computeType: "int8",
    model: value.model,
    modelRevision: value.modelRevision,
    workerAvailable: value.workerAvailable,
    modelAvailable: value.modelAvailable,
    busy: value.busy,
  };
}

function parseProgress(value: unknown): AudioTranscriptionProgress {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "jobId",
      "phase",
      "status",
      "bytesProcessed",
      "totalBytes",
      "processedMs",
      "durationMs",
      "errorCode",
    ]) ||
    !isUuid(value.jobId) ||
    typeof value.phase !== "string" ||
    !phases.has(value.phase as AudioTranscriptionPhase) ||
    typeof value.status !== "string" ||
    !statuses.has(value.status as AudioTranscriptionStatus) ||
    !isNonNegativeSafeInteger(value.bytesProcessed) ||
    !isNullableNonNegativeSafeInteger(value.totalBytes) ||
    (value.totalBytes !== null && value.bytesProcessed > value.totalBytes) ||
    !isNullableNonNegativeSafeInteger(value.processedMs) ||
    !isNullableNonNegativeSafeInteger(value.durationMs) ||
    (value.processedMs !== null &&
      value.durationMs !== null &&
      value.processedMs > value.durationMs) ||
    !isNullableErrorCode(value.errorCode) ||
    !phaseMatchesStatus(
      value.phase as AudioTranscriptionPhase,
      value.status as AudioTranscriptionStatus,
    )
  ) {
    throw internalError();
  }
  return {
    jobId: value.jobId,
    phase: value.phase as AudioTranscriptionPhase,
    status: value.status as AudioTranscriptionStatus,
    bytesProcessed: value.bytesProcessed,
    totalBytes: value.totalBytes,
    processedMs: value.processedMs,
    durationMs: value.durationMs,
    errorCode: value.errorCode as AudioTranscriptionErrorCode | null,
  };
}

function parseResult(
  value: unknown,
  expectedMeetingId?: string,
): AudioTranscriptionResult {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "cancelled",
      "jobId",
      "meeting",
      "source",
      "segmentCount",
    ]) ||
    typeof value.cancelled !== "boolean" ||
    !(value.jobId === null || isUuid(value.jobId)) ||
    !isNonNegativeSafeInteger(value.segmentCount)
  ) {
    throw internalError();
  }

  if (value.cancelled) {
    if (
      value.meeting !== null ||
      value.source !== null ||
      value.segmentCount !== 0
    ) {
      throw internalError();
    }
    return {
      cancelled: true,
      jobId: value.jobId as string | null,
      meeting: null,
      source: null,
      segmentCount: 0,
    };
  }

  if (value.jobId === null || value.meeting === null || value.source === null) {
    throw internalError();
  }
  const meeting = parseMeetingResponse(value.meeting);
  const source = parseTranscriptSourceResponse(value.source);
  if (
    source.meetingId !== meeting.id ||
    source.segmentCount !== value.segmentCount ||
    value.segmentCount === 0 ||
    (expectedMeetingId !== undefined && meeting.id !== expectedMeetingId)
  ) {
    throw internalError();
  }
  return {
    cancelled: false,
    jobId: value.jobId,
    meeting,
    source,
    segmentCount: value.segmentCount,
  };
}

function parseError(value: unknown): AudioTranscriptionApiError {
  if (
    isRecord(value) &&
    hasOnlyKeys(value, ["code", "message", "retryable"]) &&
    typeof value.code === "string" &&
    errorCodes.has(value.code as AudioTranscriptionErrorCode) &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  ) {
    return new AudioTranscriptionApiError(
      value.code as AudioTranscriptionErrorCode,
      value.retryable,
    );
  }
  return internalError();
}

function requireJobId(jobId: string): void {
  if (!isUuid(jobId)) {
    throw new AudioTranscriptionApiError("audio_invalid_input", false);
  }
}

function phaseMatchesStatus(
  phase: AudioTranscriptionPhase,
  status: AudioTranscriptionStatus,
): boolean {
  if (phase === "completed") return status === "completed";
  if (phase === "failed") return status === "failed";
  if (phase === "cancelled") return status === "cancelled";
  return status === "running";
}

function isNullableErrorCode(
  value: unknown,
): value is AudioTranscriptionErrorCode | null {
  return (
    value === null ||
    (typeof value === "string" &&
      errorCodes.has(value as AudioTranscriptionErrorCode))
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  allowed: readonly string[],
): boolean {
  const keys = Object.keys(value);
  return (
    keys.length === allowed.length && keys.every((key) => allowed.includes(key))
  );
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isNullableNonNegativeSafeInteger(
  value: unknown,
): value is number | null {
  return value === null || isNonNegativeSafeInteger(value);
}

function isSafeLabel(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    new TextEncoder().encode(value).byteLength <= 128 &&
    !Array.from(value).some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint <= 31 || codePoint === 127;
    })
  );
}

function internalError(): AudioTranscriptionApiError {
  return new AudioTranscriptionApiError("internal_error", false);
}
