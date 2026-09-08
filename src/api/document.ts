import { Channel, invoke } from "@tauri-apps/api/core";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

export const documentCommandNames = {
  selectAndImport: "select_and_import_documents",
  list: "list_documents",
  get: "get_document",
  requestDelete: "request_delete_document",
  retryDelete: "retry_delete_document",
  getImportPolicy: "get_document_import_policy",
  getParserPolicy: "get_document_parser_policy",
  parse: "parse_document",
  getParseStatus: "get_document_parse_status",
  cancelParse: "cancel_document_parse",
} as const;

export type DocumentStatus =
  | "importing"
  | "staged"
  | "import_failed"
  | "parsing"
  | "parsed"
  | "parse_failed"
  | "deleting"
  | "delete_failed"
  | "unknown";

export type DocumentImportPhase =
  | "selected"
  | "validating"
  | "copying"
  | "committing"
  | "imported"
  | "duplicate"
  | "failed"
  | "complete";

export type DocumentImportProgressStatus =
  | "in_progress"
  | "imported"
  | "duplicate"
  | "failed"
  | "complete";

export type DocumentParserExtension =
  | "pdf"
  | "docx"
  | "pptx"
  | "xlsx"
  | "txt"
  | "md"
  | "csv"
  | "png"
  | "jpg"
  | "jpeg";

export type DocumentParsePhase =
  | "validating_input"
  | "parsing"
  | "retrying"
  | "validating_output"
  | "committing"
  | "completed"
  | "failed"
  | "cancelled";

export type DocumentParseStatus =
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface DocumentResponse {
  id: string;
  meetingId: string;
  originalFilename: string;
  safeExtension: string;
  detectedMediaType: string | null;
  sizeBytes: number | null;
  status: DocumentStatus;
  importedAt: string | null;
  updatedAt: string;
  failureCode: DocumentErrorCode | null;
  parserVersion: string | null;
  pageCount: number | null;
}

export interface ListDocumentsResponse {
  documents: DocumentResponse[];
}

export interface DocumentImportPolicyResponse {
  allowedExtensions: string[];
  maxFileSizeBytes: number;
  maxBatchCount: number;
  maxMeetingFileCount: number;
  maxMeetingTotalBytes: number;
}

export interface DocumentParserFormatResponse {
  extension: DocumentParserExtension;
  available: boolean;
  requiresModels: boolean;
  ocrAvailable: boolean;
}

export interface DocumentParserPolicyResponse {
  workerAvailable: boolean;
  modelsAvailable: boolean;
  busy: boolean;
  parserVersion: string | null;
  device: "cpu";
  maxInputBytes: number;
  maxPages: number;
  timeoutMs: number;
  formats: DocumentParserFormatResponse[];
}

export interface DocumentParseProgress {
  jobId: string;
  documentId: string;
  phase: DocumentParsePhase;
  status: DocumentParseStatus;
  attempt: 1 | 2;
  errorCode: DocumentErrorCode | null;
}

export interface DocumentImportItemResponse {
  documentId: string;
  originalFilename: string;
  status: "imported" | "duplicate" | "failed";
  errorCode: DocumentErrorCode | null;
}

export interface DocumentImportBatchResult {
  batchId: string;
  cancelled: boolean;
  imported: DocumentImportItemResponse[];
  duplicates: DocumentImportItemResponse[];
  failed: DocumentImportItemResponse[];
}

export interface DocumentImportProgress {
  batchId: string;
  fileIndex: number;
  totalFiles: number;
  documentId: string | null;
  originalFilename: string;
  phase: DocumentImportPhase;
  bytesProcessed: number;
  totalBytes: number | null;
  status: DocumentImportProgressStatus;
  errorCode: DocumentErrorCode | null;
}

export interface DocumentDeleteResponse {
  documentId: string;
  status: "deleted" | "deleting";
  deleted: boolean;
  pending: boolean;
  retryable: boolean;
}

