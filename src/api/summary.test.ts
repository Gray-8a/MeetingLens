import summarySource from "./summary.ts?raw";
import type { InvokeAdapter, InvokeArguments } from "./meeting";
import {
  createSummaryApi,
  SummaryApiError,
  summaryCommandNames,
  type CreateSummaryRunRequest,
  type ConfirmSummaryExternalDisclosureRequest,
  type SummaryApi,
  type SummaryContent,
  type SummaryExternalDisclosureRequest,
  type UpdateSummaryContentRequest,
} from "./summary";
import { FakeSummaryApi, syntheticSummaryRun } from "../test/fakeSummaryApi";

const meetingId = "11111111-1111-4111-8111-111111111111";
const otherMeetingId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const sourceId = "22222222-2222-4222-8222-222222222222";
const otherSourceId = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const summaryId = "33333333-3333-4333-8333-333333333333";
const revisionId = "44444444-4444-4444-8444-444444444444";
const seriesId = "55555555-5555-4555-8555-555555555555";
const evidenceId = "66666666-6666-4666-8666-666666666666";
const otherEvidenceId = "77777777-7777-4777-8777-777777777777";
const segmentId = "88888888-8888-4888-8888-888888888888";
const documentId = "99999999-9999-4999-8999-999999999998";
const otherDocumentId = "99999999-9999-4999-8999-999999999997";
const documentChunkId = "99999999-9999-4999-8999-999999999996";
const timestamp = "2026-08-07T00:00:00.000Z";
const laterTimestamp = "2026-08-07T00:00:01.000Z";
const contentVersion = "a".repeat(64);

const policy = {
  templates: ["detailed", "concise", "actions_decisions"],
  outputLanguages: ["zh", "en", "bilingual"],
  runPurpose: "final",
  triggerKind: "manual",
  provider: "deterministic_mock",
  maxSegments: 20_000,
  maxDocuments: 20,
  maxDocumentChunks: 20_000,
  maxInputBytes: 10 * 1024 * 1024,
  maxEvidenceExcerptBytes: 1_024,
  listDefaultLimit: 50,
  listMaxLimit: 100,
  evidenceDefaultLimit: 50,
  evidenceMaxLimit: 100,
};

const deepSeekPolicy = {
  ...policy,
  provider: "deepseek",
  maxInputBytes: 1024 * 1024,
};

const deepSeekStatus = {
  keyConfigured: true,
  provider: "deepseek",
  endpointHost: "api.deepseek.com",
  model: "deepseek-v4-pro",
  consentRemembered: false,
  consentedAt: null,
  policyVersion: "deepseek-summary-input-external-v2",
  executionAvailable: true,
};

const disclosure = {
  provider: "deepseek",
  endpointHost: "api.deepseek.com",
  model: "deepseek-v4-pro",
  sourceCount: 1,
  segmentCount: 2,
  inputBytes: 128,
  documentCount: 0,
  documentChunkCount: 0,
  documentInputBytes: 0,
  template: "detailed",
  outputLanguage: "en",
  dataCategories: [
    "transcript_text",
    "speaker_labels_and_names",
    "time_offsets",
    "document_text",
    "document_structure_locations",
    "evidence_ids",
    "summary_template",
    "output_language",
  ],
  rememberedConsentValid: false,
  requiresConfirmation: true,
  policyVersion: "deepseek-summary-input-external-v2",
};

function content(overrides: Record<string, unknown> = {}): SummaryContent {
  return {
    schemaVersion: 1,
    overview: {
      text: "Synthetic overview",
      evidenceIds: [evidenceId],
    },
    decisions: [
      {
        text: "Synthetic decision",
        status: "confirmed",
        evidenceIds: [evidenceId],
      },
    ],
    actionItems: [
      {
        text: "Synthetic action",
        owner: "Synthetic owner",
        dueDate: null,
        status: "unconfirmed",
        evidenceIds: [evidenceId],
      },
    ],
    risks: [],
    openQuestions: [],
    ...overrides,
  } as SummaryContent;
}

function queuedRun(overrides: Record<string, unknown> = {}) {
  return {
    id: summaryId,
    meetingId,
    seriesId,
    previousSummaryId: null,
    revision: 1,
    runPurpose: "final",
    triggerKind: "manual",
    template: "detailed",
    outputLanguage: "en",
    status: "queued",
    provider: "deterministic_mock",
    providerMode: "mock",
    model: "meetinglens-deterministic-mock-v1",
    promptVersion: "summary-core-mock-v1",
    content: null,
    contentVersion: null,
    errorCode: null,
    sourceCount: 1,
    inputSegmentCount: 1,
    inputBytes: 64,
    documentCount: 0,
    inputDocumentChunkCount: 0,
    documentInputBytes: 0,
    isStale: false,
    staleReason: null,
    staleAt: null,
    isUserEdited: false,
    snapshotCreatedAt: timestamp,
    createdAt: timestamp,
    startedAt: null,
    finishedAt: null,
    cancelRequestedAt: null,
    updatedAt: timestamp,
    ...overrides,
  };
}

