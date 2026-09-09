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

const parseJobId = "33333333-3333-4333-8333-333333333333";

const parserPolicy = {
  workerAvailable: true,
  modelsAvailable: true,
  busy: false,
  parserVersion: "2.117.0",
  device: "cpu",
  maxInputBytes: 100 * 1024 * 1024,
  maxPages: 300,
  timeoutMs: 150_000,
  formats: [
    ["pdf", true, true, true],
    ["docx", true, false, false],
    ["pptx", true, false, false],
    ["xlsx", true, false, false],
    ["txt", true, false, false],
    ["md", true, false, false],
    ["csv", true, false, false],
    ["png", true, true, true],
    ["jpg", true, true, true],
    ["jpeg", true, true, true],
  ].map(([extension, available, requiresModels, ocrAvailable]) => ({
    extension,
    available,
    requiresModels,
    ocrAvailable,
  })),
};

const parseProgress = {
  jobId: parseJobId,
  documentId: documentDto.id,
  phase: "parsing",
  status: "running",
  attempt: 1,
  errorCode: null,
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

  it("parses the fixed local parser policy through the approved command", async () => {
    const invoke = new FakeInvokeAdapter();
    invoke.response = parserPolicy;
    const api = createDocumentApi(invoke, new FakeProgressAdapter());

    await expect(api.getParserPolicy()).resolves.toEqual(parserPolicy);
    expect(invoke.calls).toEqual([
      {
        command: documentCommandNames.getParserPolicy,
        args: undefined,
      },
    ]);
  });

  it("parses by document ID and routes status and cancellation by job ID", async () => {
    const invoke = new FakeInvokeAdapter();
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    const received: unknown[] = [];
    const parsedDocument = {
      ...documentDto,
      status: "parsed",
      parserVersion: "2.117.0",
      pageCount: 1,
    };
    invoke.response = parsedDocument;
    invoke.beforeResolve = () => progress.handler?.(parseProgress);

    await expect(
      api.parse(documentDto.id, (event) => received.push(event)),
    ).resolves.toEqual(parsedDocument);
    expect(invoke.calls[0]).toEqual({
      command: documentCommandNames.parse,
      args: {
        request: { documentId: documentDto.id },
        onProgress: progress.argument,
      },
    });
    expect(received).toEqual([parseProgress]);
    expect(progress.disposed).toBe(true);

    invoke.beforeResolve = null;
    invoke.response = {
      ...parseProgress,
      phase: "retrying",
      attempt: 2,
      errorCode: "parser_worker_crashed",
    };
    await expect(api.getParseStatus(parseJobId)).resolves.toEqual(
      invoke.response,
    );
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: documentCommandNames.getParseStatus,
      args: { request: { jobId: parseJobId } },
    });

    invoke.response = {
      ...parseProgress,
      phase: "cancelled",
      status: "cancelled",
      errorCode: "parser_cancelled",
    };
    await expect(api.cancelParse(parseJobId)).resolves.toEqual(invoke.response);
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: documentCommandNames.cancelParse,
      args: { request: { jobId: parseJobId } },
    });
    expect(JSON.stringify(invoke.calls)).not.toMatch(
      /(?:source|storage|artifact|file)?path|sha(?:256)?|manifest|chunks?|content|text/i,
    );
  });

  it("keeps only safe active parse progress available across page remounts", async () => {
    let resolveInvoke: (value: unknown) => void = () => undefined;
    const pending = new Promise<unknown>((resolve) => {
      resolveInvoke = resolve;
    });
    const invoke: InvokeAdapter = {
      invoke: () => pending,
    };
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    const parsedDocument = {
      ...documentDto,
      status: "parsed",
      parserVersion: "2.117.0",
      pageCount: 1,
    };

    const parsePromise = api.parse(documentDto.id, () => undefined);
    progress.handler?.(parseProgress);
    expect(api.getActiveParseProgress(documentDto.id)).toEqual(parseProgress);

    resolveInvoke(parsedDocument);
    await expect(parsePromise).resolves.toEqual(parsedDocument);
    expect(api.getActiveParseProgress(documentDto.id)).toBeNull();
    expect(progress.disposed).toBe(true);
  });

  it("rejects an asynchronous malformed channel event and cancels the known job", async () => {
    let resolveParse: (value: unknown) => void = () => undefined;
    const pendingParse = new Promise<unknown>((resolve) => {
      resolveParse = resolve;
    });
    const calls: string[] = [];
    const invoke: InvokeAdapter = {
      invoke: (command) => {
        calls.push(command);
        if (command === documentCommandNames.cancelParse) {
          return Promise.resolve({
            ...parseProgress,
            phase: "cancelled",
            status: "cancelled",
            errorCode: "parser_cancelled",
          });
        }
        return pendingParse;
      },
    };
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    const parsePromise = api.parse(documentDto.id, () => undefined);

    progress.handler?.(parseProgress);
    await Promise.resolve();
    progress.handler?.({ ...parseProgress, text: "untrusted body" });

    await expect(parsePromise).rejects.toMatchObject({
      code: "internal_error",
    });
    expect(calls).toEqual([
      documentCommandNames.parse,
      documentCommandNames.cancelParse,
    ]);
    expect(progress.disposed).toBe(true);
    resolveParse({
      ...documentDto,
      status: "parsed",
      parserVersion: "2.117.0",
    });
    await Promise.resolve();
  });

  it("rejects non-terminal or unprovenanced parse success DTOs", async () => {
    const invalidResults = [
      documentDto,
      {
        ...documentDto,
        status: "parsed",
        failureCode: "parser_failed",
        parserVersion: "2.117.0",
      },
      { ...documentDto, status: "parsed", parserVersion: null },
    ];

    for (const response of invalidResults) {
      const invoke = new FakeInvokeAdapter();
      invoke.response = response;
      const api = createDocumentApi(invoke, new FakeProgressAdapter());
      await expect(
        api.parse(documentDto.id, () => undefined),
      ).rejects.toMatchObject({ code: "internal_error" });
    }
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

  it.each([
    "parser_unavailable",
    "parser_environment_missing",
    "parser_dependency_incompatible",
    "parser_models_missing",
    "parser_models_incompatible",
    "parser_invalid_input",
    "parser_unsupported_type",
    "parser_ocr_unavailable",
    "parser_protocol_error",
    "parser_worker_start_failed",
    "parser_worker_crashed",
    "parser_timeout",
    "parser_cancelled",
    "parser_resource_limit",
    "parser_page_limit_exceeded",
    "parser_output_too_large",
    "parser_partial_result",
    "parser_integrity_failed",
    "parser_failed",
  ] as const)("accepts the stable %s backend error", async (code) => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());
    invoke.error = {
      code,
      message: "private backend wording is ignored",
      retryable: true,
    };

    await expect(api.getParserPolicy()).rejects.toMatchObject({ code });
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
    await expect(api.get(documentDto.id)).rejects.toMatchObject({
      code: "internal_error",
    });
    invoke.response = { ...documentDto, storageRelativePath: "private" };
    await expect(api.get(documentDto.id)).rejects.toMatchObject({
      code: "internal_error",
    });
    invoke.response = {
      ...documentDto,
      status: "parse_failed",
      failureCode: "parser_timeout",
    };
    await expect(api.get(documentDto.id)).resolves.toMatchObject({
      status: "parse_failed",
      failureCode: "parser_timeout",
    });
    invoke.response = { ...documentDto, failureCode: "future_error" };
    await expect(api.get(documentDto.id)).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("keeps the document list readable after Rust recovers an interrupted parse", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());
    const interrupted = {
      ...documentDto,
      status: "parse_failed",
      failureCode: "document_parse_interrupted",
    };
    invoke.response = { documents: [interrupted] };
    await expect(api.list(documentDto.meetingId)).resolves.toEqual({
      documents: [interrupted],
    });
    invoke.response = interrupted;
    await expect(api.get(documentDto.id)).resolves.toEqual(interrupted);
  });

  it("fails closed for malformed parser policy and capability payloads", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());
    const invalidPolicies = [
      { ...parserPolicy, device: "gpu" },
      { ...parserPolicy, maxPages: 0 },
      { ...parserPolicy, parserVersion: "2.117.0\u0000private" },
      { ...parserPolicy, modelsPath: "C:\\private\\models" },
      {
        ...parserPolicy,
        formats: parserPolicy.formats.slice(1),
      },
      {
        ...parserPolicy,
        formats: parserPolicy.formats.map((format, index) =>
          index === 0 ? { ...format, extension: "txt" } : format,
        ),
      },
      {
        ...parserPolicy,
        formats: parserPolicy.formats.map((format, index) =>
          index === 0
            ? { ...format, manifest: "private worker details" }
            : format,
        ),
      },
    ];

    for (const policy of invalidPolicies) {
      invoke.response = policy;
      await expect(api.getParserPolicy()).rejects.toMatchObject({
        code: "internal_error",
      });
    }
  });

  it("fails closed for malformed parse progress and leaked parser data", async () => {
    const invalidProgress = [
      { ...parseProgress, phase: "future_phase" },
      { ...parseProgress, status: "failed" },
      { ...parseProgress, attempt: 3 },
      { ...parseProgress, errorCode: "future_error" },
      { ...parseProgress, errorCode: "parser_failed" },
      {
        ...parseProgress,
        phase: "retrying",
        attempt: 1,
        errorCode: "parser_worker_crashed",
      },
      {
        ...parseProgress,
        phase: "completed",
        status: "completed",
        errorCode: "parser_failed",
      },
      { ...parseProgress, sourcePath: "C:\\private\\document.pdf" },
      { ...parseProgress, sha256: "synthetic-hash" },
      { ...parseProgress, manifest: { parser: "private" } },
      { ...parseProgress, text: "synthetic meeting body" },
    ];

    for (const event of invalidProgress) {
      const invoke = new FakeInvokeAdapter();
      const progress = new FakeProgressAdapter();
      const api = createDocumentApi(invoke, progress);
      invoke.response = { ...documentDto, status: "parsed" };
      invoke.beforeResolve = () => progress.handler?.(event);
      await expect(
        api.parse(documentDto.id, () => undefined),
      ).rejects.toMatchObject({ code: "internal_error" });
      expect(progress.disposed).toBe(true);
    }
  });

  it("rejects cross-document progress, cross-job status and leaked parse results", async () => {
    const invoke = new FakeInvokeAdapter();
    const progress = new FakeProgressAdapter();
    const api = createDocumentApi(invoke, progress);
    invoke.response = { ...documentDto, status: "parsed" };
    invoke.beforeResolve = () =>
      progress.handler?.({
        ...parseProgress,
        documentId: "44444444-4444-4444-8444-444444444444",
      });
    await expect(
      api.parse(documentDto.id, () => undefined),
    ).rejects.toMatchObject({ code: "internal_error" });

    invoke.beforeResolve = null;
    invoke.response = {
      ...parseProgress,
      jobId: "44444444-4444-4444-8444-444444444444",
    };
    await expect(api.getParseStatus(parseJobId)).rejects.toMatchObject({
      code: "internal_error",
    });

    invoke.response = {
      ...documentDto,
      status: "parsed",
      chunks: [{ text: "synthetic meeting body" }],
    };
    await expect(
      api.parse(documentDto.id, () => undefined),
    ).rejects.toMatchObject({ code: "internal_error" });
  });

  it("rejects invalid parse and job IDs before invoking Rust", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createDocumentApi(invoke, new FakeProgressAdapter());

    await expect(
      api.parse("not-a-document-id", () => undefined),
    ).rejects.toMatchObject({ code: "parser_invalid_input" });
    await expect(api.getParseStatus("not-a-job-id")).rejects.toMatchObject({
      code: "parser_invalid_input",
    });
    await expect(api.cancelParse("not-a-job-id")).rejects.toMatchObject({
      code: "parser_invalid_input",
    });
    expect(invoke.calls).toEqual([]);
  });

  it("does not expose open, reveal, read, source-path or storage APIs", () => {
    const api = createDocumentApi(
      new FakeInvokeAdapter(),
      new FakeProgressAdapter(),
    );
    expect(Object.keys(api).sort()).toEqual(
      [
        "get",
        "getActiveParseProgress",
        "getImportPolicy",
        "getParseStatus",
        "getParserPolicy",
        "list",
        "cancelParse",
        "parse",
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
