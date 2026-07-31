import { describe, expect, it } from "vitest";
import {
  createDocumentApi,
  DocumentApiError,
  documentCommandNames,
  type ProgressChannelAdapter,
  type ProgressChannelHandle,
} from "./document";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

const documentDto = {
  id: "11111111-1111-4111-8111-111111111111",
  meetingId: "22222222-2222-4222-8222-222222222222",
  originalFilename: "synthetic.txt",
  safeExtension: "txt",
  detectedMediaType: "text/plain",
  sizeBytes: 9,
  status: "staged",
  importedAt: "2026-07-31T00:00:00.000Z",
  updatedAt: "2026-07-31T00:00:00.000Z",
  failureCode: null,
  parserVersion: null,
  pageCount: null,
};

class FakeInvokeAdapter implements InvokeAdapter {
  calls: Array<{ command: string; args?: InvokeArguments }> = [];
  response: unknown = null;
  error: unknown = null;
  beforeResolve: (() => void) | null = null;

  async invoke(command: string, args?: InvokeArguments): Promise<unknown> {
    this.calls.push({ command, args });
    this.beforeResolve?.();
    if (this.error !== null) throw this.error;
    return this.response;
  }
}

class FakeProgressAdapter implements ProgressChannelAdapter {
  handler: ((value: unknown) => void) | null = null;
  disposed = false;
  readonly argument = { syntheticChannel: true };

  create(handler: (value: unknown) => void): ProgressChannelHandle {
    this.handler = handler;
    return {
      argument: this.argument,
      dispose: () => {
        this.disposed = true;
        this.handler = null;
      },
    };
  }
}

