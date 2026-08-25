import { invoke } from "@tauri-apps/api/core";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

export const summaryCommandNames = {
  getPolicy: "get_summary_policy",
  getDeepSeekProviderStatus: "get_deepseek_provider_status",
  getExternalDisclosure: "get_summary_external_disclosure",
  confirmExternalDisclosure: "confirm_summary_external_disclosure",
  revokeDeepSeekConsent: "revoke_deepseek_consent",
  createRun: "create_summary_run",
  createRevision: "create_summary_revision",
  executeRun: "execute_summary_run",
  getRun: "get_summary_run",
  listRuns: "list_summary_runs",
  cancelRun: "cancel_summary_run",
  listEvidence: "list_summary_evidence",
  getEvidence: "get_summary_evidence",
  updateContent: "update_summary_content",
  getMarkdown: "get_summary_markdown",
  saveMarkdown: "save_summary_markdown",
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
export type SummaryProvider = "deepseek" | "deterministic_mock";
export type SummaryProviderMode = "external" | "mock";

export type SummaryExternalDataCategory =
  | "evidence_ids"
  | "output_language"
  | "speaker_labels_and_names"
  | "summary_template"
  | "time_offsets"
  | "transcript_text";

export interface DeepSeekProviderStatusResponse {
  keyConfigured: boolean;
  provider: "deepseek";
  endpointHost: "api.deepseek.com";
  model: "deepseek-v4-pro";
  consentRemembered: boolean;
  consentedAt: string | null;
  policyVersion: string;
  executionAvailable: boolean;
}

export interface SummaryExternalDisclosureResponse {
  provider: "deepseek";
  endpointHost: "api.deepseek.com";
  model: "deepseek-v4-pro";
  sourceCount: number;
  segmentCount: number;
  inputBytes: number;
  template: SummaryTemplate;
  outputLanguage: SummaryOutputLanguage;
  dataCategories: SummaryExternalDataCategory[];
  rememberedConsentValid: boolean;
  requiresConfirmation: boolean;
  policyVersion: string;
}

export interface ConfirmSummaryExternalDisclosureRequest {
  summaryId: string;
  remember: boolean;
}

export interface SummaryExternalDisclosureRequest {
  summaryId: string;
}

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

export interface GetSummaryEvidenceRequest extends SummaryRunReferenceRequest {
  evidenceId: string;
}

export interface SummaryEditableActionItem {
  text: string;
  owner: string | null;
  dueDate: string | null;
}

export interface SummaryEditableContent {
  overviewText: string;
  decisionTexts: string[];
  actionItems: SummaryEditableActionItem[];
  riskTexts: string[];
  openQuestionTexts: string[];
}

export interface UpdateSummaryContentRequest
  extends SummaryRunReferenceRequest {
  expectedContentVersion: string;
  editableContent: SummaryEditableContent;
}

export interface SummaryMarkdownRequest extends SummaryRunReferenceRequest {
  expectedContentVersion: string;
}

export interface SummaryMarkdownResponse {
  summaryId: string;
  contentVersion: string;
  markdown: string;
  bytes: number;
}

export interface SaveSummaryMarkdownResponse {
  summaryId: string;
  contentVersion: string;
  saved: boolean;
  cancelled: boolean;
  bytesWritten: number;
}

export interface ListSummaryRunsRequest {
  meetingId: string;
  cursor?: string | null;
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
  evidenceIds: string[];
}

export interface SummaryDecision extends SummaryClaim {
  status: SummaryDecisionStatus;
}

export interface SummaryActionItem extends SummaryClaim {
  owner: string | null;
  dueDate: string | null;
  status: SummaryActionStatus;
}

export interface SummaryContent {
  schemaVersion: 1;
  overview: SummaryClaim;
  decisions: SummaryDecision[];
  actionItems: SummaryActionItem[];
  risks: SummaryClaim[];
  openQuestions: SummaryClaim[];
}

export type SummaryRunFailureCode = SummaryErrorCode | "summary_interrupted";

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
  provider: SummaryProvider;
  providerMode: SummaryProviderMode;
  model: string;
  promptVersion: string;
  content: SummaryContent | null;
  contentVersion: string | null;
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
  nextCursor: string | null;
  hasMore: boolean;
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
  provider: SummaryProvider;
  maxSegments: number;
  maxInputBytes: number;
  maxEvidenceExcerptBytes: number;
  listDefaultLimit: number;
  listMaxLimit: number;
  evidenceDefaultLimit: number;
  evidenceMaxLimit: number;
}

