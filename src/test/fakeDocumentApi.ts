import {
  DocumentApiError,
  type DocumentApi,
  type DocumentDeleteResponse,
  type DocumentImportBatchResult,
  type DocumentImportPolicyResponse,
  type DocumentImportProgress,
  type DocumentParseProgress,
  type DocumentParserPolicyResponse,
  type DocumentResponse,
  type ListDocumentsResponse,
} from "../api/document";

export const documentIds = {
  staged: "55555555-5555-4555-8555-555555555555",
  failed: "66666666-6666-4666-8666-666666666666",
  parsed: "77777777-7777-4777-8777-777777777777",
};

export const documentParseJobId = "88888888-8888-4888-8888-888888888888";

export function syntheticDocument(
  overrides: Partial<DocumentResponse> = {},
): DocumentResponse {
  return {
    id: documentIds.staged,
    meetingId: "11111111-1111-4111-8111-111111111111",
    originalFilename: "synthetic-notes.txt",
    safeExtension: "txt",
    detectedMediaType: "text/plain",
    sizeBytes: 128,
    status: "staged",
    importedAt: "2026-07-31T00:00:00.000Z",
    updatedAt: "2026-07-31T00:00:00.000Z",
    failureCode: null,
    parserVersion: null,
    pageCount: null,
    ...overrides,
  };
}

export function syntheticDocumentParseProgress(
  overrides: Partial<DocumentParseProgress> = {},
): DocumentParseProgress {
  return {
    jobId: documentParseJobId,
    documentId: documentIds.staged,
    phase: "parsing",
    status: "running",
    attempt: 1,
    errorCode: null,
    ...overrides,
  };
}

export class FakeDocumentApi implements DocumentApi {
  documents: DocumentResponse[];
  activeParseProgress = new Map<string, DocumentParseProgress>();
  selectCalls: string[] = [];
  listCalls: string[] = [];
  getCalls: string[] = [];
  requestDeleteCalls: string[] = [];
  retryDeleteCalls: string[] = [];
  getParserPolicyCalls = 0;
  parseCalls: string[] = [];
  getParseStatusCalls: string[] = [];
  cancelParseCalls: string[] = [];
  listError: unknown = null;
  selectError: unknown = null;
  deleteError: unknown = null;
  retryError: unknown = null;
  parserPolicyError: unknown = null;
  parseError: unknown = null;
  parseStatusError: unknown = null;
  cancelParseError: unknown = null;
  selectPromise: Promise<DocumentImportBatchResult> | null = null;
  parsePromise: Promise<DocumentResponse> | null = null;
  progressHandler: ((progress: DocumentImportProgress) => void) | null = null;
  parseProgressHandler: ((progress: DocumentParseProgress) => void) | null =
    null;
  progressEvents: DocumentImportProgress[] = [];
  parseProgressEvents: DocumentParseProgress[] = [];
  selectResult: DocumentImportBatchResult = {
    batchId: "77777777-7777-4777-8777-777777777777",
    cancelled: true,
    imported: [],
    duplicates: [],
    failed: [],
  };
  deleteResponse: DocumentDeleteResponse | null = null;
  parseResult: DocumentResponse | null = null;
  parseStatus: DocumentParseProgress = syntheticDocumentParseProgress();
  cancelParseResult: DocumentParseProgress | null = null;
  policy: DocumentImportPolicyResponse = {
    allowedExtensions: [
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
    ],
    maxFileSizeBytes: 100 * 1024 * 1024,
    maxBatchCount: 10,
    maxMeetingFileCount: 20,
    maxMeetingTotalBytes: 500 * 1024 * 1024,
  };
  parserPolicy: DocumentParserPolicyResponse = {
    workerAvailable: true,
    modelsAvailable: true,
    busy: false,
    parserVersion: "2.117.0",
    device: "cpu",
    maxInputBytes: 100 * 1024 * 1024,
    maxPages: 300,
    timeoutMs: 150_000,
    formats: [
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
    ].map((extension) => ({
      extension,
      available: true,
      requiresModels: ["pdf", "png", "jpg", "jpeg"].includes(extension),
      ocrAvailable: ["pdf", "png", "jpg", "jpeg"].includes(extension),
    })) as DocumentParserPolicyResponse["formats"],
  };

  constructor(documents: DocumentResponse[] = []) {
    this.documents = documents.map(cloneDocument);
  }

  async selectAndImport(
    meetingId: string,
    progressHandler: (progress: DocumentImportProgress) => void,
  ): Promise<DocumentImportBatchResult> {
    this.selectCalls.push(meetingId);
    if (this.selectError !== null) throw this.selectError;
    this.progressHandler = progressHandler;
    for (const event of this.progressEvents) progressHandler({ ...event });
    const result = this.selectPromise
      ? await this.selectPromise
      : this.selectResult;
    this.progressHandler = null;
    return structuredClone(result);
  }