describe("documentApi", () => {
  it("uses the approved commands and serializes IDs without paths", async () => {
    const invoke = new FakeInvokeAdapter();
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);

    invoke.response = { documents: [documentDto] };
    await expect(api.list(documentDto.meetingId)).resolves.toEqual({
      documents: [documentDto],
    });
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: documentCommandNames.list,
      args: { request: { meetingId: documentDto.meetingId } },
    });

    invoke.response = documentDto;
    await api.get(documentDto.id);
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: documentCommandNames.get,
      args: { request: { documentId: documentDto.id } },
    });

    invoke.response = {
      documentId: documentDto.id,
      status: "deleted",
      deleted: true,
      pending: false,
      retryable: false,
    };
    await api.requestDelete(documentDto.id);
    expect(invoke.calls[invoke.calls.length - 1]?.command).toBe(
      documentCommandNames.requestDelete,
    );
    await api.retryDelete(documentDto.id);
    expect(invoke.calls[invoke.calls.length - 1]?.command).toBe(
      documentCommandNames.retryDelete,
    );

    expect(JSON.stringify(invoke.calls)).not.toMatch(
      /sourcePath|storageRelativePath|sha256/i,
    );
  });

  it("selectAndImport sends only meetingId and a typed progress channel", async () => {
    const invoke = new FakeInvokeAdapter();
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    const received: unknown[] = [];
    invoke.response = {
      batchId: "33333333-3333-4333-8333-333333333333",
      cancelled: false,
      imported: [],
      duplicates: [],
      failed: [],
    };
    invoke.beforeResolve = () =>
      progress.handler?.({
        batchId: "33333333-3333-4333-8333-333333333333",
        fileIndex: 1,
        totalFiles: 1,
        documentId: documentDto.id,
        originalFilename: "safe\u0000name.txt",
        phase: "copying",
        bytesProcessed: 4,
        totalBytes: 9,
        status: "in_progress",
        errorCode: null,
      });

    await api.selectAndImport(documentDto.meetingId, (event) =>
      received.push(event),
    );

    expect(invoke.calls[0]).toEqual({
      command: documentCommandNames.selectAndImport,
      args: {
        request: { meetingId: documentDto.meetingId },
        onProgress: progress.argument,
      },
    });
    expect(JSON.stringify(invoke.calls[0])).not.toContain("path");
    expect(received).toMatchObject([{ originalFilename: "safe�name.txt" }]);
    expect(progress.disposed).toBe(true);
  });

  it("parses policy, success, duplicate, partial failure and cancellation", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());

    invoke.response = {
      allowedExtensions: ["pdf", "txt"],
      maxFileSizeBytes: 104857600,
      maxBatchCount: 10,
      maxMeetingFileCount: 20,
      maxMeetingTotalBytes: 524288000,
    };
    await expect(api.getImportPolicy()).resolves.toMatchObject({
      maxBatchCount: 10,
      maxMeetingFileCount: 20,
    });

    invoke.response = {
      batchId: "33333333-3333-4333-8333-333333333333",
      cancelled: false,
      imported: [
        {
          documentId: documentDto.id,
          originalFilename: "synthetic.txt",
          status: "imported",
          errorCode: null,
        },
      ],
      duplicates: [
        {
          documentId: documentDto.id,
          originalFilename: "duplicate.txt",
          status: "duplicate",
          errorCode: "document_already_imported",
        },
      ],
      failed: [
        {
          documentId: "44444444-4444-4444-8444-444444444444",
          originalFilename: "bad.pdf",
          status: "failed",
          errorCode: "document_type_mismatch",
        },
      ],
    };
    await expect(
      api.selectAndImport(documentDto.meetingId, () => undefined),
    ).resolves.toMatchObject({
      imported: [{ status: "imported" }],
      duplicates: [{ status: "duplicate" }],
      failed: [{ status: "failed" }],
    });

    invoke.response = {
      batchId: "44444444-4444-4444-8444-444444444444",
      cancelled: true,
      imported: [],
      duplicates: [],
      failed: [],
    };
    await expect(
      api.selectAndImport(documentDto.meetingId, () => undefined),
    ).resolves.toMatchObject({ cancelled: true });
  });

  it("converts stable backend failures and unknown values safely", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());

    invoke.error = {
      code: "document_quota_exceeded",
      message: "backend wording is ignored",
      retryable: false,
    };
    await expect(api.list(documentDto.meetingId)).rejects.toMatchObject({
      code: "document_quota_exceeded",
      message: "This meeting has reached its document limit.",
    });

    invoke.error = { path: "private", raw: "details" };
    await expect(api.list(documentDto.meetingId)).rejects.toEqual(
      new DocumentApiError("internal_error", false),
    );
  });

  it("fails closed for invalid progress and response payloads", async () => {
    const invoke = new FakeInvokeAdapter();
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    invoke.response = {
      batchId: "33333333-3333-4333-8333-333333333333",
      cancelled: false,
      imported: [],
      duplicates: [],
      failed: [],
    };
    invoke.beforeResolve = () =>
      progress.handler?.({
        batchId: "33333333-3333-4333-8333-333333333333",
        fileIndex: 1,
        totalFiles: 1,
        documentId: null,
        originalFilename: "synthetic.txt",
        phase: "copying",
        bytesProcessed: 10,
        totalBytes: 9,
        status: "in_progress",
        errorCode: null,
      });

    await expect(
      api.selectAndImport(documentDto.meetingId, () => undefined),
    ).rejects.toMatchObject({ code: "internal_error" });
    expect(progress.disposed).toBe(true);

    invoke.beforeResolve = () =>
      progress.handler?.({
        batchId: "33333333-3333-4333-8333-333333333333",
        fileIndex: 1,
        totalFiles: 1,
        documentId: null,
        originalFilename: "synthetic.txt",
        phase: "selected",
        bytesProcessed: 0,
        totalBytes: null,
        status: "in_progress",
        errorCode: null,
        sourcePath: "private",
      });
    await expect(
      api.selectAndImport(documentDto.meetingId, () => undefined),
    ).rejects.toMatchObject({ code: "internal_error" });

    invoke.beforeResolve = null;
    invoke.response = {
      batchId: "33333333-3333-4333-8333-333333333333",
      cancelled: false,
      imported: [],
      duplicates: [],
      failed: [],
      storageRelativePath: "private",
    };
    await expect(
      api.selectAndImport(documentDto.meetingId, () => undefined),
    ).rejects.toMatchObject({ code: "internal_error" });

    invoke.response = { ...documentDto, status: "future_status" };
    await expect(api.get(documentDto.id)).resolves.toMatchObject({
      status: "unknown",
    });
    invoke.response = { ...documentDto, storageRelativePath: "private" };
    await expect(api.get(documentDto.id)).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("does not expose open, reveal, read, source-path or storage APIs", () => {
    const api = createDocumentApi(
      new FakeInvokeAdapter(),
      new FakeProgressAdapter(),
    );
    expect(Object.keys(api).sort()).toEqual(
      [
        "get",
        "getImportPolicy",
        "list",
        "requestDelete",
        "retryDelete",
        "selectAndImport",
      ].sort(),
    );
    expect(JSON.stringify(documentCommandNames)).not.toMatch(
      /open|reveal|read|path|storage/i,
    );
  });
});