export type SummaryErrorCode =
  | "deepseek_access_denied"
  | "deepseek_api_key_missing"
  | "deepseek_auth_failed"
  | "deepseek_consent_invalid"
  | "deepseek_consent_required"
  | "deepseek_content_filtered"
  | "deepseek_insufficient_balance"
  | "deepseek_model_unavailable"
  | "deepseek_network_failed"
  | "deepseek_output_truncated"
  | "deepseek_rate_limited"
  | "deepseek_request_invalid"
  | "deepseek_response_empty"
  | "deepseek_response_invalid"
  | "deepseek_service_unavailable"
  | "deepseek_timeout"
  | "summary_input_insufficient"
  | "summary_input_too_large"
  | "summary_provider_input_too_large"
  | "summary_evidence_invalid"
  | "summary_edit_conflict"
  | "summary_edit_too_large"
  | "summary_export_failed"
  | "summary_markdown_too_large"
  | "internal_error"
  | "summary_invalid_input"
  | "summary_invalid_state"
  | "meeting_invalid_state"
  | "meeting_not_found"
  | "summary_persistence_failed"
  | "summary_provider_failed"
  | "summary_provider_output_invalid"
  | "summary_revision_conflict"
  | "summary_run_not_found"
  | "summary_segment_limit_exceeded"
  | "summary_snapshot_missing"
  | "summary_source_not_found";

