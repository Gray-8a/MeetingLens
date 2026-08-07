import { invoke } from "@tauri-apps/api/core";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

export const summaryCommandNames = {
  getPolicy: "get_summary_policy",
  createRun: "create_summary_run",
  createRevision: "create_summary_revision",
  executeRun: "execute_summary_run",
  getRun: "get_summary_run",
  listRuns: "list_summary_runs",
  cancelRun: "cancel_summary_run",
  listEvidence: "list_summary_evidence",
} as const;

export type SummaryTemplate = "actions_decisions" | "concise" | "detailed";
export type SummaryOutputLanguage = "bilingual" | "en" | "zh";
export type SummaryStatus =
  | "cancelled"
  | "completed"
  | "failed"
  | "queued"
  | "running";
export type SummaryDecisionStatus = "confirmed" | "proposed" | "superseded";
export type SummaryActionStatus = "assigned" | "suggested" | "unconfirmed";
export type SummaryStaleReason = "segment_revised" | "source_deleted";

export interface CreateSummaryRunRequest {
  meetingId: string;
  sourceIds: string[];
  template: SummaryTemplate;
  outputLanguage: SummaryOutputLanguage;
}

export interface SummaryRunReferenceRequest {
  meetingId: string;
  summaryId: string;
}

export interface ListSummaryRunsRequest {
  meetingId: string;
  offset?: number;
  limit?: number;
}

export interface ListSummaryEvidenceRequest {
  meetingId: string;
  summaryId: string;
  cursor?: number | null;
  limit?: number;
}

export interface SummaryClaim {
  text: string;
  evidence_ids: string[];
}

export interface SummaryDecision extends SummaryClaim {
  status: SummaryDecisionStatus;
}

export interface SummaryActionItem extends SummaryClaim {
  owner: string | null;
  due_date: string | null;
  status: SummaryActionStatus;
}

export interface SummaryContent {
  schema_version: 1;
  overview: SummaryClaim;
  decisions: SummaryDecision[];
  action_items: SummaryActionItem[];
  risks: SummaryClaim[];
  open_questions: SummaryClaim[];
}

export type SummaryRunFailureCode =
  | "internal_error"
  | "meeting_invalid_state"
  | "meeting_not_found"
  | "summary_input_insufficient"
  | "summary_input_too_large"
  | "summary_interrupted"
  | "summary_invalid_input"
  | "summary_invalid_state"
  | "summary_persistence_failed"
  | "summary_provider_failed"
  | "summary_provider_output_invalid"
  | "summary_run_not_found"
  | "summary_segment_limit_exceeded"
  | "summary_snapshot_missing"
  | "summary_source_not_found"
  | "summary_unknown_evidence";

export interface SummaryRunResponse {
  id: string;
  meetingId: string;
  seriesId: string;
  previousSummaryId: string | null;
  revision: number;
  runPurpose: "final";
  triggerKind: "manual";
  template: SummaryTemplate;
  outputLanguage: SummaryOutputLanguage;
  status: SummaryStatus;
  provider: "deterministic_mock";
  model: string;
  promptVersion: string;
  content: SummaryContent | null;
  errorCode: SummaryRunFailureCode | null;
  sourceCount: number;
  inputSegmentCount: number;
  inputBytes: number;
  isStale: boolean;
  staleReason: SummaryStaleReason | null;
  staleAt: string | null;
  isUserEdited: boolean;
  snapshotCreatedAt: string;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  cancelRequestedAt: string | null;
  updatedAt: string;
}

export interface SummaryEvidenceResponse {
  id: string;
  summaryId: string;
  meetingId: string;
  kind: "transcript";
  sourceId: string;
  transcriptSegmentId: string;
  segmentRevision: number;
  label: string;
  speakerSnapshot: string | null;
  startOffsetMs: number | null;
  endOffsetMs: number | null;
  ordinal: number;
  globalOrdinal: number;
  excerpt: string;
  sourceDeleted: boolean;
  createdAt: string;
}

export interface ListSummaryRunsResponse {
  items: SummaryRunResponse[];
  offset: number;
  limit: number;
  totalCount: number;
}