export type DocumentErrorCode =
  | "document_invalid_type"
  | "document_type_mismatch"
  | "document_empty"
  | "document_too_large"
  | "document_quota_exceeded"
  | "document_filename_too_long"
  | "document_import_failed"
  | "document_parse_interrupted"
  | "document_not_found"
  | "document_already_imported"
  | "document_invalid_state"
  | "document_storage_unavailable"
  | "document_integrity_failed"
  | "document_delete_pending"
  | "document_delete_failed"
  | "meeting_delete_pending"
  | "meeting_delete_failed"
  | "meeting_invalid_state"
  | "parser_unavailable"
  | "parser_environment_missing"
  | "parser_dependency_incompatible"
  | "parser_models_missing"
  | "parser_models_incompatible"
  | "parser_invalid_input"
  | "parser_unsupported_type"
  | "parser_ocr_unavailable"
  | "parser_protocol_error"
  | "parser_worker_start_failed"
  | "parser_worker_crashed"
  | "parser_timeout"
  | "parser_cancelled"
  | "parser_resource_limit"
  | "parser_page_limit_exceeded"
  | "parser_output_too_large"
  | "parser_partial_result"
  | "parser_integrity_failed"
  | "parser_failed"
  | "internal_error";

const safeMessages: Record<DocumentErrorCode, string> = {
  document_invalid_type: "This file type is not supported.",
  document_type_mismatch: "This file does not match its filename type.",
  document_empty: "This file is empty.",
  document_too_large: "This file is larger than the allowed limit.",
  document_quota_exceeded: "This meeting has reached its document limit.",
  document_filename_too_long: "This filename is too long.",
  document_import_failed: "This file could not be imported.",
  document_parse_interrupted:
    "Document parsing was interrupted and did not resume.",
  document_not_found: "The document was not found.",
  document_already_imported: "This file is already attached to the meeting.",
  document_invalid_state:
    "Documents cannot be changed in the current meeting state.",
  document_storage_unavailable: "Document storage is unavailable.",
  document_integrity_failed: "This file could not be verified safely.",
  document_delete_pending: "The document is still being deleted.",
  document_delete_failed: "The document could not be deleted safely.",
  meeting_delete_pending: "The meeting is still being deleted.",
  meeting_delete_failed: "The meeting could not be deleted safely.",
  meeting_invalid_state: "The meeting cannot be changed in its current state.",
  parser_unavailable: "The local document parser is unavailable.",
  parser_environment_missing: "The document parser environment is missing.",
  parser_dependency_incompatible:
    "The document parser dependencies are incompatible.",
  parser_models_missing: "Required local parser models are missing.",
  parser_models_incompatible: "The local parser models are incompatible.",
  parser_invalid_input: "The document parser input is invalid.",
  parser_unsupported_type: "The document type is not supported.",
  parser_ocr_unavailable: "Local OCR is unavailable.",
  parser_protocol_error: "The document parser protocol is invalid.",
  parser_worker_start_failed: "The document parser could not start.",
  parser_worker_crashed: "The document parser stopped unexpectedly.",
  parser_timeout: "The document parser timed out.",
  parser_cancelled: "Document parsing was cancelled.",
  parser_resource_limit: "The document exceeded a parser resource limit.",
  parser_page_limit_exceeded: "The document exceeds the page limit.",
  parser_output_too_large: "The parser output exceeds the size limit.",
  parser_partial_result: "The parser produced only a partial result.",
  parser_integrity_failed: "A parser integrity check failed.",
  parser_failed: "The document could not be parsed.",
  internal_error: "An internal error occurred.",
};

export function documentErrorMessage(code: DocumentErrorCode): string {
  return safeMessages[code];
}