const safeMessages: Record<SummaryErrorCode, string> = {
  deepseek_access_denied: "DeepSeek denied access to this request.",
  deepseek_api_key_missing: "Configure the DeepSeek API key before sending.",
  deepseek_auth_failed: "DeepSeek could not authenticate this request.",
  deepseek_consent_invalid:
    "The DeepSeek sharing confirmation is no longer valid.",
  deepseek_consent_required: "Review and confirm the DeepSeek sharing details.",
  deepseek_content_filtered: "DeepSeek could not return this summary.",
  deepseek_insufficient_balance:
    "The DeepSeek account has insufficient balance.",
  deepseek_model_unavailable: "The configured DeepSeek model is unavailable.",
  deepseek_network_failed: "MeetingLens could not reach DeepSeek.",
  deepseek_output_truncated: "DeepSeek returned an incomplete summary.",
  deepseek_rate_limited: "DeepSeek is receiving too many requests.",
  deepseek_request_invalid: "DeepSeek rejected the summary request.",
  deepseek_response_empty: "DeepSeek returned an empty response.",
  deepseek_response_invalid: "DeepSeek returned an invalid response.",
  deepseek_service_unavailable: "DeepSeek is temporarily unavailable.",
  deepseek_timeout: "DeepSeek did not respond in time.",
  summary_input_insufficient:
    "The selected sources do not contain usable final transcript segments.",
  summary_input_too_large: "The selected summary input is too large.",
  summary_provider_input_too_large:
    "The selected summary input is too large for DeepSeek.",
  summary_evidence_invalid:
    "The summary result contains invalid evidence references.",
  summary_edit_conflict: "The summary changed before this edit could be saved.",
  summary_edit_too_large: "The edited summary is too large.",
  summary_export_failed: "The Markdown file could not be saved.",
  summary_markdown_too_large: "The Markdown result is too large.",
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
  summary_revision_conflict: "The summary revision is no longer current.",
  summary_run_not_found: "The summary run was not found.",
  summary_segment_limit_exceeded:
    "The selected summary input contains too many segments.",
  summary_snapshot_missing: "The summary input snapshot is unavailable.",
  summary_source_not_found: "A selected transcript source was not found.",
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
const deepSeekPolicyVersion = "deepseek-transcript-external-v1";
const deepSeekProvider = "deepseek";
const deepSeekEndpointHost = "api.deepseek.com";
const deepSeekModel = "deepseek-v4-pro";
const deepSeekPromptVersion = "summary-deepseek-v1";
const deepSeekMaxInputBytes = 1024 * 1024;
const deterministicMockModel = "meetinglens-deterministic-mock-v1";
const deterministicMockPromptVersion = "summary-core-mock-v1";
const frozenExternalDataCategories: readonly SummaryExternalDataCategory[] = [
  "transcript_text",
  "speaker_labels_and_names",
  "time_offsets",
  "evidence_ids",
  "summary_template",
  "output_language",
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
  default: 50,
  maximum: 100,
} as const;

const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const utcIsoPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const contentVersionPattern = /^[0-9a-f]{64}$/;

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
  getDeepSeekProviderStatus(): Promise<DeepSeekProviderStatusResponse>;
  getExternalDisclosure(
    request: SummaryExternalDisclosureRequest,
  ): Promise<SummaryExternalDisclosureResponse>;
  confirmExternalDisclosure(
    request: ConfirmSummaryExternalDisclosureRequest,
  ): Promise<SummaryExternalDisclosureResponse>;
  revokeDeepSeekConsent(): Promise<DeepSeekProviderStatusResponse>;
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
  getEvidence(
    request: GetSummaryEvidenceRequest,
  ): Promise<SummaryEvidenceResponse>;
  updateContent(
    request: UpdateSummaryContentRequest,
  ): Promise<SummaryRunResponse>;
  getMarkdown(
    request: SummaryMarkdownRequest,
  ): Promise<SummaryMarkdownResponse>;
  saveMarkdown(
    request: SummaryMarkdownRequest,
  ): Promise<SaveSummaryMarkdownResponse>;
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
    getDeepSeekProviderStatus: () =>
      invokeAndParse(
        adapter,
        summaryCommandNames.getDeepSeekProviderStatus,
        undefined,
        parseDeepSeekProviderStatus,
      ),
    getExternalDisclosure: async (request) => {
      validateDisclosureRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.getExternalDisclosure,
        { request: { summaryId: request.summaryId } },
        parseExternalDisclosure,
      );
    },
    confirmExternalDisclosure: async (request) => {
      validateDisclosureRequest(request);
      if (typeof request.remember !== "boolean") throw invalidInput();
      const disclosure = await invokeAndParse(
        adapter,
        summaryCommandNames.confirmExternalDisclosure,
        {
          request: {
            summaryId: request.summaryId,
            remember: request.remember,
          },
        },
        parseExternalDisclosure,
      );
      if (
        disclosure.requiresConfirmation ||
        disclosure.rememberedConsentValid !== request.remember
      ) {
        throw internalError();
      }
      return disclosure;
    },
    revokeDeepSeekConsent: () =>
      invokeAndParse(
        adapter,
        summaryCommandNames.revokeDeepSeekConsent,
        undefined,
        (value) => {
          const status = parseDeepSeekProviderStatus(value);
          if (status.consentRemembered || status.consentedAt !== null) {
            throw internalError();
          }
          return status;
        },
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
            ...(page.cursor !== null ? { cursor: page.cursor } : {}),
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
    getEvidence: async (request) => {
      validateEvidenceReferenceRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.getEvidence,
        {
          request: {
            meetingId: request.meetingId,
            summaryId: request.summaryId,
            evidenceId: request.evidenceId,
          },
        },
        (value) => {
          const item = parseEvidence(value);
          if (
            item.meetingId !== request.meetingId ||
            item.summaryId !== request.summaryId ||
            item.id !== request.evidenceId
          ) {
            throw internalError();
          }
          return item;
        },
      );
    },
    updateContent: async (request) => {
      validateUpdateContentRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.updateContent,
        {
          request: {
            meetingId: request.meetingId,
            summaryId: request.summaryId,
            expectedContentVersion: request.expectedContentVersion,
            editableContent: cloneEditableContent(request.editableContent),
          },
        },
        (value) => {
          const run = parseReferencedRun(value, request, "completed");
          if (run.contentVersion === null) throw internalError();
          return run;
        },
      );
    },
    getMarkdown: async (request) => {
      validateMarkdownRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.getMarkdown,
        { request: pickMarkdownReference(request) },
        (value) => parseMarkdown(value, request),
      );
    },
    saveMarkdown: async (request) => {
      validateMarkdownRequest(request);
      return invokeAndParse(
        adapter,
        summaryCommandNames.saveMarkdown,
        { request: pickMarkdownReference(request) },
        (value) => parseSaveMarkdown(value, request),
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

function validateDisclosureRequest(
  request: SummaryExternalDisclosureRequest,
): void {
  if (!isRecord(request) || !isUuid(request.summaryId)) throw invalidInput();
}

function validateEvidenceReferenceRequest(
  request: GetSummaryEvidenceRequest,
): void {
  validateReferenceRequest(request);
  if (!isUuid(request.evidenceId)) throw invalidInput();
}

function validateUpdateContentRequest(
  request: UpdateSummaryContentRequest,
): void {
  validateReferenceRequest(request);
  if (
    !isContentVersion(request.expectedContentVersion) ||
    !isRecord(request.editableContent) ||
    !hasExactlyKeys(request.editableContent, [
      "overviewText",
      "decisionTexts",
      "actionItems",
      "riskTexts",
      "openQuestionTexts",
    ]) ||
    !isEditableText(request.editableContent.overviewText) ||
    !isEditableTextArray(request.editableContent.decisionTexts) ||
    !Array.isArray(request.editableContent.actionItems) ||
    request.editableContent.actionItems.length >
      summaryLimits.maxResultItemsPerSection ||
    !request.editableContent.actionItems.every(isEditableActionItem) ||
    !isEditableTextArray(request.editableContent.riskTexts) ||
    !isEditableTextArray(request.editableContent.openQuestionTexts) ||
    request.editableContent.decisionTexts.length +
      request.editableContent.actionItems.length +
      request.editableContent.riskTexts.length +
      request.editableContent.openQuestionTexts.length >
      summaryLimits.maxResultItems
  ) {
    throw invalidInput();
  }
}

function validateMarkdownRequest(request: SummaryMarkdownRequest): void {
  validateReferenceRequest(request);
  if (!isContentVersion(request.expectedContentVersion)) throw invalidInput();
}

function pickMarkdownReference(
  request: SummaryMarkdownRequest,
): SummaryMarkdownRequest {
  return {
    meetingId: request.meetingId,
    summaryId: request.summaryId,
    expectedContentVersion: request.expectedContentVersion,
  };
}

function cloneEditableContent(
  content: SummaryEditableContent,
): SummaryEditableContent {
  return {
    overviewText: content.overviewText,
    decisionTexts: [...content.decisionTexts],
    actionItems: content.actionItems.map((item) => ({ ...item })),
    riskTexts: [...content.riskTexts],
    openQuestionTexts: [...content.openQuestionTexts],
  };
}

interface NormalizedRunPage {
  cursor: string | null;
  limit: number;
}

function normalizeRunPage(request: ListSummaryRunsRequest): NormalizedRunPage {
  if (!isRecord(request) || !isUuid(request.meetingId)) throw invalidInput();
  const cursor = request.cursor === undefined ? null : request.cursor;
  const limit =
    request.limit === undefined ? summaryRunPageLimits.default : request.limit;
  if (
    (cursor !== null && !isUuid(cursor)) ||
    !isPositiveSafeInteger(limit) ||
    limit > summaryRunPageLimits.maximum
  ) {
    throw invalidInput();
  }
  return { cursor, limit };
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
    "providerMode",
    "model",
    "promptVersion",
    "content",
    "contentVersion",
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
    !hasValidProviderIdentity(value) ||
    !isSafeBoundedText(value.model, summaryLimits.maxResultMetadataBytes) ||
    !isSafeBoundedText(
      value.promptVersion,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    !(value.content === null || isRecord(value.content)) ||
    !isNullableContentVersion(value.contentVersion) ||
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
  if (
    !hasValidRunState(value, status, content) ||
    (status === "completed"
      ? value.contentVersion === null
      : value.contentVersion !== null)
  ) {
    throw internalError();
  }

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
    provider: value.provider as SummaryProvider,
    providerMode: value.providerMode as SummaryProviderMode,
    model: value.model,
    promptVersion: value.promptVersion,
    content,
    contentVersion: value.contentVersion,
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
      "schemaVersion",
      "overview",
      "decisions",
      "actionItems",
      "risks",
      "openQuestions",
    ]) ||
    value.schemaVersion !== 1 ||
    !Array.isArray(value.decisions) ||
    !Array.isArray(value.actionItems) ||
    !Array.isArray(value.risks) ||
    !Array.isArray(value.openQuestions) ||
    value.decisions.length > summaryLimits.maxResultItemsPerSection ||
    value.actionItems.length > summaryLimits.maxResultItemsPerSection ||
    value.risks.length > summaryLimits.maxResultItemsPerSection ||
    value.openQuestions.length > summaryLimits.maxResultItemsPerSection ||
    value.decisions.length +
      value.actionItems.length +
      value.risks.length +
      value.openQuestions.length >
      summaryLimits.maxResultItems
  ) {
    throw internalError();
  }
  const content: SummaryContent = {
    schemaVersion: 1,
    overview: parseClaim(value.overview),
    decisions: value.decisions.map(parseDecision),
    actionItems: value.actionItems.map(parseActionItem),
    risks: value.risks.map(parseClaim),
    openQuestions: value.openQuestions.map(parseClaim),
  };
  if (utf8Length(JSON.stringify(content)) > summaryLimits.maxResultBytes) {
    throw internalError();
  }
  return content;
}