export interface ListSummaryEvidenceResponse {
  items: SummaryEvidenceResponse[];
  nextCursor: number | null;
  hasMore: boolean;
  totalCount: number;
}

export interface SummaryPolicyResponse {
  templates: SummaryTemplate[];
  outputLanguages: SummaryOutputLanguage[];
  runPurpose: "final";
  triggerKind: "manual";
  provider: "deterministic_mock";
  maxSegments: number;
  maxInputBytes: number;
  maxEvidenceExcerptBytes: number;
  listDefaultLimit: number;
  listMaxLimit: number;
  evidenceDefaultLimit: number;
  evidenceMaxLimit: number;
}

export type SummaryErrorCode =
  | "summary_input_insufficient"
  | "summary_input_too_large"
  | "internal_error"
  | "summary_invalid_input"
  | "summary_invalid_state"
  | "meeting_invalid_state"
  | "meeting_not_found"
  | "summary_persistence_failed"
  | "summary_provider_failed"
  | "summary_provider_output_invalid"
  | "summary_run_not_found"
  | "summary_segment_limit_exceeded"
  | "summary_snapshot_missing"
  | "summary_source_not_found"
  | "summary_unknown_evidence";

const safeMessages: Record<SummaryErrorCode, string> = {
  summary_input_insufficient:
    "The selected sources do not contain usable final transcript segments.",
  summary_input_too_large: "The selected summary input is too large.",
  internal_error: "An internal error occurred.",
  summary_invalid_input: "The summary request is invalid.",
  summary_invalid_state:
    "The summary run cannot be changed in its current state.",
  meeting_invalid_state:
    "The meeting cannot be summarized in its current state.",
  meeting_not_found: "The meeting was not found.",
  summary_persistence_failed: "The summary run could not be saved.",
  summary_provider_failed: "The summary provider could not complete the run.",
  summary_provider_output_invalid:
    "The summary provider returned an invalid result.",
  summary_run_not_found: "The summary run was not found.",
  summary_segment_limit_exceeded:
    "The selected summary input contains too many segments.",
  summary_snapshot_missing: "The summary input snapshot is unavailable.",
  summary_source_not_found: "A selected transcript source was not found.",
  summary_unknown_evidence: "The summary result referenced unknown evidence.",
};

const errorCodes = new Set<SummaryErrorCode>(
  Object.keys(safeMessages) as SummaryErrorCode[],
);
const runFailureCodes = new Set<SummaryRunFailureCode>([
  ...errorCodes,
  "summary_interrupted",
]);
const summaryTemplates = new Set<SummaryTemplate>([
  "actions_decisions",
  "concise",
  "detailed",
]);
const outputLanguages = new Set<SummaryOutputLanguage>([
  "bilingual",
  "en",
  "zh",
]);
const summaryStatuses = new Set<SummaryStatus>([
  "cancelled",
  "completed",
  "failed",
  "queued",
  "running",
]);
const decisionStatuses = new Set<SummaryDecisionStatus>([
  "confirmed",
  "proposed",
  "superseded",
]);
const actionStatuses = new Set<SummaryActionStatus>([
  "assigned",
  "suggested",
  "unconfirmed",
]);
const staleReasons = new Set<SummaryStaleReason>([
  "segment_revised",
  "source_deleted",
]);
const frozenTemplates: readonly SummaryTemplate[] = [
  "detailed",
  "concise",
  "actions_decisions",
];
const frozenOutputLanguages: readonly SummaryOutputLanguage[] = [
  "zh",
  "en",
  "bilingual",
];

const summaryLimits = {
  maxSegments: 20_000,
  maxInputBytes: 10 * 1024 * 1024,
  maxEvidenceExcerptBytes: 1_024,
  maxResultBytes: 1024 * 1024,
  maxResultItemsPerSection: 500,
  maxResultItems: 1_000,
  maxResultTextBytes: 16 * 1024,
  maxResultMetadataBytes: 255,
} as const;

export const summaryRunPageLimits = {
  default: 50,
  maximum: 100,
} as const;

export const summaryEvidencePageLimits = {
  default: 100,
  maximum: 200,
} as const;