function completedRun(overrides: Record<string, unknown> = {}) {
  return queuedRun({
    status: "completed",
    content: content(),
    contentVersion,
    startedAt: timestamp,
    finishedAt: laterTimestamp,
    updatedAt: laterTimestamp,
    ...overrides,
  });
}

function cancelledRun(overrides: Record<string, unknown> = {}) {
  return queuedRun({
    status: "cancelled",
    finishedAt: laterTimestamp,
    cancelRequestedAt: laterTimestamp,
    updatedAt: laterTimestamp,
    ...overrides,
  });
}

function failedRun(overrides: Record<string, unknown> = {}) {
  return queuedRun({
    status: "failed",
    errorCode: "summary_interrupted",
    startedAt: timestamp,
    finishedAt: laterTimestamp,
    updatedAt: laterTimestamp,
    ...overrides,
  });
}

function revisionRun(overrides: Record<string, unknown> = {}) {
  return queuedRun({
    id: revisionId,
    previousSummaryId: summaryId,
    revision: 2,
    createdAt: laterTimestamp,
    snapshotCreatedAt: laterTimestamp,
    updatedAt: laterTimestamp,
    ...overrides,
  });
}

function evidence(overrides: Record<string, unknown> = {}) {
  return {
    id: evidenceId,
    summaryId,
    meetingId,
    kind: "transcript",
    sourceId,
    transcriptSegmentId: segmentId,
    segmentRevision: 1,
    label: "Transcript segment 1",
    speakerSnapshot: "Synthetic speaker",
    startOffsetMs: 0,
    endOffsetMs: 1_000,
    ordinal: 0,
    globalOrdinal: 0,
    excerpt: "Synthetic transcript excerpt",
    sourceDeleted: false,
    createdAt: timestamp,
    ...overrides,
  };
}

function documentEvidence(overrides: Record<string, unknown> = {}) {
  return {
    id: otherEvidenceId,
    summaryId,
    meetingId,
    kind: "document",
    documentId,
    documentChunkId,
    label: "Reference document 1, chunk 1",
    parserVersion: "2.117.0",
    pageStart: 1,
    pageEnd: 2,
    sectionPath: ["Synthetic section"],
    tableOrdinal: null,
    ordinal: 0,
    globalOrdinal: 1,
    excerpt: "Synthetic document excerpt",
    sourceDeleted: false,
    createdAt: timestamp,
    ...overrides,
  };
}

class FakeInvokeAdapter implements InvokeAdapter {
  calls: Array<{ command: string; args?: InvokeArguments }> = [];
  response: unknown = null;
  error: unknown = null;

  async invoke(command: string, args?: InvokeArguments): Promise<unknown> {
    this.calls.push({ command, args });
    if (this.error !== null) throw this.error;
    return this.response;
  }
}

describe("FakeSummaryApi document contract", () => {
  it("keeps historical synthetic runs at zero document input", () => {
    expect(syntheticSummaryRun()).toMatchObject({
      documentCount: 0,
      inputDocumentChunkCount: 0,
      documentInputBytes: 0,
    });
  });

  it("clones selected document IDs and reports synthetic document totals", async () => {
    const api = new FakeSummaryApi();
    const sourceIds = [sourceId];
    const documentIds = [documentId];
    const run = await api.createRun({
      meetingId,
      sourceIds,
      documentIds,
      template: "detailed",
      outputLanguage: "en",
    });

    sourceIds[0] = otherSourceId;
    documentIds[0] = otherDocumentId;

    expect(api.createCalls[0]).toMatchObject({
      sourceIds: [sourceId],
      documentIds: [documentId],
    });
    expect(run).toMatchObject({
      documentCount: 1,
      inputDocumentChunkCount: 1,
      documentInputBytes: 64,
    });
  });
});