function parseClaim(value: unknown): SummaryClaim {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["text", "evidenceIds"]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes)
  ) {
    throw internalError();
  }
  return {
    text: value.text,
    evidenceIds: parseEvidenceIds(value.evidenceIds),
  };
}

function parseDecision(value: unknown): SummaryDecision {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, ["text", "status", "evidenceIds"]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes) ||
    typeof value.status !== "string" ||
    !decisionStatuses.has(value.status as SummaryDecisionStatus)
  ) {
    throw internalError();
  }
  return {
    text: value.text,
    status: value.status as SummaryDecisionStatus,
    evidenceIds: parseEvidenceIds(value.evidenceIds),
  };
}

function parseActionItem(value: unknown): SummaryActionItem {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "text",
      "owner",
      "dueDate",
      "status",
      "evidenceIds",
    ]) ||
    !isSafeBoundedText(value.text, summaryLimits.maxResultTextBytes) ||
    !isNullableSafeBoundedText(
      value.owner,
      summaryLimits.maxResultMetadataBytes,
    ) ||
    !isNullableSafeBoundedText(
      value.dueDate,
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
    dueDate: value.dueDate,
    status: value.status as SummaryActionStatus,
    evidenceIds: parseEvidenceIds(value.evidenceIds),
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
    !hasExactlyKeys(value, ["items", "nextCursor", "hasMore", "totalCount"]) ||
    !Array.isArray(value.items) ||
    (value.nextCursor !== null && !isUuid(value.nextCursor)) ||
    typeof value.hasMore !== "boolean" ||
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
    (page.cursor !== null && ids.includes(page.cursor)) ||
    !isRunPageOrder(items) ||
    value.totalCount < items.length ||
    (page.cursor === null && value.totalCount > 0 && items.length === 0) ||
    (value.hasMore &&
      (items.length === 0 ||
        value.nextCursor !== items[items.length - 1]?.id)) ||
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

function isRunPageOrder(items: readonly SummaryRunResponse[]): boolean {
  for (let index = 1; index < items.length; index += 1) {
    const previous = items[index - 1];
    const current = items[index];
    if (previous === undefined || current === undefined) return false;
    const previousTime = Date.parse(previous.createdAt);
    const currentTime = Date.parse(current.createdAt);
    if (
      previousTime < currentTime ||
      (previousTime === currentTime && previous.id >= current.id)
    ) {
      return false;
    }
  }
  return true;
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

function parseMarkdown(
  value: unknown,
  request: SummaryMarkdownRequest,
): SummaryMarkdownResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "summaryId",
      "contentVersion",
      "markdown",
      "bytes",
    ]) ||
    value.summaryId !== request.summaryId ||
    value.contentVersion !== request.expectedContentVersion ||
    typeof value.markdown !== "string" ||
    !isContentVersion(value.contentVersion) ||
    !isNonNegativeSafeInteger(value.bytes) ||
    value.bytes !== utf8Length(value.markdown) ||
    value.bytes > 10 * 1024 * 1024
  ) {
    throw internalError();
  }
  return {
    summaryId: value.summaryId,
    contentVersion: value.contentVersion,
    markdown: value.markdown,
    bytes: value.bytes,
  };
}