const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const utcIsoPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

export class SummaryApiError extends Error {
  readonly code: SummaryErrorCode;
  readonly retryable: boolean;

  constructor(code: SummaryErrorCode, retryable: boolean) {
    super(safeMessages[code]);
    this.name = "SummaryApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

const tauriInvokeAdapter: InvokeAdapter = {
  invoke: (command, args) => invoke<unknown>(command, args),
};

export interface SummaryApi {
  getPolicy(): Promise<SummaryPolicyResponse>;
  createRun(request: CreateSummaryRunRequest): Promise<SummaryRunResponse>;
  createRevision(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse>;
  executeRun(request: SummaryRunReferenceRequest): Promise<SummaryRunResponse>;
  getRun(request: SummaryRunReferenceRequest): Promise<SummaryRunResponse>;
  listRuns(request: ListSummaryRunsRequest): Promise<ListSummaryRunsResponse>;
  cancelRun(request: SummaryRunReferenceRequest): Promise<SummaryRunResponse>;
  listEvidence(
    request: ListSummaryEvidenceRequest,
  ): Promise<ListSummaryEvidenceResponse>;
}

export function createSummaryApi(
  adapter: InvokeAdapter = tauriInvokeAdapter,
): SummaryApi {
  return {
    getPolicy: () =>
      invokeAndParse(
        adapter,
        summaryCommandNames.getPolicy,
        undefined,
        parsePolicy,
      ),
    createRun: async (request) => {
      validateCreateRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.createRun,
        {
          request: {
            meetingId: request.meetingId,
            sourceIds: [...request.sourceIds],
            template: request.template,
            outputLanguage: request.outputLanguage,
          },
        },
        (value) => parseCreatedRun(value, request),
      );
    },
    createRevision: async (request) => {
      validateReferenceRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.createRevision,
        { request: pickReference(request) },
        (value) => parseCreatedRevision(value, request),
      );
    },
    executeRun: async (request) => {
      validateReferenceRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.executeRun,
        { request: pickReference(request) },
        (value) => {
          const run = parseReferencedRun(value, request);
          if (run.status !== "completed" && run.status !== "cancelled") {
            throw internalError();
          }
          return run;
        },
      );
    },
    getRun: async (request) => {
      validateReferenceRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.getRun,
        { request: pickReference(request) },
        (value) => parseReferencedRun(value, request),
      );
    },
    listRuns: async (request) => {
      const page = normalizeRunPage(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.listRuns,
        {
          request: {
            meetingId: request.meetingId,
            ...(request.offset !== undefined ? { offset: page.offset } : {}),
            ...(request.limit !== undefined ? { limit: page.limit } : {}),
          },
        },
        (value) => parseRunPage(value, request.meetingId, page),
      );
    },
    cancelRun: async (request) => {
      validateReferenceRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.cancelRun,
        { request: pickReference(request) },
        (value) => parseReferencedRun(value, request, "cancelled"),
      );
    },
    listEvidence: async (request) => {
      const page = normalizeEvidencePage(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.listEvidence,
        {
          request: {
            meetingId: request.meetingId,
            summaryId: request.summaryId,
            ...(page.cursor !== null ? { cursor: page.cursor } : {}),
            ...(request.limit !== undefined ? { limit: page.limit } : {}),
          },
        },
        (value) =>
          parseEvidencePage(value, request.meetingId, request.summaryId, page),
      );
    },
  };
}

export const summaryApi = createSummaryApi();

async function invokeAndParse<T>(
  adapter: InvokeAdapter,
  command: string,
  args: InvokeArguments | undefined,
  parse: (value: unknown) => T,
): Promise<T> {
  try {
    return parse(await adapter.invoke(command, args));
  } catch (error: unknown) {
    if (error instanceof SummaryApiError) throw error;
    throw parseSummaryError(error);
  }
}

function pickReference(
  request: SummaryRunReferenceRequest,
): SummaryRunReferenceRequest {
  return { meetingId: request.meetingId, summaryId: request.summaryId };
}