describe("summaryApi command routing", () => {
  const createRequest: CreateSummaryRunRequest = {
    meetingId,
    sourceIds: [sourceId],
    documentIds: [documentId],
    template: "detailed",
    outputLanguage: "en",
  };
  const reference = { meetingId, summaryId };
  const markdownReference = {
    ...reference,
    expectedContentVersion: contentVersion,
  };
  const editRequest: UpdateSummaryContentRequest = {
    ...markdownReference,
    editableContent: {
      overviewText: "Synthetic edited overview",
      decisionTexts: ["Synthetic decision"],
      actionItems: [
        {
          text: "Synthetic action",
          owner: "Synthetic owner",
          dueDate: null,
        },
      ],
      riskTexts: [],
      openQuestionTexts: [],
    },
  };
  const cases: Array<{
    command: keyof typeof summaryCommandNames;
    run: (api: SummaryApi) => Promise<unknown>;
    expectedArgs: InvokeArguments | undefined;
    response: unknown;
  }> = [
    {
      command: "getPolicy",
      run: (api) => api.getPolicy(),
      expectedArgs: undefined,
      response: policy,
    },
    {
      command: "getDeepSeekProviderStatus",
      run: (api) => api.getDeepSeekProviderStatus(),
      expectedArgs: undefined,
      response: deepSeekStatus,
    },
    {
      command: "getExternalDisclosure",
      run: (api) => api.getExternalDisclosure({ summaryId }),
      expectedArgs: { request: { summaryId } },
      response: disclosure,
    },
    {
      command: "confirmExternalDisclosure",
      run: (api) =>
        api.confirmExternalDisclosure({ summaryId, remember: true }),
      expectedArgs: { request: { summaryId, remember: true } },
      response: {
        ...disclosure,
        rememberedConsentValid: true,
        requiresConfirmation: false,
      },
    },
    {
      command: "revokeDeepSeekConsent",
      run: (api) => api.revokeDeepSeekConsent(),
      expectedArgs: undefined,
      response: deepSeekStatus,
    },
    {
      command: "createRun",
      run: (api) => api.createRun(createRequest),
      expectedArgs: { request: createRequest },
      response: queuedRun({
        documentCount: 1,
        inputDocumentChunkCount: 1,
        documentInputBytes: 64,
      }),
    },
    {
      command: "createRevision",
      run: (api) => api.createRevision(reference),
      expectedArgs: { request: reference },
      response: revisionRun(),
    },
    {
      command: "executeRun",
      run: (api) => api.executeRun(reference),
      expectedArgs: { request: reference },
      response: completedRun(),
    },
    {
      command: "getRun",
      run: (api) => api.getRun(reference),
      expectedArgs: { request: reference },
      response: queuedRun(),
    },
    {
      command: "listRuns",
      run: (api) => api.listRuns({ meetingId }),
      expectedArgs: { request: { meetingId } },
      response: {
        items: [queuedRun()],
        nextCursor: null,
        hasMore: false,
        totalCount: 1,
      },
    },
    {
      command: "cancelRun",
      run: (api) => api.cancelRun(reference),
      expectedArgs: { request: reference },
      response: cancelledRun(),
    },
    {
      command: "listEvidence",
      run: (api) => api.listEvidence(reference),
      expectedArgs: { request: reference },
      response: {
        items: [evidence()],
        nextCursor: null,
        hasMore: false,
        totalCount: 1,
      },
    },
    {
      command: "getEvidence",
      run: (api) => api.getEvidence({ ...reference, evidenceId }),
      expectedArgs: { request: { ...reference, evidenceId } },
      response: evidence(),
    },
    {
      command: "updateContent",
      run: (api) => api.updateContent(editRequest),
      expectedArgs: { request: editRequest },
      response: completedRun({ isUserEdited: true }),
    },
    {
      command: "getMarkdown",
      run: (api) => api.getMarkdown(markdownReference),
      expectedArgs: { request: markdownReference },
      response: {
        summaryId,
        contentVersion,
        markdown: "# Synthetic\n",
        bytes: 12,
      },
    },
    {
      command: "saveMarkdown",
      run: (api) => api.saveMarkdown(markdownReference),
      expectedArgs: { request: markdownReference },
      response: {
        summaryId,
        contentVersion,
        saved: true,
        cancelled: false,
        bytesWritten: 12,
      },
    },
  ];

  it.each(cases)(
    "routes $command with an exact request DTO",
    async (testCase) => {
      const adapter = new FakeInvokeAdapter();
      adapter.response = testCase.response;

      await expect(testCase.run(createSummaryApi(adapter))).resolves.toEqual(
        testCase.response,
      );
      expect(adapter.calls).toEqual([
        {
          command: summaryCommandNames[testCase.command],
          args: testCase.expectedArgs,
        },
      ]);
    },
  );

  it("serializes explicit pagination and omits null cursors", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    adapter.response = {
      items: [],
      nextCursor: null,
      hasMore: false,
      totalCount: 4,
    };
    await api.listRuns({ meetingId, cursor: summaryId, limit: 10 });
    expect(adapter.calls[0]?.args).toEqual({
      request: { meetingId, cursor: summaryId, limit: 10 },
    });

    adapter.response = {
      items: [],
      nextCursor: null,
      hasMore: false,
      totalCount: 0,
    };
    await api.listEvidence({ meetingId, summaryId, cursor: null, limit: 25 });
    expect(adapter.calls[1]?.args).toEqual({
      request: { meetingId, summaryId, limit: 25 },
    });
  });

  it("accepts ordered keyset pages and binds the next cursor to the last item", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = {
      items: [revisionRun(), queuedRun()],
      nextCursor: summaryId,
      hasMore: true,
      totalCount: 3,
    };

    await expect(
      createSummaryApi(adapter).listRuns({ meetingId, limit: 2 }),
    ).resolves.toMatchObject({
      nextCursor: summaryId,
      hasMore: true,
      totalCount: 3,
    });
    expect(adapter.calls).toEqual([
      {
        command: summaryCommandNames.listRuns,
        args: { request: { meetingId, limit: 2 } },
      },
    ]);
  });

  it("filters unapproved caller properties before invoking Rust", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = queuedRun({
      documentCount: 1,
      inputDocumentChunkCount: 1,
      documentInputBytes: 64,
    });
    const expandedCreateRequest = {
      ...createRequest,
      sourcePath: "private",
      providerResult: "private",
    };
    await createSummaryApi(adapter).createRun(expandedCreateRequest);
    expect(adapter.calls[0]?.args).toEqual({ request: createRequest });

    adapter.response = queuedRun();
    const expandedReference = {
      ...reference,
      privateLocation: "private",
    };
    await createSummaryApi(adapter).getRun(expandedReference);
    expect(adapter.calls[1]?.args).toEqual({ request: reference });
  });
});

