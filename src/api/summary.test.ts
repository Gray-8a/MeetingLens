import summarySource from "./summary.ts?raw";
import type { InvokeAdapter, InvokeArguments } from "./meeting";
import {
  createSummaryApi,
  SummaryApiError,
  summaryCommandNames,
  type CreateSummaryRunRequest,
  type SummaryApi,
  type SummaryContent,
  type SummaryEvidenceResponse,
} from "./summary";

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
const timestamp = "2026-08-07T00:00:00.000Z";
const laterTimestamp = "2026-08-07T00:00:01.000Z";

const policy = {
  templates: ["detailed", "concise", "actions_decisions"],
  outputLanguages: ["zh", "en", "bilingual"],
  runPurpose: "final",
  triggerKind: "manual",
  provider: "deterministic_mock",
  maxSegments: 20_000,
  maxInputBytes: 10 * 1024 * 1024,
  maxEvidenceExcerptBytes: 1_024,
  listDefaultLimit: 50,
  listMaxLimit: 100,
  evidenceDefaultLimit: 100,
  evidenceMaxLimit: 200,
};

function content(overrides: Record<string, unknown> = {}): SummaryContent {
  return {
    schema_version: 1,
    overview: {
      text: "Synthetic overview",
      evidence_ids: [evidenceId],
    },
    decisions: [
      {
        text: "Synthetic decision",
        status: "confirmed",
        evidence_ids: [evidenceId],
      },
    ],
    action_items: [
      {
        text: "Synthetic action",
        owner: "Synthetic owner",
        due_date: null,
        status: "unconfirmed",
        evidence_ids: [evidenceId],
      },
    ],
    risks: [],
    open_questions: [],
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
    model: "meetinglens-deterministic-mock-v1",
    promptVersion: "summary-core-mock-v1",
    content: null,
    errorCode: null,
    sourceCount: 1,
    inputSegmentCount: 1,
    inputBytes: 64,
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

function evidence(
  overrides: Partial<SummaryEvidenceResponse> & Record<string, unknown> = {},
) {
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

describe("summaryApi command routing", () => {
  const createRequest: CreateSummaryRunRequest = {
    meetingId,
    sourceIds: [sourceId],
    template: "detailed",
    outputLanguage: "en",
  };
  const reference = { meetingId, summaryId };
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
      command: "createRun",
      run: (api) => api.createRun(createRequest),
      expectedArgs: { request: createRequest },
      response: queuedRun(),
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
        offset: 0,
        limit: 50,
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
    adapter.response = { items: [], offset: 4, limit: 10, totalCount: 4 };
    await api.listRuns({ meetingId, offset: 4, limit: 10 });
    expect(adapter.calls[0]?.args).toEqual({
      request: { meetingId, offset: 4, limit: 10 },
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

  it("filters unapproved caller properties before invoking Rust", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = queuedRun();
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
          template: "unknown" as CreateSummaryRunRequest["template"],
          outputLanguage: "en",
        }),
    },
    {
      name: "a negative run offset",
      run: (api: SummaryApi) => api.listRuns({ meetingId, offset: -1 }),
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
        api.listEvidence({ meetingId, summaryId, limit: 201 }),
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
    adapter.response = completedRun();
    const run = await createSummaryApi(adapter).getRun({
      meetingId,
      summaryId,
    });
    expect(run.content?.overview.evidence_ids).toEqual([evidenceId]);
    expect(run.content?.action_items[0]).toMatchObject({
      owner: "Synthetic owner",
      due_date: null,
      status: "unconfirmed",
    });

    adapter.response = {
      items: [evidence({ sourceDeleted: true })],
      nextCursor: null,
      hasMore: false,
      totalCount: 1,
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
              evidence_ids: [evidenceId],
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
              evidence_ids: [evidenceId, evidenceId],
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
                evidence_ids: [evidenceId],
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

  it("rejects malformed and cross-meeting run pages", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createSummaryApi(adapter);
    adapter.response = {
      items: [queuedRun(), queuedRun()],
      offset: 0,
      limit: 50,
      totalCount: 2,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = {
      items: [queuedRun({ meetingId: otherMeetingId })],
      offset: 0,
      limit: 50,
      totalCount: 1,
    };
    await expect(api.listRuns({ meetingId })).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = {
      items: [queuedRun()],
      offset: 1,
      limit: 50,
      totalCount: 1,
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
    { ...policy, extraLimit: 1 },
    { ...policy, templates: ["detailed", "detailed", "actions_decisions"] },
    { ...policy, outputLanguages: ["en", "zh", "bilingual"] },
    { ...policy, maxInputBytes: 1 },
  ])("rejects a drifted or expanded policy", async (response) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = response;
    await expect(createSummaryApi(adapter).getPolicy()).rejects.toEqual(
      new SummaryApiError("internal_error", false),
    );
  });
});

describe("summaryApi security surface", () => {
  it("exposes exactly the approved eight methods", () => {
    expect(
      Object.keys(createSummaryApi(new FakeInvokeAdapter())).sort(),
    ).toEqual(
      [
        "getPolicy",
        "createRun",
        "createRevision",
        "executeRun",
        "getRun",
        "listRuns",
        "cancelRun",
        "listEvidence",
      ].sort(),
    );
  });

  it("contains only the approved command names", () => {
    expect(Object.values(summaryCommandNames).sort()).toEqual(
      [
        "get_summary_policy",
        "create_summary_run",
        "create_summary_revision",
        "execute_summary_run",
        "get_summary_run",
        "list_summary_runs",
        "cancel_summary_run",
        "list_summary_evidence",
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
      /localStorage|sessionStorage|indexedDB|console\./,
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

  it("does not expose document, screen or non-transcript evidence kinds", () => {
    expect(summarySource).not.toMatch(
      /kind:\s*"(?:document|screen|manual_note|audio)"/,
    );
    expect(otherSourceId).not.toEqual(sourceId);
  });
});