function validateCreateRequest(request: CreateSummaryRunRequest): void {
  if (
    !isRecord(request) ||
    !isUuid(request.meetingId) ||
    !Array.isArray(request.sourceIds) ||
    request.sourceIds.length === 0 ||
    request.sourceIds.length > summaryLimits.maxSegments ||
    !request.sourceIds.every(isUuid) ||
    new Set(request.sourceIds).size !== request.sourceIds.length ||
    typeof request.template !== "string" ||
    !summaryTemplates.has(request.template as SummaryTemplate) ||
    typeof request.outputLanguage !== "string" ||
    !outputLanguages.has(request.outputLanguage as SummaryOutputLanguage)
  ) {
    throw invalidInput();
  }
}

function validateReferenceRequest(request: SummaryRunReferenceRequest): void {
  if (
    !isRecord(request) ||
    !isUuid(request.meetingId) ||
    !isUuid(request.summaryId)
  ) {
    throw invalidInput();
  }
}

interface NormalizedRunPage {
  offset: number;
  limit: number;
}

function normalizeRunPage(request: ListSummaryRunsRequest): NormalizedRunPage {
  if (!isRecord(request) || !isUuid(request.meetingId)) throw invalidInput();
  const offset = request.offset === undefined ? 0 : request.offset;
  const limit =
    request.limit === undefined ? summaryRunPageLimits.default : request.limit;
  if (
    !isNonNegativeSafeInteger(offset) ||
    !isPositiveSafeInteger(limit) ||
    limit > summaryRunPageLimits.maximum
  ) {
    throw invalidInput();
  }
  return { offset, limit };
}

interface NormalizedEvidencePage {
  cursor: number | null;
  limit: number;
}

function normalizeEvidencePage(
  request: ListSummaryEvidenceRequest,
): NormalizedEvidencePage {
  validateReferenceRequest(request);
  const cursor = request.cursor === undefined ? null : request.cursor;
  const limit =
    request.limit === undefined
      ? summaryEvidencePageLimits.default
      : request.limit;
  if (
    !isNullableNonNegativeSafeInteger(cursor) ||
    !isPositiveSafeInteger(limit) ||
    limit > summaryEvidencePageLimits.maximum
  ) {
    throw invalidInput();
  }
  return { cursor, limit };
}

function parseCreatedRun(
  value: unknown,
  request: CreateSummaryRunRequest,
): SummaryRunResponse {
  const run = parseRun(value);
  if (
    run.meetingId !== request.meetingId ||
    run.status !== "queued" ||
    run.revision !== 1 ||
    run.previousSummaryId !== null ||
    run.template !== request.template ||
    run.outputLanguage !== request.outputLanguage ||
    run.sourceCount !== request.sourceIds.length
  ) {
    throw internalError();
  }
  return run;
}

function parseCreatedRevision(
  value: unknown,
  request: SummaryRunReferenceRequest,
): SummaryRunResponse {
  const run = parseRun(value);
  if (
    run.meetingId !== request.meetingId ||
    run.id === request.summaryId ||
    run.previousSummaryId !== request.summaryId ||
    run.revision <= 1 ||
    run.status !== "queued"
  ) {
    throw internalError();
  }
  return run;
}

function parseReferencedRun(
  value: unknown,
  request: SummaryRunReferenceRequest,
  expectedStatus?: SummaryStatus,
): SummaryRunResponse {
  const run = parseRun(value);
  if (
    run.meetingId !== request.meetingId ||
    run.id !== request.summaryId ||
    (expectedStatus !== undefined && run.status !== expectedStatus)
  ) {
    throw internalError();
  }
  return run;
}