describe("summaryApi request boundary", () => {
  it.each([
    {
      name: "an invalid meeting ID",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId: "not-a-uuid",
          sourceIds: [sourceId],
          documentIds: [],
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "an empty source selection",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [],
          documentIds: [],
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "duplicate sources",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId, sourceId],
          documentIds: [],
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "an unknown template",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId],
          documentIds: [],
          template: "unknown" as CreateSummaryRunRequest["template"],
          outputLanguage: "en",
        }),
    },
    {
      name: "an invalid document ID",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId],
          documentIds: ["not-a-uuid"],
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "a missing document selection array",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId],
          template: "detailed",
          outputLanguage: "en",
        } as unknown as CreateSummaryRunRequest),
    },
    {
      name: "duplicate documents",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId],
          documentIds: [documentId, documentId],
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "more than twenty documents",
      run: (api: SummaryApi) =>
        api.createRun({
          meetingId,
          sourceIds: [sourceId],
          documentIds: Array.from(
            { length: 21 },
            (_, index) =>
              `99999999-9999-4999-8999-${String(index).padStart(12, "0")}`,
          ),
          template: "detailed",
          outputLanguage: "en",
        }),
    },
    {
      name: "an invalid run cursor",
      run: (api: SummaryApi) =>
        api.listRuns({ meetingId, cursor: "not-a-uuid" }),
    },
    {
      name: "an excessive run limit",
      run: (api: SummaryApi) => api.listRuns({ meetingId, limit: 101 }),
    },
    {
      name: "a fractional evidence cursor",
      run: (api: SummaryApi) =>
        api.listEvidence({ meetingId, summaryId, cursor: 0.5 }),
    },
    {
      name: "an excessive evidence limit",
      run: (api: SummaryApi) =>
        api.listEvidence({ meetingId, summaryId, limit: 101 }),
    },
    {
      name: "an invalid disclosure summary ID",
      run: (api: SummaryApi) =>
        api.getExternalDisclosure({ summaryId: "not-a-uuid" }),
    },
    {
      name: "a null disclosure request",
      run: (api: SummaryApi) =>
        api.getExternalDisclosure(
          null as unknown as SummaryExternalDisclosureRequest,
        ),
    },
    {
      name: "a null confirmation request",
      run: (api: SummaryApi) =>
        api.confirmExternalDisclosure(
          null as unknown as ConfirmSummaryExternalDisclosureRequest,
        ),
    },
    {
      name: "a non-boolean remember choice",
      run: (api: SummaryApi) =>
        api.confirmExternalDisclosure({
          summaryId,
          remember: "yes" as unknown as boolean,
        }),
    },
    {
      name: "an invalid exact evidence ID",
      run: (api: SummaryApi) =>
        api.getEvidence({ meetingId, summaryId, evidenceId: "not-a-uuid" }),
    },
    {
      name: "an uppercase content version",
      run: (api: SummaryApi) =>
        api.getMarkdown({
          meetingId,
          summaryId,
          expectedContentVersion: "A".repeat(64),
        }),
    },
    {
      name: "an expanded editable content DTO",
      run: (api: SummaryApi) =>
        api.updateContent({
          meetingId,
          summaryId,
          expectedContentVersion: contentVersion,
          editableContent: {
            overviewText: "Synthetic overview",
            decisionTexts: [],
            actionItems: [],
            riskTexts: [],
            openQuestionTexts: [],
            privateText: "must-not-cross-boundary",
          } as UpdateSummaryContentRequest["editableContent"],
        }),
    },
  ])("rejects $name before invoke", async ({ run }) => {
    const adapter = new FakeInvokeAdapter();
    await expect(run(createSummaryApi(adapter))).rejects.toEqual(
      new SummaryApiError("summary_invalid_input", false),
    );
    expect(adapter.calls).toHaveLength(0);
  });
});