const errorCodes = new Set<DocumentErrorCode>(
  Object.keys(safeMessages) as DocumentErrorCode[],
);
const knownDocumentStatuses = new Set<DocumentStatus>([
  "importing",
  "staged",
  "import_failed",
  "parsing",
  "parsed",
  "parse_failed",
  "deleting",
  "delete_failed",
]);
const importPhases = new Set<DocumentImportPhase>([
  "selected",
  "validating",
  "copying",
  "committing",
  "imported",
  "duplicate",
  "failed",
  "complete",
]);
const progressStatuses = new Set<DocumentImportProgressStatus>([
  "in_progress",
  "imported",
  "duplicate",
  "failed",
  "complete",
]);
const itemStatuses = new Set(["imported", "duplicate", "failed"] as const);
const parserExtensions = [
  "pdf",
  "docx",
  "pptx",
  "xlsx",
  "txt",
  "md",
  "csv",
  "png",
  "jpg",
  "jpeg",
] as const satisfies readonly DocumentParserExtension[];
const parsePhases = new Set<DocumentParsePhase>([
  "validating_input",
  "parsing",
  "retrying",
  "validating_output",
  "committing",
  "completed",
  "failed",
  "cancelled",
]);
const parseStatuses = new Set<DocumentParseStatus>([
  "running",
  "completed",
  "failed",
  "cancelled",
]);
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export class DocumentApiError extends Error {
  readonly code: DocumentErrorCode;
  readonly retryable: boolean;

  constructor(code: DocumentErrorCode, retryable: boolean) {
    super(documentErrorMessage(code));
    this.name = "DocumentApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

export interface ProgressChannelHandle {
  readonly argument: unknown;
  dispose(): void;
}

export interface ProgressChannelAdapter {
  create(handler: (value: unknown) => void): ProgressChannelHandle;
}

const tauriInvokeAdapter: InvokeAdapter = {
  invoke: (command, args) => invoke<unknown>(command, args),
};

const tauriProgressChannelAdapter: ProgressChannelAdapter = {
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

export interface DocumentApi {
  selectAndImport(
    meetingId: string,
    progressHandler: (progress: DocumentImportProgress) => void,
  ): Promise<DocumentImportBatchResult>;
  list(meetingId: string): Promise<ListDocumentsResponse>;
  get(documentId: string): Promise<DocumentResponse>;
  requestDelete(documentId: string): Promise<DocumentDeleteResponse>;
  retryDelete(documentId: string): Promise<DocumentDeleteResponse>;
  getImportPolicy(): Promise<DocumentImportPolicyResponse>;
  getParserPolicy(): Promise<DocumentParserPolicyResponse>;
  parse(
    documentId: string,
    progressHandler: (progress: DocumentParseProgress) => void,
  ): Promise<DocumentResponse>;
  getParseStatus(jobId: string): Promise<DocumentParseProgress>;
  cancelParse(jobId: string): Promise<DocumentParseProgress>;
  getActiveParseProgress(documentId: string): DocumentParseProgress | null;
}

export function createDocumentApi(
  invokeAdapter: InvokeAdapter = tauriInvokeAdapter,
  channelAdapter: ProgressChannelAdapter = tauriProgressChannelAdapter,
): DocumentApi {
  const activeParseProgress = new Map<string, DocumentParseProgress>();
  return {
    async selectAndImport(meetingId, progressHandler) {
      const progressChannel = channelAdapter.create((value) => {
        progressHandler(parseProgress(value));
      });
      try {
        return await invokeAndParse(
          invokeAdapter,
          documentCommandNames.selectAndImport,
          {
            request: { meetingId },
            onProgress: progressChannel.argument,
          },
          parseBatchResult,
        );
      } finally {
        progressChannel.dispose();
      }
    },
    list: (meetingId) =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.list,
        { request: { meetingId } },
        parseList,
      ),
    get: (documentId) =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.get,
        { request: { documentId } },
        parseDocument,
      ),
    requestDelete: (documentId) =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.requestDelete,
        { request: { documentId } },
        parseDelete,
      ),
    retryDelete: (documentId) =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.retryDelete,
        { request: { documentId } },
        parseDelete,
      ),
    getImportPolicy: () =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.getImportPolicy,
        undefined,
        parsePolicy,
      ),
    getParserPolicy: () =>
      invokeAndParse(
        invokeAdapter,
        documentCommandNames.getParserPolicy,
        undefined,
        parseParserPolicy,
      ),
    parse: async (documentId, progressHandler) => {
      requireUuid(documentId);
      let jobId: string | null = null;
      let progressFailure: DocumentApiError | null = null;
      let rejectProgressFailure: (reason: DocumentApiError) => void = () =>
        undefined;
      const progressFailurePromise = new Promise<never>((_, reject) => {
        rejectProgressFailure = reject;
      });
      const progressChannel = channelAdapter.create((value) => {
        try {
          const progress = parseDocumentParseProgress(value);
          if (
            progress.documentId !== documentId ||
            (jobId !== null && progress.jobId !== jobId)
          ) {
            throw internalError();
          }
          jobId = progress.jobId;
          activeParseProgress.set(documentId, progress);
          progressHandler(progress);
        } catch {
          if (progressFailure === null) {
            progressFailure = internalError();
            rejectProgressFailure(progressFailure);
          }
        }
      });
      try {
        const invokePromise = invokeAndParse(
          invokeAdapter,
          documentCommandNames.parse,
          {
            request: { documentId },
            onProgress: progressChannel.argument,
          },
          (value) => {
            const document = parseDocument(value);
            if (
              document.id !== documentId ||
              document.status !== "parsed" ||
              document.failureCode !== null ||
              document.parserVersion === null
            ) {
              throw internalError();
            }
            return document;
          },
        );
        try {
          return await Promise.race([invokePromise, progressFailurePromise]);
        } catch (error: unknown) {
          const cancellableJobId = jobId;
          if (progressFailure !== null && cancellableJobId !== null) {
            void invokeAndParse(
              invokeAdapter,
              documentCommandNames.cancelParse,
              { request: { jobId: cancellableJobId } },
              (value) => {
                const progress = parseDocumentParseProgress(value);
                if (progress.jobId !== cancellableJobId) throw internalError();
                return progress;
              },
            ).catch(() => undefined);
          }
          throw error;
        }
      } finally {
        if (activeParseProgress.get(documentId)?.jobId === jobId) {
          activeParseProgress.delete(documentId);
        }
        progressChannel.dispose();
      }
    },
    getParseStatus: async (jobId) => {
      requireUuid(jobId);
      return await invokeAndParse(
        invokeAdapter,
        documentCommandNames.getParseStatus,
        { request: { jobId } },
        (value) => {
          const progress = parseDocumentParseProgress(value);
          if (progress.jobId !== jobId) throw internalError();
          return progress;
        },
      );
    },
    cancelParse: async (jobId) => {
      requireUuid(jobId);
      return await invokeAndParse(
        invokeAdapter,
        documentCommandNames.cancelParse,
        { request: { jobId } },
        (value) => {
          const progress = parseDocumentParseProgress(value);
          if (progress.jobId !== jobId) throw internalError();
          return progress;
        },
      );
    },
    getActiveParseProgress: (documentId) => {
      requireUuid(documentId);
      const progress = activeParseProgress.get(documentId);
      return progress === undefined ? null : { ...progress };
    },
  };
}