function parseRun(value: unknown): SummaryRunResponse {
  const keys = [
    "id",
    "meetingId",
    "seriesId",
    "previousSummaryId",
    "revision",
    "runPurpose",
    "triggerKind",
    "template",
    "outputLanguage",
    "status",
    "provider",
    "model",
    "promptVersion",
    "content",
    "errorCode",
    "sourceCount",
    "inputSegmentCount",
    "inputBytes",
    "isStale",
    "staleReason",
    "staleAt",
    "isUserEdited",
    "snapshotCreatedAt",
    "createdAt",
    "startedAt",
    "finishedAt",
    "cancelRequestedAt",
    "updatedAt",
  ] as const;
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, keys) ||
    !isUuid(value.id) ||
    !isUuid(value.meetingId) ||
    !isUuid(value.seriesId) ||
    !isNullableUuid(value.previousSummaryId) ||
    !isPositiveSafeInteger(value.revision) ||
    value.runPurpose !== "final" ||
    value.triggerKind !== "manual" ||
    typeof value.template !== "string" ||
    !summaryTemplates.has(value.template as SummaryTemplate) ||
    typeof value.outputLanguage !== "string" ||
    !outputLanguages.has(value.outputLanguage as SummaryOutputLanguage) ||
    typeof value.status !== "string" ||
    !summaryStatuses.has(value.status as SummaryStatus) ||
    value.provider !== "deterministic_mock" ||
    !isSafeBoundedText(value.model, summaryLimits.maxResultMetadataBytes) ||
    !isSafeBoundedText(
      value.promptVersion,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    !(value.content === null || isRecord(value.content)) ||
    !isNullableRunFailureCode(value.errorCode) ||
    !isPositiveSafeInteger(value.sourceCount) ||
    !isPositiveSafeInteger(value.inputSegmentCount) ||
    value.sourceCount > value.inputSegmentCount ||
    value.inputSegmentCount > summaryLimits.maxSegments ||
    !isPositiveSafeInteger(value.inputBytes) ||
    value.inputBytes > summaryLimits.maxInputBytes ||
    typeof value.isStale !== "boolean" ||
    !isNullableStaleReason(value.staleReason) ||
    !isNullableUtcIso(value.staleAt) ||
    typeof value.isUserEdited !== "boolean" ||
    !isUtcIso(value.snapshotCreatedAt) ||
    !isUtcIso(value.createdAt) ||
    !isNullableUtcIso(value.startedAt) ||
    !isNullableUtcIso(value.finishedAt) ||
    !isNullableUtcIso(value.cancelRequestedAt) ||
    !isUtcIso(value.updatedAt) ||
    (value.revision === 1 && value.previousSummaryId !== null) ||
    (value.revision > 1 && value.previousSummaryId === null) ||
    value.previousSummaryId === value.id ||
    (value.isStale
      ? value.staleReason === null || value.staleAt === null
      : value.staleReason !== null || value.staleAt !== null)
  ) {
    throw internalError();
  }

  const status = value.status as SummaryStatus;
  const content = value.content === null ? null : parseContent(value.content);
  if (!hasValidRunState(value, status, content)) throw internalError();

  return {
    id: value.id,
    meetingId: value.meetingId,
    seriesId: value.seriesId,
    previousSummaryId: value.previousSummaryId,
    revision: value.revision,
    runPurpose: "final",
    triggerKind: "manual",
    template: value.template as SummaryTemplate,
    outputLanguage: value.outputLanguage as SummaryOutputLanguage,
    status,
    provider: "deterministic_mock",
    model: value.model,
    promptVersion: value.promptVersion,
    content,
    errorCode: value.errorCode,
    sourceCount: value.sourceCount,
    inputSegmentCount: value.inputSegmentCount,
    inputBytes: value.inputBytes,
    isStale: value.isStale,
    staleReason: value.staleReason,
    staleAt: value.staleAt,
    isUserEdited: value.isUserEdited,
    snapshotCreatedAt: value.snapshotCreatedAt,
    createdAt: value.createdAt,
    startedAt: value.startedAt,
    finishedAt: value.finishedAt,
    cancelRequestedAt: value.cancelRequestedAt,
    updatedAt: value.updatedAt,
  };
}