describe("summaryApi response and error boundary", () => {
  it("returns fully typed content and evidence DTOs", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = completedRun({
      documentCount: 1,
      inputDocumentChunkCount: 1,
      documentInputBytes: 64,
    });
    const run = await createSummaryApi(adapter).getRun({
      meetingId,
      summaryId,
    });
    expect(run.content?.overview.evidenceIds).toEqual([evidenceId]);
    expect(run.content?.actionItems[0]).toMatchObject({
      owner: "Synthetic owner",
      dueDate: null,
      status: "unconfirmed",
    });
    expect(run).toMatchObject({
      documentCount: 1,
      inputDocumentChunkCount: 1,
      documentInputBytes: 64,
    });

    adapter.response = {
      items: [
        evidence({ sourceDeleted: true }),
        documentEvidence({ sourceDeleted: true }),
      ],
      nextCursor: null,
      hasMore: false,
      totalCount: 2,
    };
    const page = await createSummaryApi(adapter).listEvidence({
      meetingId,
      summaryId,
    });
    expect(page.items[0]).toMatchObject({
      kind: "transcript",
      sourceDeleted: true,
      excerpt: "Synthetic transcript excerpt",
    });
    const documentItem = page.items[1];
    expect(documentItem).toEqual(documentEvidence({ sourceDeleted: true }));
    if (documentItem?.kind !== "document") throw new Error("Expected document");
    expect(documentItem.sectionPath).toEqual(["Synthetic section"]);
  });

  it("binds a created run to the requested document count", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = queuedRun();

    await expect(
      createSummaryApi(adapter).createRun({
        meetingId,
        sourceIds: [sourceId],
        documentIds: [documentId],
        template: "detailed",
        outputLanguage: "en",
      }),
    ).rejects.toEqual(new SummaryApiError("internal_error", false));
  });

  it("accepts the stable interrupted failure state", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = failedRun();
    await expect(
      createSummaryApi(adapter).getRun({ meetingId, summaryId }),
    ).resolves.toMatchObject({
      status: "failed",
      errorCode: "summary_interrupted",
    });
  });

  it("accepts cancellation that wins the execute completion race", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = cancelledRun({ startedAt: timestamp });
    await expect(
      createSummaryApi(adapter).executeRun({ meetingId, summaryId }),
    ).resolves.toMatchObject({
      status: "cancelled",
      startedAt: timestamp,
    });
  });

  it("requires contentVersion only for completed runs", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);

    for (const response of [
      queuedRun({ contentVersion }),
      failedRun({ contentVersion }),
      cancelledRun({ contentVersion }),
      completedRun({ contentVersion: null }),
      completedRun({ contentVersion: "A".repeat(64) }),
    ]) {
      adapter.response = response;
      await expect(api.getRun({ meetingId, summaryId })).rejects.toEqual(
        new SummaryApiError("internal_error", false),
      );
    }
  });

  it("strictly validates canonical Markdown responses", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    const request = {
      meetingId,
      summaryId,
      expectedContentVersion: contentVersion,
    };

    for (const response of [
      { summaryId, contentVersion, markdown: "# Synthetic\n", bytes: 11 },
      {
        summaryId,
        contentVersion: "b".repeat(64),
        markdown: "# Synthetic\n",
        bytes: 12,
      },
      {
        summaryId,
        contentVersion,
        markdown: "# Synthetic\n",
        bytes: 12,
        sourcePath: "must-not-cross-boundary",
      },
    ]) {
      adapter.response = response;
      await expect(api.getMarkdown(request)).rejects.toEqual(
        new SummaryApiError("internal_error", false),
      );
    }
  });

  it("strictly validates native Markdown save outcomes", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    const request = {
      meetingId,
      summaryId,
      expectedContentVersion: contentVersion,
    };

    for (const response of [
      {
        summaryId,
        contentVersion,
        saved: true,
        cancelled: true,
        bytesWritten: 12,
      },
      {
        summaryId,
        contentVersion,
        saved: false,
        cancelled: true,
        bytesWritten: 12,
      },
      {
        summaryId,
        contentVersion,
        saved: true,
        cancelled: false,
        bytesWritten: 0,
      },
    ]) {
      adapter.response = response;
      await expect(api.saveMarkdown(request)).rejects.toEqual(
        new SummaryApiError("internal_error", false),
      );
    }
  });

  it("uses stable safe errors and ignores backend wording", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.error = {
      code: "summary_provider_failed",
      message: "private backend details",
      retryable: true,
    };

    await expect(
      createSummaryApi(adapter).executeRun({ meetingId, summaryId }),
    ).rejects.toEqual(new SummaryApiError("summary_provider_failed", true));
  });

  it.each([
    {
      code: "unknown_summary_error",
      message: "private database details",
      retryable: false,
    },
    {
      code: "summary_provider_failed",
      message: "private database details",
      retryable: true,
      path: "C:\\private",
    },
    new Error("private stack details"),
  ])(
    "converts unknown or expanded failures without leaking them",
    async (failure) => {
      const adapter = new FakeInvokeAdapter();
      adapter.error = failure;
      const error = await createSummaryApi(adapter)
        .getRun({ meetingId, summaryId })
        .catch((reason: unknown) => reason);
      expect(error).toEqual(new SummaryApiError("internal_error", false));
      expect(JSON.stringify(error)).not.toContain("private");
    },
  );

  it.each([
    ["extra run fields", () => queuedRun({ snapshotBody: "private" })],
    ["a cross-run ID", () => queuedRun({ id: revisionId })],
    ["a cross-meeting ID", () => queuedRun({ meetingId: otherMeetingId })],
    ["an unknown provider", () => queuedRun({ provider: "external" })],
    ["an unknown failure code", () => failedRun({ errorCode: "private" })],
    [
      "document chunks without a document",
      () => queuedRun({ inputDocumentChunkCount: 1, documentInputBytes: 64 }),
    ],
    ["a document without frozen chunks", () => queuedRun({ documentCount: 1 })],
    [
      "more documents than frozen chunks",
      () =>
        queuedRun({
          documentCount: 2,
          inputDocumentChunkCount: 1,
          documentInputBytes: 64,
        }),
    ],
    [
      "a combined input count above the limit",
      () =>
        queuedRun({
          inputSegmentCount: 20_000,
          documentCount: 1,
          inputDocumentChunkCount: 1,
          documentInputBytes: 64,
        }),
    ],
    [
      "combined input bytes above the limit",
      () =>
        queuedRun({
          inputBytes: 10 * 1024 * 1024,
          documentCount: 1,
          inputDocumentChunkCount: 1,
          documentInputBytes: 1,
        }),
    ],
    ["queued content", () => queuedRun({ content: content() })],
    ["queued start time", () => queuedRun({ startedAt: timestamp })],
    ["completed content missing", () => completedRun({ content: null })],
    ["failed error missing", () => failedRun({ errorCode: null })],
    [
      "cancel timestamp missing",
      () => cancelledRun({ cancelRequestedAt: null }),
    ],
    ["revision lineage missing", () => queuedRun({ revision: 2 })],
    [
      "incomplete stale metadata",
      () => queuedRun({ isStale: true, staleReason: "segment_revised" }),
    ],
    [
      "extra nested content fields",
      () => completedRun({ content: content({ rawPrompt: "private" }) }),
    ],
    [
      "extra claim fields",
      () =>
        completedRun({
          content: content({
            overview: {
              text: "Synthetic overview",
              evidenceIds: [evidenceId],
              privateText: "private",
            },
          }),
        }),
    ],
    [
      "duplicate evidence references",
      () =>
        completedRun({
          content: content({
            overview: {
              text: "Synthetic overview",
              evidenceIds: [evidenceId, evidenceId],
            },
          }),
        }),
    ],
    [
      "an invalid decision status",
      () =>
        completedRun({
          content: content({
            decisions: [
              {
                text: "Synthetic decision",
                status: "accepted",
                evidenceIds: [evidenceId],
              },
            ],
          }),
        }),
    ],
  ] as const)("rejects %s fail-closed", async (_name, response) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = response();
    await expect(
      createSummaryApi(adapter).getRun({ meetingId, summaryId }),
    ).rejects.toEqual(new SummaryApiError("internal_error", false));
  });

  it("rejects cross-ID revision responses", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = revisionRun({ previousSummaryId: otherEvidenceId });
    await expect(
      createSummaryApi(adapter).createRevision({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = revisionRun({ meetingId: otherMeetingId });
    await expect(
      createSummaryApi(adapter).createRevision({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });
  });

  it("accepts historical DeepSeek v1 runs but rejects v1 for new runs and revisions", async () => {
    const deepSeekV1 = {
      provider: "deepseek",
      providerMode: "external",
      model: "deepseek-v4-pro",
      promptVersion: "summary-deepseek-v1",
    };
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);

    adapter.response = queuedRun(deepSeekV1);
    await expect(api.getRun({ meetingId, summaryId })).resolves.toMatchObject({
      promptVersion: "summary-deepseek-v1",
    });
    await expect(
      api.createRun({
        meetingId,
        sourceIds: [sourceId],
        documentIds: [],
        template: "detailed",
        outputLanguage: "en",
      }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = revisionRun(deepSeekV1);
    await expect(
      api.createRevision({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });
  });

  it("rejects malformed and cross-meeting run pages", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    adapter.response = {
      items: [queuedRun(), queuedRun()],
      nextCursor: null,
      hasMore: false,
      totalCount: 2,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = {
      items: [queuedRun({ meetingId: otherMeetingId })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = {
      items: [queuedRun()],
      nextCursor: summaryId,
      hasMore: false,
      totalCount: 1,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = {
      items: [queuedRun(), revisionRun()],
      nextCursor: null,
      hasMore: false,
      totalCount: 2,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("rejects malformed, unordered and cross-ID evidence pages", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    adapter.response = {
      items: [evidence({ privateText: "private" })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(
      api.listEvidence({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      items: [
        evidence({ globalOrdinal: 2 }),
        evidence({ id: otherEvidenceId, globalOrdinal: 1 }),
      ],
      nextCursor: null,
      hasMore: false,
      totalCount: 2,
    };
    await expect(
      api.listEvidence({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      items: [evidence({ meetingId: otherMeetingId })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(
      api.listEvidence({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      items: [evidence({ summaryId: revisionId })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(
      api.listEvidence({ meetingId, summaryId }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      items: [evidence({ globalOrdinal: 4 })],
      nextCursor: null,
      hasMore: true,
      totalCount: 2,
    };
    await expect(
      api.listEvidence({ meetingId, summaryId, cursor: 3 }),
    ).rejects.toMatchObject({ code: "internal_error" });
  });

  it("rejects evidence at or behind the requested cursor", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = {
      items: [evidence({ globalOrdinal: 3 })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(
      createSummaryApi(adapter).listEvidence({
        meetingId,
        summaryId,
        cursor: 3,
      }),
    ).rejects.toMatchObject({ code: "internal_error" });
  });

  it.each([
    ["an unknown evidence kind", () => evidence({ kind: "screen" })],
    [
      "transcript-only fields on document evidence",
      () => documentEvidence({ sourceId }),
    ],
    [
      "document-only fields on transcript evidence",
      () => evidence({ documentId }),
    ],
    [
      "an invalid document ID",
      () => documentEvidence({ documentId: "not-a-uuid" }),
    ],
    [
      "an unsafe parser version",
      () => documentEvidence({ parserVersion: "2.117.0 private" }),
    ],
    ["a partial page range", () => documentEvidence({ pageEnd: null })],
    [
      "a reversed page range",
      () => documentEvidence({ pageStart: 2, pageEnd: 1 }),
    ],
    [
      "a page outside the parser limit",
      () => documentEvidence({ pageStart: 1, pageEnd: 301 }),
    ],
    [
      "an over-deep section path",
      () =>
        documentEvidence({
          sectionPath: Array.from({ length: 17 }, () => "Section"),
        }),
    ],
    [
      "an empty section component",
      () => documentEvidence({ sectionPath: [" "] }),
    ],
    [
      "an out-of-range table ordinal",
      () => documentEvidence({ tableOrdinal: 20_000 }),
    ],
  ] as const)("rejects %s fail-closed", async (_name, response) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = {
      items: [response()],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
    };
    await expect(
      createSummaryApi(adapter).listEvidence({ meetingId, summaryId }),
    ).rejects.toEqual(new SummaryApiError("internal_error", false));
  });

  it.each([
    { ...policy, extraLimit: 1 },
    { ...policy, templates: ["detailed", "detailed", "actions_decisions"] },
    { ...policy, outputLanguages: ["en", "zh", "bilingual"] },
    { ...policy, maxDocuments: 21 },
    { ...policy, maxDocumentChunks: 20_001 },
    { ...policy, maxInputBytes: 1 },
    { ...policy, provider: "deepseek" },
    { ...deepSeekPolicy, maxInputBytes: policy.maxInputBytes },
  ])("rejects a drifted or expanded policy", async (response) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = response;
    await expect(createSummaryApi(adapter).getPolicy()).rejects.toEqual(
      new SummaryApiError("internal_error", false),
    );
  });

  it("uses the fixed provider-specific DeepSeek input limit", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = deepSeekPolicy;

    await expect(createSummaryApi(adapter).getPolicy()).resolves.toEqual(
      deepSeekPolicy,
    );
  });
});

describe("summaryApi security surface", () => {
  it("accepts historical Mock and fixed DeepSeek run identities only", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = queuedRun({
      provider: "deepseek",
      providerMode: "external",
      model: "deepseek-v4-pro",
      promptVersion: "summary-deepseek-v2",
    });
    await expect(
      createSummaryApi(adapter).getRun({ meetingId, summaryId }),
    ).resolves.toMatchObject({
      provider: "deepseek",
      providerMode: "external",
      model: "deepseek-v4-pro",
      promptVersion: "summary-deepseek-v2",
    });

    adapter.response = queuedRun({
      provider: "deepseek",
      providerMode: "external",
      model: "deepseek-v4-pro",
      promptVersion: "summary-deepseek-v1",
    });
    await expect(
      createSummaryApi(adapter).getRun({ meetingId, summaryId }),
    ).resolves.toMatchObject({ promptVersion: "summary-deepseek-v1" });

    for (const overrides of [
      { provider: "deepseek", providerMode: "mock" },
      { provider: "deepseek", model: "user-selected-model" },
      { provider: "deterministic_mock", providerMode: "external" },
    ]) {
      adapter.response = queuedRun(overrides);
      await expect(
        createSummaryApi(adapter).getRun({ meetingId, summaryId }),
      ).rejects.toEqual(new SummaryApiError("internal_error", false));
    }
  });

  it("strictly validates provider status and disclosure metadata", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);

    for (const response of [
      { ...deepSeekStatus, endpointHost: "example.invalid" },
      { ...deepSeekStatus, consentRemembered: true, consentedAt: null },
      {
        ...deepSeekStatus,
        consentRemembered: true,
        consentedAt: "2026-02-30T00:00:00.000Z",
      },
      {
        ...deepSeekStatus,
        consentRemembered: true,
        consentedAt: "2026-08-17T02:03:04Z",
      },
      {
        ...deepSeekStatus,
        policyVersion: "deepseek-transcript-external-v1",
      },
      { ...deepSeekStatus, unexpected: true },
    ]) {
      adapter.response = response;
      await expect(api.getDeepSeekProviderStatus()).rejects.toEqual(
        new SummaryApiError("internal_error", false),
      );
    }

    adapter.response = {
      ...disclosure,
      inputBytes: 192,
      documentCount: 1,
      documentChunkCount: 1,
      documentInputBytes: 64,
    };
    await expect(
      api.getExternalDisclosure({ summaryId }),
    ).resolves.toMatchObject({
      inputBytes: 192,
      documentCount: 1,
      documentChunkCount: 1,
      documentInputBytes: 64,
    });

    for (const response of [
      { ...disclosure, sourceCount: 3, segmentCount: 2 },
      { ...disclosure, inputBytes: 0 },
      { ...disclosure, inputBytes: 1024 * 1024 + 1 },
      {
        ...disclosure,
        documentChunkCount: 1,
        documentInputBytes: 64,
      },
      {
        ...disclosure,
        documentCount: 1,
        documentChunkCount: 1,
        documentInputBytes: 129,
      },
      {
        ...disclosure,
        documentCount: 1,
        documentChunkCount: 1,
        documentInputBytes: disclosure.inputBytes,
      },
      {
        ...disclosure,
        segmentCount: 20_000,
        documentCount: 1,
        documentChunkCount: 1,
        documentInputBytes: 1,
      },
      {
        ...disclosure,
        dataCategories: [...disclosure.dataCategories].reverse(),
      },
      { ...disclosure, policyVersion: "future-policy" },
      { ...disclosure, transcript: "must-not-cross-boundary" },
    ]) {
      adapter.response = response;
      await expect(api.getExternalDisclosure({ summaryId })).rejects.toEqual(
        new SummaryApiError("internal_error", false),
      );
    }
  });

  it("filters unapproved disclosure and confirmation properties", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = disclosure;
    await createSummaryApi(adapter).getExternalDisclosure({
      summaryId,
      provider: "caller-controlled",
    } as { summaryId: string; provider: string });
    expect(adapter.calls[0]?.args).toEqual({ request: { summaryId } });

    adapter.response = {
      ...disclosure,
      requiresConfirmation: false,
    };
    await createSummaryApi(adapter).confirmExternalDisclosure({
      summaryId,
      remember: false,
      model: "caller-controlled",
      endpoint: "https://example.invalid",
    } as {
      summaryId: string;
      remember: boolean;
      model: string;
      endpoint: string;
    });
    expect(adapter.calls[1]?.args).toEqual({
      request: { summaryId, remember: false },
    });
  });

  it("exposes exactly the approved sixteen methods", () => {
    expect(
      Object.keys(createSummaryApi(new FakeInvokeAdapter())).sort(),
    ).toEqual(
      [
        "getPolicy",
        "getDeepSeekProviderStatus",
        "getExternalDisclosure",
        "confirmExternalDisclosure",
        "revokeDeepSeekConsent",
        "createRun",
        "createRevision",
        "executeRun",
        "getRun",
        "listRuns",
        "cancelRun",
        "listEvidence",
        "getEvidence",
        "updateContent",
        "getMarkdown",
        "saveMarkdown",
      ].sort(),
    );
  });

  it("contains only the approved command names", () => {
    expect(Object.values(summaryCommandNames).sort()).toEqual(
      [
        "get_summary_policy",
        "get_deepseek_provider_status",
        "get_summary_external_disclosure",
        "confirm_summary_external_disclosure",
        "revoke_deepseek_consent",
        "create_summary_run",
        "create_summary_revision",
        "execute_summary_run",
        "get_summary_run",
        "list_summary_runs",
        "cancel_summary_run",
        "list_summary_evidence",
        "get_summary_evidence",
        "update_summary_content",
        "get_summary_markdown",
        "save_summary_markdown",
      ].sort(),
    );
  });

  it("has no provider-output, arbitrary-location, persistence or console surface", () => {
    expect(JSON.stringify(summaryCommandNames)).not.toMatch(
      /submit|provider_output|open|reveal|arbitrary|location|storage/i,
    );
    expect(summarySource).not.toMatch(
      /sourcePath|storageRelativePath|snapshotBody|originalText|providerResult/,
    );
    expect(summarySource).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\.|\bfetch\s*\(|XMLHttpRequest|WebSocket/,
    );
  });

  it("contains no explicit any type and works with only an injected adapter", async () => {
    expect(summarySource).not.toMatch(/\bany\b/);
    const adapter = new FakeInvokeAdapter();
    adapter.response = queuedRun();
    await expect(
      createSummaryApi(adapter).getRun({ meetingId, summaryId }),
    ).resolves.toMatchObject({ id: summaryId, meetingId });
  });

  it("exposes only transcript and document evidence kinds", () => {
    expect(summarySource).toMatch(/kind:\s*"transcript"/);
    expect(summarySource).toMatch(/kind:\s*"document"/);
    expect(summarySource).not.toMatch(/kind:\s*"(?:screen|manual_note|audio)"/);
    expect(otherSourceId).not.toEqual(sourceId);
    expect(otherDocumentId).not.toEqual(documentId);
  });
});