export const documentApi = createDocumentApi();

async function invokeAndParse<T>(
  adapter: InvokeAdapter,
  command: string,
  args: InvokeArguments | undefined,
  parse: (value: unknown) => T,
): Promise<T> {
  try {
    return parse(await adapter.invoke(command, args));
  } catch (error: unknown) {
    if (error instanceof DocumentApiError) throw error;
    throw parseDocumentError(error);
  }
}

function parseDocument(value: unknown): DocumentResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "id",
      "meetingId",
      "originalFilename",
      "safeExtension",
      "detectedMediaType",
      "sizeBytes",
      "status",
      "importedAt",
      "updatedAt",
      "failureCode",
      "parserVersion",
      "pageCount",
    ]) ||
    !isUuid(value.id) ||
    !isUuid(value.meetingId) ||
    typeof value.originalFilename !== "string" ||
    typeof value.safeExtension !== "string" ||
    !isNullableString(value.detectedMediaType) ||
    !isNullableNonNegativeNumber(value.sizeBytes) ||
    typeof value.status !== "string" ||
    !knownDocumentStatuses.has(value.status as DocumentStatus) ||
    !isNullableString(value.importedAt) ||
    typeof value.updatedAt !== "string" ||
    !isNullableErrorCode(value.failureCode) ||
    !isNullableSafeLabel(value.parserVersion) ||
    !isNullableNonNegativeNumber(value.pageCount)
  ) {
    throw internalError();
  }
  return {
    id: value.id,
    meetingId: value.meetingId,
    originalFilename: safeDisplayFilename(value.originalFilename),
    safeExtension: value.safeExtension,
    detectedMediaType: value.detectedMediaType,
    sizeBytes: value.sizeBytes,
    status: value.status as DocumentStatus,
    importedAt: value.importedAt,
    updatedAt: value.updatedAt,
    failureCode: value.failureCode as DocumentErrorCode | null,
    parserVersion: value.parserVersion,
    pageCount: value.pageCount,
  };
}