function hasValidRunState(
  value: Record<string, unknown>,
  status: SummaryStatus,
  content: SummaryContent | null,
): boolean {
  switch (status) {
    case "queued":
      return (
        content === null &&
        value.errorCode === null &&
        value.startedAt === null &&
        value.finishedAt === null &&
        value.cancelRequestedAt === null
      );
    case "running":
      return (
        content === null &&
        value.errorCode === null &&
        value.startedAt !== null &&
        value.finishedAt === null
      );
    case "completed":
      return (
        content !== null &&
        value.errorCode === null &&
        value.startedAt !== null &&
        value.finishedAt !== null &&
        value.cancelRequestedAt === null
      );
    case "failed":
      return (
        content === null &&
        value.errorCode !== null &&
        value.startedAt !== null &&
        value.finishedAt !== null &&
        value.cancelRequestedAt === null
      );
    case "cancelled":
      return (
        content === null &&
        value.errorCode === null &&
        value.finishedAt !== null &&
        value.cancelRequestedAt !== null
      );
  }
}

function parseContent(value: Record<string, unknown>): SummaryContent {
  if (
    !hasExactlyKeys(value, [
      "schema_version",
      "overview",
      "decisions",
      "action_items",
      "risks",
      "open_questions",
    ]) ||
    value.schema_version !== 1 ||
    !Array.isArray(value.decisions) ||
    !Array.isArray(value.action_items) ||
    !Array.isArray(value.risks) ||
    !Array.isArray(value.open_questions) ||
    value.decisions.length > summaryLimits.maxResultItemsPerSection ||
    value.action_items.length > summaryLimits.maxResultItemsPerSection ||
    value.risks.length > summaryLimits.maxResultItemsPerSection ||
    value.open_questions.length > summaryLimits.maxResultItemsPerSection ||
    value.decisions.length +
      value.action_items.length +
      value.risks.length +
      value.open_questions.length >
      summaryLimits.maxResultItems
  ) {
    throw internalError();
  }
  const content: SummaryContent = {
    schema_version: 1,
    overview: parseClaim(value.overview),
    decisions: value.decisions.map(parseDecision),
    action_items: value.action_items.map(parseActionItem),
    risks: value.risks.map(parseClaim),
    open_questions: value.open_questions.map(parseClaim),
  };
  if (utf8Length(JSON.stringify(content)) > summaryLimits.maxResultBytes) {
    throw internalError();
  }
  return content;
}

function parseClaim(value: unknown): SummaryClaim {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["text", "evidence_ids"]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes)
  ) {
    throw internalError();
  }
  return {
    text: value.text,
    evidence_ids: parseEvidenceIds(value.evidence_ids),
  };
}

function parseDecision(value: unknown): SummaryDecision {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["text", "status", "evidence_ids"]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes) ||
    typeof value.status !== "string" ||
    !decisionStatuses.has(value.status as SummaryDecisionStatus)
  ) {
    throw internalError();
  }
  return {
    text: value.text,
    status: value.status as SummaryDecisionStatus,
    evidence_ids: parseEvidenceIds(value.evidence_ids),
  };
}

function parseActionItem(value: unknown): SummaryActionItem {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "text",
      "owner",
      "due_date",
      "status",
      "evidence_ids",
    ]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes) ||
    !isNullableSafeBoundedText(
      value.owner,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    !isNullableSafeBoundedText(
      value.due_date,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    typeof value.status !== "string" ||
    !actionStatuses.has(value.status as SummaryActionStatus)
  ) {
    throw internalError();
  }
  return {
    text: value.text,
    owner: value.owner,
    due_date: value.due_date,
    status: value.status as SummaryActionStatus,
    evidence_ids: parseEvidenceIds(value.evidence_ids),
  };
}

function parseEvidenceIds(value: unknown): string[] {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.length > summaryLimits.maxSegments ||
    !value.every(isUuid) ||
    new Set(value).size !== value.length
  ) {
    throw internalError();
  }
  return [...value];
}

function parseRunPage(
  value: unknown,
  expectedMeetingId: string,
  page: NormalizedRunPage,
): ListSummaryRunsResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["items", "offset", "limit", "totalCount"]) ||
    !Array.isArray(value.items) ||
    value.offset !== page.offset ||
    value.limit !== page.limit ||
    !isNonNegativeSafeInteger(value.totalCount)
  ) {
    throw internalError();
  }
  const items = value.items.map(parseRun);
  const ids = items.map((run) => run.id);
  if (
    items.length > page.limit ||
    new Set(ids).size !== ids.length ||
    items.some((run) => run.meetingId !== expectedMeetingId) ||
    (items.length > 0 && value.totalCount < page.offset + items.length) ||
    (value.totalCount > page.offset && items.length === 0)
  ) {
    throw internalError();
  }
  return {
    items,
    offset: value.offset,
    limit: value.limit,
    totalCount: value.totalCount,
  };
}