function parseSaveMarkdown(
  value: unknown,
  request: SummaryMarkdownRequest,
): SaveSummaryMarkdownResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "summaryId",
      "contentVersion",
      "saved",
      "cancelled",
      "bytesWritten",
    ]) ||
    value.summaryId !== request.summaryId ||
    value.contentVersion !== request.expectedContentVersion ||
    !isContentVersion(value.contentVersion) ||
    typeof value.saved !== "boolean" ||
    typeof value.cancelled !== "boolean" ||
    value.saved === value.cancelled ||
    !isNonNegativeSafeInteger(value.bytesWritten) ||
    (value.cancelled && value.bytesWritten !== 0) ||
    (value.saved && value.bytesWritten === 0) ||
    value.bytesWritten > 10 * 1024 * 1024
  ) {
    throw internalError();
  }
  return {
    summaryId: value.summaryId,
    contentVersion: value.contentVersion,
    saved: value.saved,
    cancelled: value.cancelled,
    bytesWritten: value.bytesWritten,
  };
}

function parsePolicy(value: unknown): SummaryPolicyResponse {
  const expectedMaxInputBytes =
    isRecord(value) && value.provider === deepSeekProvider
      ? deepSeekMaxInputBytes
      : summaryLimits.maxInputBytes;
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
    (value.provider !== "deterministic_mock" &&
      value.provider !== deepSeekProvider) ||
    value.maxSegments !== summaryLimits.maxSegments ||
    value.maxInputBytes !== expectedMaxInputBytes ||
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
    provider: value.provider as SummaryProvider,
    maxSegments: summaryLimits.maxSegments,
    maxInputBytes: expectedMaxInputBytes,
    maxEvidenceExcerptBytes: summaryLimits.maxEvidenceExcerptBytes,
    listDefaultLimit: summaryRunPageLimits.default,
    listMaxLimit: summaryRunPageLimits.maximum,
    evidenceDefaultLimit: summaryEvidencePageLimits.default,
    evidenceMaxLimit: summaryEvidencePageLimits.maximum,
  };
}