function parseList(value: unknown): ListDocumentsResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["documents"]) ||
    !Array.isArray(value.documents)
  ) {
    throw internalError();
  }
  return { documents: value.documents.map(parseDocument) };
}

function parsePolicy(value: unknown): DocumentImportPolicyResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "allowedExtensions",
      "maxFileSizeBytes",
      "maxBatchCount",
      "maxMeetingFileCount",
      "maxMeetingTotalBytes",
    ]) ||
    !Array.isArray(value.allowedExtensions) ||
    !value.allowedExtensions.every(
      (extension) => typeof extension === "string",
    ) ||
    !isNonNegativeNumber(value.maxFileSizeBytes) ||
    !isNonNegativeNumber(value.maxBatchCount) ||
    !isNonNegativeNumber(value.maxMeetingFileCount) ||
    !isNonNegativeNumber(value.maxMeetingTotalBytes)
  ) {
    throw internalError();
  }
  return {
    allowedExtensions: [...value.allowedExtensions] as string[],
    maxFileSizeBytes: value.maxFileSizeBytes,
    maxBatchCount: value.maxBatchCount,
    maxMeetingFileCount: value.maxMeetingFileCount,
    maxMeetingTotalBytes: value.maxMeetingTotalBytes,
  };
}

function parseParserPolicy(value: unknown): DocumentParserPolicyResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "workerAvailable",
      "modelsAvailable",
      "busy",
      "parserVersion",
      "device",
      "maxInputBytes",
      "maxPages",
      "timeoutMs",
      "formats",
    ]) ||
    typeof value.workerAvailable !== "boolean" ||
    typeof value.modelsAvailable !== "boolean" ||
    typeof value.busy !== "boolean" ||
    !isNullableSafeLabel(value.parserVersion) ||
    value.device !== "cpu" ||
    !isPositiveSafeInteger(value.maxInputBytes) ||
    !isPositiveSafeInteger(value.maxPages) ||
    !isPositiveSafeInteger(value.timeoutMs) ||
    !Array.isArray(value.formats) ||
    value.formats.length !== parserExtensions.length
  ) {
    throw internalError();
  }
  const formats = value.formats.map((format, index) =>
    parseParserFormat(format, parserExtensions[index]),
  );
  return {
    workerAvailable: value.workerAvailable,
    modelsAvailable: value.modelsAvailable,
    busy: value.busy,
    parserVersion: value.parserVersion,
    device: "cpu",
    maxInputBytes: value.maxInputBytes,
    maxPages: value.maxPages,
    timeoutMs: value.timeoutMs,
    formats,
  };
}