function parseEvidencePage(
  value: unknown,
  expectedMeetingId: string,
  expectedSummaryId: string,
  page: NormalizedEvidencePage,
): ListSummaryEvidenceResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["items", "nextCursor", "hasMore", "totalCount"]) ||
    !Array.isArray(value.items) ||
    !isNullableNonNegativeSafeInteger(value.nextCursor) ||
    typeof value.hasMore !== "boolean" ||
    !isNonNegativeSafeInteger(value.totalCount)
  ) {
    throw internalError();
  }
  const items = value.items.map(parseEvidence);
  const ids = items.map((item) => item.id);
  const ordinals = items.map((item) => item.globalOrdinal);
  if (
    items.length > page.limit ||
    value.totalCount < items.length ||
    new Set(ids).size !== ids.length ||
    items.some(
      (item) =>
        item.meetingId !== expectedMeetingId ||
        item.summaryId !== expectedSummaryId ||
        (page.cursor !== null && item.globalOrdinal <= page.cursor),
    ) ||
    !isStrictlyIncreasing(ordinals) ||
    (value.hasMore &&
      (items.length === 0 ||
        value.nextCursor !== items[items.length - 1]?.globalOrdinal)) ||
    (!value.hasMore && value.nextCursor !== null)
  ) {
    throw internalError();
  }
  return {
    items,
    nextCursor: value.nextCursor,
    hasMore: value.hasMore,
    totalCount: value.totalCount,
  };
}

function parseEvidence(value: unknown): SummaryEvidenceResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "id",
      "summaryId",
      "meetingId",
      "kind",
      "sourceId",
      "transcriptSegmentId",
      "segmentRevision",
      "label",
      "speakerSnapshot",
      "startOffsetMs",
      "endOffsetMs",
      "ordinal",
      "globalOrdinal",
      "excerpt",
      "sourceDeleted",
      "createdAt",
    ]) ||
    !isUuid(value.id) ||
    !isUuid(value.summaryId) ||
    !isUuid(value.meetingId) ||
    value.kind !== "transcript" ||
    !isUuid(value.sourceId) ||
    !isUuid(value.transcriptSegmentId) ||
    !isPositiveSafeInteger(value.segmentRevision) ||
    !isSafeBoundedText(value.label, summaryLimits.maxResultMetadataBytes) ||
    !isNullableSafeBoundedText(
      value.speakerSnapshot,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    !isNullableNonNegativeSafeInteger(value.startOffsetMs) ||
    !isNullableNonNegativeSafeInteger(value.endOffsetMs) ||
    (value.startOffsetMs !== null &&
      value.endOffsetMs !== null &&
      value.endOffsetMs < value.startOffsetMs) ||
    !isNonNegativeSafeInteger(value.ordinal) ||
    !isNonNegativeSafeInteger(value.globalOrdinal) ||
    !isSafeBoundedText(value.excerpt, summaryLimits.maxEvidenceExcerptBytes) ||
    typeof value.sourceDeleted !== "boolean" ||
    !isUtcIso(value.createdAt)
  ) {
    throw internalError();
  }
  return {
    id: value.id,
    summaryId: value.summaryId,
    meetingId: value.meetingId,
    kind: "transcript",
    sourceId: value.sourceId,
    transcriptSegmentId: value.transcriptSegmentId,
    segmentRevision: value.segmentRevision,
    label: value.label,
    speakerSnapshot: value.speakerSnapshot,
    startOffsetMs: value.startOffsetMs,
    endOffsetMs: value.endOffsetMs,
    ordinal: value.ordinal,
    globalOrdinal: value.globalOrdinal,
    excerpt: value.excerpt,
    sourceDeleted: value.sourceDeleted,
    createdAt: value.createdAt,
  };
}

