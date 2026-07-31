import { Channel, invoke } from "@tauri-apps/api/core";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

export const documentCommandNames = {
  selectAndImport: "select_and_import_documents",
  list: "list_documents",
  get: "get_document",
  requestDelete: "request_delete_document",
  retryDelete: "retry_delete_document",
  getImportPolicy: "get_document_import_policy",
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
  failureCode: string | null;
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
  | "internal_error";

const safeMessages: Record<DocumentErrorCode, string> = {
  document_invalid_type: "This file type is not supported.",
  document_type_mismatch: "This file does not match its filename type.",
  document_empty: "This file is empty.",
  document_too_large: "This file is larger than the allowed limit.",
  document_quota_exceeded: "This meeting has reached its document limit.",
  document_filename_too_long: "This filename is too long.",
  document_import_failed: "This file could not be imported.",
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
}

export function createDocumentApi(
  invokeAdapter: InvokeAdapter = tauriInvokeAdapter,
  channelAdapter: ProgressChannelAdapter = tauriProgressChannelAdapter,
): DocumentApi {
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
    typeof value.id !== "string" ||
    typeof value.meetingId !== "string" ||
    typeof value.originalFilename !== "string" ||
    typeof value.safeExtension !== "string" ||
    !isNullableString(value.detectedMediaType) ||
    !isNullableNonNegativeNumber(value.sizeBytes) ||
    typeof value.status !== "string" ||
    !isNullableString(value.importedAt) ||
    typeof value.updatedAt !== "string" ||
    !isNullableString(value.failureCode) ||
    !isNullableString(value.parserVersion) ||
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
    status: knownDocumentStatuses.has(value.status as DocumentStatus)
      ? (value.status as DocumentStatus)
      : "unknown",
    importedAt: value.importedAt,
    updatedAt: value.updatedAt,
    failureCode: value.failureCode,
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

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isNonNegativeNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
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

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const allowed = new Set(keys);
  return Object.keys(value).every((key) => allowed.has(key));
}