function parseParserFormat(
  value: unknown,
  expectedExtension: DocumentParserExtension | undefined,
): DocumentParserFormatResponse {
  if (
    expectedExtension === undefined ||
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "extension",
      "available",
      "requiresModels",
      "ocrAvailable",
    ]) ||
    value.extension !== expectedExtension ||
    typeof value.available !== "boolean" ||
    typeof value.requiresModels !== "boolean" ||
    typeof value.ocrAvailable !== "boolean"
  ) {
    throw internalError();
  }
  return {
    extension: expectedExtension,
    available: value.available,
    requiresModels: value.requiresModels,
    ocrAvailable: value.ocrAvailable,
  };
}

function parseDocumentParseProgress(value: unknown): DocumentParseProgress {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "jobId",
      "documentId",
      "phase",
      "status",
      "attempt",
      "errorCode",
    ]) ||
    !isUuid(value.jobId) ||
    !isUuid(value.documentId) ||
    typeof value.phase !== "string" ||
    !parsePhases.has(value.phase as DocumentParsePhase) ||
    typeof value.status !== "string" ||
    !parseStatuses.has(value.status as DocumentParseStatus) ||
    (value.attempt !== 1 && value.attempt !== 2) ||
    !isNullableErrorCode(value.errorCode)
  ) {
    throw internalError();
  }
  const phase = value.phase as DocumentParsePhase;
  const status = value.status as DocumentParseStatus;
  const errorCode = value.errorCode as DocumentErrorCode | null;
  if (!parseStateIsCoherent(phase, status, value.attempt, errorCode)) {
    throw internalError();
  }
  return {
    jobId: value.jobId,
    documentId: value.documentId,
    phase,
    status,
    attempt: value.attempt,
    errorCode,
  };
}

function parseBatchResult(value: unknown): DocumentImportBatchResult {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "batchId",
      "cancelled",
      "imported",
      "duplicates",
      "failed",
    ]) ||
    typeof value.batchId !== "string" ||
    typeof value.cancelled !== "boolean" ||
    !Array.isArray(value.imported) ||
    !Array.isArray(value.duplicates) ||
    !Array.isArray(value.failed)
  ) {
    throw internalError();
  }
  return {
    batchId: value.batchId,
    cancelled: value.cancelled,
    imported: value.imported.map(parseImportItem),
    duplicates: value.duplicates.map(parseImportItem),
    failed: value.failed.map(parseImportItem),
  };
}

function parseImportItem(value: unknown): DocumentImportItemResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "documentId",
      "originalFilename",
      "status",
      "errorCode",
    ]) ||
    typeof value.documentId !== "string" ||
    typeof value.originalFilename !== "string" ||
    typeof value.status !== "string" ||
    !itemStatuses.has(value.status as "imported" | "duplicate" | "failed") ||
    !isNullableErrorCode(value.errorCode)
  ) {
    throw internalError();
  }
  return {
    documentId: value.documentId,
    originalFilename: safeDisplayFilename(value.originalFilename),
    status: value.status as "imported" | "duplicate" | "failed",
    errorCode: value.errorCode,
  };
}