function parsePolicy(value: unknown): SummaryPolicyResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "templates",
      "outputLanguages",
      "runPurpose",
      "triggerKind",
      "provider",
      "maxSegments",
      "maxInputBytes",
      "maxEvidenceExcerptBytes",
      "listDefaultLimit",
      "listMaxLimit",
      "evidenceDefaultLimit",
      "evidenceMaxLimit",
    ]) ||
    !hasExactArray(value.templates, frozenTemplates) ||
    !hasExactArray(value.outputLanguages, frozenOutputLanguages) ||
    value.runPurpose !== "final" ||
    value.triggerKind !== "manual" ||
    value.provider !== "deterministic_mock" ||
    value.maxSegments !== summaryLimits.maxSegments ||
    value.maxInputBytes !== summaryLimits.maxInputBytes ||
    value.maxEvidenceExcerptBytes !== summaryLimits.maxEvidenceExcerptBytes ||
    value.listDefaultLimit !== summaryRunPageLimits.default ||
    value.listMaxLimit !== summaryRunPageLimits.maximum ||
    value.evidenceDefaultLimit !== summaryEvidencePageLimits.default ||
    value.evidenceMaxLimit !== summaryEvidencePageLimits.maximum
  ) {
    throw internalError();
  }
  return {
    templates: [...frozenTemplates],
    outputLanguages: [...frozenOutputLanguages],
    runPurpose: "final",
    triggerKind: "manual",
    provider: "deterministic_mock",
    maxSegments: summaryLimits.maxSegments,
    maxInputBytes: summaryLimits.maxInputBytes,
    maxEvidenceExcerptBytes: summaryLimits.maxEvidenceExcerptBytes,
    listDefaultLimit: summaryRunPageLimits.default,
    listMaxLimit: summaryRunPageLimits.maximum,
    evidenceDefaultLimit: summaryEvidencePageLimits.default,
    evidenceMaxLimit: summaryEvidencePageLimits.maximum,
  };
}

function parseSummaryError(value: unknown): SummaryApiError {
  if (
    isRecord(value) &&
    hasExactlyKeys(value, ["code", "message", "retryable"]) &&
    typeof value.code === "string" &&
    errorCodes.has(value.code as SummaryErrorCode) &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  ) {
    return new SummaryApiError(value.code as SummaryErrorCode, value.retryable);
  }
  return internalError();
}

function invalidInput(): SummaryApiError {
  return new SummaryApiError("summary_invalid_input", false);
}

function internalError(): SummaryApiError {
  return new SummaryApiError("internal_error", false);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const actual = Object.keys(value);
  const expected = new Set(keys);
  return (
    actual.length === expected.size && actual.every((key) => expected.has(key))
  );
}

function hasExactArray(value: unknown, expected: readonly string[]): boolean {
  return (
    Array.isArray(value) &&
    value.length === expected.length &&
    value.every((item, index) => item === expected[index])
  );
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isNullableUuid(value: unknown): value is string | null {
  return value === null || isUuid(value);
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

function isSafeBoundedText(value: unknown, maxBytes: number): value is string {
  return (
    typeof value === "string" &&
    value.trim().length > 0 &&
    utf8Length(value) <= maxBytes &&
    !Array.from(value).some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return (
        (codePoint <= 0x1f && character !== "\n" && character !== "\t") ||
        (codePoint >= 0x7f && codePoint <= 0x9f)
      );
    })
  );
}

function isNullableSafeBoundedText(
  value: unknown,
  maxBytes: number,
): value is string | null {
  return value === null || isSafeBoundedText(value, maxBytes);
}

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).length;
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

function isNullableRunFailureCode(
  value: unknown,
): value is SummaryRunFailureCode | null {
  return (
    value === null ||
    (typeof value === "string" &&
      runFailureCodes.has(value as SummaryRunFailureCode))
  );
}

function isNullableStaleReason(
  value: unknown,
): value is SummaryStaleReason | null {
  return (
    value === null ||
    (typeof value === "string" && staleReasons.has(value as SummaryStaleReason))
  );
}

function isStrictlyIncreasing(values: readonly number[]): boolean {
  return values.every(
    (value, index) => index === 0 || value > values[index - 1]!,
  );
}