function parseDeepSeekProviderStatus(
  value: unknown,
): DeepSeekProviderStatusResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "keyConfigured",
      "provider",
      "endpointHost",
      "model",
      "consentRemembered",
      "consentedAt",
      "policyVersion",
      "executionAvailable",
    ]) ||
    typeof value.keyConfigured !== "boolean" ||
    value.provider !== deepSeekProvider ||
    value.endpointHost !== deepSeekEndpointHost ||
    value.model !== deepSeekModel ||
    typeof value.consentRemembered !== "boolean" ||
    !isNullableUtcIso(value.consentedAt) ||
    value.policyVersion !== deepSeekPolicyVersion ||
    typeof value.executionAvailable !== "boolean" ||
    value.executionAvailable !== value.keyConfigured ||
    (value.consentRemembered
      ? value.consentedAt === null
      : value.consentedAt !== null)
  ) {
    throw internalError();
  }
  return {
    keyConfigured: value.keyConfigured,
    provider: deepSeekProvider,
    endpointHost: deepSeekEndpointHost,
    model: deepSeekModel,
    consentRemembered: value.consentRemembered,
    consentedAt: value.consentedAt,
    policyVersion: deepSeekPolicyVersion,
    executionAvailable: value.executionAvailable,
  };
}

function parseExternalDisclosure(
  value: unknown,
): SummaryExternalDisclosureResponse {
  if (
    !isRecord(value) ||
    !hasExactlyKeys(value, [
      "provider",
      "endpointHost",
      "model",
      "sourceCount",
      "segmentCount",
      "inputBytes",
      "template",
      "outputLanguage",
      "dataCategories",
      "rememberedConsentValid",
      "requiresConfirmation",
      "policyVersion",
    ]) ||
    value.provider !== deepSeekProvider ||
    value.endpointHost !== deepSeekEndpointHost ||
    value.model !== deepSeekModel ||
    !isPositiveSafeInteger(value.sourceCount) ||
    !isPositiveSafeInteger(value.segmentCount) ||
    value.sourceCount > value.segmentCount ||
    value.segmentCount > summaryLimits.maxSegments ||
    !isPositiveSafeInteger(value.inputBytes) ||
    value.inputBytes > deepSeekMaxInputBytes ||
    typeof value.template !== "string" ||
    !summaryTemplates.has(value.template as SummaryTemplate) ||
    typeof value.outputLanguage !== "string" ||
    !outputLanguages.has(value.outputLanguage as SummaryOutputLanguage) ||
    !hasExactArray(value.dataCategories, frozenExternalDataCategories) ||
    typeof value.rememberedConsentValid !== "boolean" ||
    typeof value.requiresConfirmation !== "boolean" ||
    (value.rememberedConsentValid && value.requiresConfirmation) ||
    value.policyVersion !== deepSeekPolicyVersion
  ) {
    throw internalError();
  }
  return {
    provider: deepSeekProvider,
    endpointHost: deepSeekEndpointHost,
    model: deepSeekModel,
    sourceCount: value.sourceCount,
    segmentCount: value.segmentCount,
    inputBytes: value.inputBytes,
    template: value.template as SummaryTemplate,
    outputLanguage: value.outputLanguage as SummaryOutputLanguage,
    dataCategories: [...frozenExternalDataCategories],
    rememberedConsentValid: value.rememberedConsentValid,
    requiresConfirmation: value.requiresConfirmation,
    policyVersion: deepSeekPolicyVersion,
  };
}

