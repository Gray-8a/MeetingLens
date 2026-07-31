import {
  DocumentApiError,
  type DocumentApi,
  type DocumentDeleteResponse,
  type DocumentImportBatchResult,
  type DocumentImportPolicyResponse,
  type DocumentImportProgress,
  type DocumentResponse,
  type ListDocumentsResponse,
} from "../api/document";

export const documentIds = {
  staged: "55555555-5555-4555-8555-555555555555",
  failed: "66666666-6666-4666-8666-666666666666",
};

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

export class FakeDocumentApi implements DocumentApi {
  documents: DocumentResponse[];
  selectCalls: string[] = [];
  listCalls: string[] = [];
  getCalls: string[] = [];
  requestDeleteCalls: string[] = [];
  retryDeleteCalls: string[] = [];
  listError: unknown = null;
  selectError: unknown = null;
  deleteError: unknown = null;
  retryError: unknown = null;
  selectPromise: Promise<DocumentImportBatchResult> | null = null;
  progressHandler: ((progress: DocumentImportProgress) => void) | null = null;
  progressEvents: DocumentImportProgress[] = [];
  selectResult: DocumentImportBatchResult = {
    batchId: "77777777-7777-4777-8777-777777777777",
    cancelled: true,
    imported: [],
    duplicates: [],
    failed: [],
  };
  deleteResponse: DocumentDeleteResponse | null = null;
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
}

function cloneDocument(document: DocumentResponse): DocumentResponse {
  return { ...document };
}