  async list(meetingId: string): Promise<ListDocumentsResponse> {
    this.listCalls.push(meetingId);
    if (this.listError !== null) throw this.listError;
    return {
      documents: this.documents
        .filter((document) => document.meetingId === meetingId)
        .map(cloneDocument),
    };
  }

  async get(documentId: string): Promise<DocumentResponse> {
    this.getCalls.push(documentId);
    const document = this.documents.find(
      (candidate) => candidate.id === documentId,
    );
    if (!document) throw new DocumentApiError("document_not_found", false);
    return cloneDocument(document);
  }

  async requestDelete(documentId: string): Promise<DocumentDeleteResponse> {
    this.requestDeleteCalls.push(documentId);
    if (this.deleteError !== null) throw this.deleteError;
    const response = this.deleteResponse ?? {
      documentId,
      status: "deleted",
      deleted: true,
      pending: false,
      retryable: false,
    };
    if (response.deleted) {
      this.documents = this.documents.filter(
        (document) => document.id !== documentId,
      );
    }
    return response;
  }

  async retryDelete(documentId: string): Promise<DocumentDeleteResponse> {
    this.retryDeleteCalls.push(documentId);
    if (this.retryError !== null) throw this.retryError;
    this.documents = this.documents.filter(
      (document) => document.id !== documentId,
    );
    return {
      documentId,
      status: "deleted",
      deleted: true,
      pending: false,
      retryable: false,
    };
  }

  async getImportPolicy(): Promise<DocumentImportPolicyResponse> {
    return {
      ...this.policy,
      allowedExtensions: [...this.policy.allowedExtensions],
    };
  }

  async getParserPolicy(): Promise<DocumentParserPolicyResponse> {
    this.getParserPolicyCalls += 1;
    if (this.parserPolicyError !== null) throw this.parserPolicyError;
    return cloneParserPolicy(this.parserPolicy);
  }

  async parse(
    documentId: string,
    progressHandler: (progress: DocumentParseProgress) => void,
  ): Promise<DocumentResponse> {
    this.parseCalls.push(documentId);
    if (this.parseError !== null) throw this.parseError;
    this.documents = this.documents.map((document) =>
      document.id === documentId
        ? { ...document, status: "parsing", failureCode: null }
        : document,
    );
    this.parseProgressHandler = progressHandler;
    try {
      for (const event of this.parseProgressEvents) {
        this.parseStatus = { ...event };
        this.activeParseProgress.set(documentId, { ...event });
        progressHandler({ ...event });
      }
      const result = this.parsePromise
        ? await this.parsePromise
        : (this.parseResult ?? this.defaultParsedDocument(documentId));
      const cloned = cloneDocument(result);
      const index = this.documents.findIndex(
        (document) => document.id === cloned.id,
      );
      if (index >= 0) this.documents[index] = cloned;
      else this.documents.push(cloned);
      return cloneDocument(cloned);
    } finally {
      this.parseProgressHandler = null;
      this.activeParseProgress.delete(documentId);
    }
  }

  async getParseStatus(jobId: string): Promise<DocumentParseProgress> {
    this.getParseStatusCalls.push(jobId);
    if (this.parseStatusError !== null) throw this.parseStatusError;
    if (this.parseStatus.jobId !== jobId) {
      throw new DocumentApiError("document_not_found", false);
    }
    return { ...this.parseStatus };
  }

  async cancelParse(jobId: string): Promise<DocumentParseProgress> {
    this.cancelParseCalls.push(jobId);
    if (this.cancelParseError !== null) throw this.cancelParseError;
    if (this.parseStatus.jobId !== jobId) {
      throw new DocumentApiError("document_not_found", false);
    }
    const cancelled = this.cancelParseResult ?? {
      ...this.parseStatus,
      phase: "cancelled",
      status: "cancelled",
      errorCode: "parser_cancelled",
    };
    this.parseStatus = { ...cancelled };
    this.activeParseProgress.set(cancelled.documentId, { ...cancelled });
    if (cancelled.status === "cancelled") {
      this.documents = this.documents.map((document) =>
        document.id === cancelled.documentId
          ? { ...document, status: "staged", failureCode: null }
          : document,
      );
    }
    return { ...cancelled };
  }

  getActiveParseProgress(documentId: string): DocumentParseProgress | null {
    const progress = this.activeParseProgress.get(documentId);
    return progress === undefined ? null : { ...progress };
  }

  private defaultParsedDocument(documentId: string): DocumentResponse {
    const existing = this.documents.find(
      (document) => document.id === documentId,
    );
    return syntheticDocument({
      ...existing,
      id: documentId,
      status: "parsed",
      failureCode: null,
      parserVersion: "2.117.0",
      pageCount: 1,
      updatedAt: "2026-08-26T00:00:00.000Z",
    });
  }
}

function cloneDocument(document: DocumentResponse): DocumentResponse {
  return { ...document };
}

function cloneParserPolicy(
  policy: DocumentParserPolicyResponse,
): DocumentParserPolicyResponse {
  return {
    ...policy,
    formats: policy.formats.map((format) => ({ ...format })),
  };
}