function parseProgress(value: unknown): DocumentImportProgress {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "batchId",
      "fileIndex",
      "totalFiles",
      "documentId",
      "originalFilename",
      "phase",
      "bytesProcessed",
      "totalBytes",
      "status",
      "errorCode",
    ]) ||
    typeof value.batchId !== "string" ||
    !isNonNegativeNumber(value.fileIndex) ||
    !isNonNegativeNumber(value.totalFiles) ||
    !isNullableString(value.documentId) ||
    typeof value.originalFilename !== "string" ||
    typeof value.phase !== "string" ||
    !importPhases.has(value.phase as DocumentImportPhase) ||
    !isNonNegativeNumber(value.bytesProcessed) ||
    !isNullableNonNegativeNumber(value.totalBytes) ||
    typeof value.status !== "string" ||
    !progressStatuses.has(value.status as DocumentImportProgressStatus) ||
    !isNullableErrorCode(value.errorCode)
  ) {
    throw internalError();
  }
  if (value.totalBytes !== null && value.bytesProcessed > value.totalBytes) {
    throw internalError();
  }
  return {
    batchId: value.batchId,
    fileIndex: value.fileIndex,
    totalFiles: value.totalFiles,
    documentId: value.documentId,
    originalFilename: safeDisplayFilename(value.originalFilename),
    phase: value.phase as DocumentImportPhase,
    bytesProcessed: value.bytesProcessed,
    totalBytes: value.totalBytes,
    status: value.status as DocumentImportProgressStatus,
    errorCode: value.errorCode,
  };
}

function parseDelete(value: unknown): DocumentDeleteResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "documentId",
      "status",
      "deleted",
      "pending",
      "retryable",
    ]) ||
    typeof value.documentId !== "string" ||
    (value.status !== "deleted" && value.status !== "deleting") ||
    typeof value.deleted !== "boolean" ||
    typeof value.pending !== "boolean" ||
    typeof value.retryable !== "boolean"
  ) {
    throw internalError();
  }
  return {
    documentId: value.documentId,
    status: value.status,
    deleted: value.deleted,
    pending: value.pending,
    retryable: value.retryable,
  };
}

function parseDocumentError(value: unknown): DocumentApiError {
  if (
    isRecord(value) &&
    hasOnlyKeys(value, ["code", "message", "retryable"]) &&
    typeof value.code === "string" &&
    errorCodes.has(value.code as DocumentErrorCode) &&
    typeof value.message === "string" &&
    typeof value.retryable === "boolean"
  ) {
    return new DocumentApiError(
      value.code as DocumentErrorCode,
      value.retryable,
    );
  }
  return internalError();
}

function internalError(): DocumentApiError {
  return new DocumentApiError("internal_error", false);
}

function requireUuid(value: string): void {
  if (!isUuid(value)) {
    throw new DocumentApiError("parser_invalid_input", false);
  }
}

function parseStateIsCoherent(
  phase: DocumentParsePhase,
  status: DocumentParseStatus,
  attempt: 1 | 2,
  errorCode: DocumentErrorCode | null,
): boolean {
  if (phase === "completed") {
    return status === "completed" && errorCode === null;
  }
  if (phase === "failed") {
    return status === "failed" && errorCode !== null;
  }
  if (phase === "cancelled") {
    return status === "cancelled" && errorCode === "parser_cancelled";
  }
  if (status !== "running") return false;
  if (phase === "retrying") {
    return attempt === 2 && errorCode !== null;
  }
  if (phase === "validating_input") {
    return attempt === 1 && errorCode === null;
  }
  return errorCode === null;
}

function safeDisplayFilename(value: string): string {
  const normalized = Array.from(value, (character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)
      ? "�"
      : character;
  }).join("");
  return normalized.length === 0 ? "Selected file" : normalized;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isUuid(value: unknown): value is string {
  return typeof value === "string" && uuidPattern.test(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isNonNegativeNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function isNullableNonNegativeNumber(value: unknown): value is number | null {
  return value === null || isNonNegativeNumber(value);
}

function isNullableErrorCode(
  value: unknown,
): value is DocumentErrorCode | null {
  return (
    value === null ||
    (typeof value === "string" && errorCodes.has(value as DocumentErrorCode))
  );
}

function isNullableSafeLabel(value: unknown): value is string | null {
  return value === null || isSafeLabel(value);
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

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const actual = Object.keys(value);
  const allowed = new Set(keys);
  return (
    actual.length === keys.length && actual.every((key) => allowed.has(key))
  );
}