function hasValidProviderIdentity(value: Record<string, unknown>): boolean {
  return (
    (value.provider === "deterministic_mock" &&
      value.providerMode === "mock" &&
      value.model === deterministicMockModel &&
      value.promptVersion === deterministicMockPromptVersion) ||
    (value.provider === deepSeekProvider &&
      value.providerMode === "external" &&
      value.model === deepSeekModel &&
      value.promptVersion === deepSeekPromptVersion)
  );
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

export function toSummaryError(value: unknown): SummaryApiError {
  if (value instanceof SummaryApiError) return value;
  return parseSummaryError(value);
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

function isContentVersion(value: unknown): value is string {
  return typeof value === "string" && contentVersionPattern.test(value);
}

function isNullableContentVersion(value: unknown): value is string | null {
  return value === null || isContentVersion(value);
}

function isEditableText(value: unknown): value is string {
  return isSafeBoundedText(value, summaryLimits.maxResultTextBytes);
}

function isEditableTextArray(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= summaryLimits.maxResultItemsPerSection &&
    value.every(isEditableText)
  );
}

function isEditableActionItem(
  value: unknown,
): value is SummaryEditableActionItem {
  return (
    isRecord(value) &&
    hasExactlyKeys(value, ["text", "owner", "dueDate"]) &&
    isEditableText(value.text) &&
    isNullableSafeBoundedText(
      value.owner,
      summaryLimits.maxResultMetadataBytes,
    ) &&
    isNullableSafeBoundedText(
      value.dueDate,
      summaryLimits.maxResultMetadataBytes,
    )
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
  if (typeof value !== "string" || !utcIsoPattern.test(value)) return false;
  const parsed = new Date(value);
  return Number.isFinite(parsed.getTime()) && parsed.toISOString() === value;
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
