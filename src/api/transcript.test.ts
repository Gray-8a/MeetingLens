import transcriptSource from "./transcript.ts?raw";
import type {
  InvokeAdapter,
  InvokeArguments,
  MeetingResponse,
} from "./meeting";
import {
  createTranscriptApi,
  TranscriptApiError,
  transcriptCommandNames,
  type TranscriptApi,
  type TranscriptSegmentResponse,
  type TranscriptSourceResponse,
} from "./transcript";

const meetingId = "11111111-1111-4111-8111-111111111111";
const otherMeetingId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const sourceId = "22222222-2222-4222-8222-222222222222";
const segmentId = "33333333-3333-4333-8333-333333333333";
const timestamp = "2026-08-06T00:00:00.000Z";

function meeting(overrides: Partial<MeetingResponse> = {}): MeetingResponse {
  return {
    id: meetingId,
    title: "Synthetic transcript meeting",
    status: "draft",
    primaryLanguage: "en-AU",
    selectedWindowJson: null,
    startedAt: null,
    endedAt: null,
    createdAt: timestamp,
    updatedAt: timestamp,
    deletionErrorCode: null,
    ...overrides,
  };
}

function source(
  overrides: Partial<TranscriptSourceResponse> = {},
): TranscriptSourceResponse {
  return {
    id: sourceId,
    meetingId,
    sourceType: "pasted_text",
    displayName: "Synthetic pasted transcript",
    originalFilename: null,
    language: "en-AU",
    baseStartedAt: null,
    segmentCount: 1,
    importedAt: timestamp,
    createdAt: timestamp,
    updatedAt: timestamp,
    ...overrides,
  };
}

function segment(
  overrides: Partial<TranscriptSegmentResponse> = {},
): TranscriptSegmentResponse {
  return {
    id: segmentId,
    sourceId,
    meetingId,
    ordinal: 0,
    speakerLabel: "Speaker 1",
    speakerName: null,
    startOffsetMs: 0,
    endOffsetMs: 1_000,
    originalText: "Synthetic transcript body",
    translatedText: null,
    language: "en-AU",
    confidence: null,
    isFinal: true,
    revision: 1,
    correctedAt: null,
    ...overrides,
  };
}

function completedImport(duplicate = false) {
  return {
    cancelled: false,
    duplicate,
    meeting: meeting(),
    source: source(),
    segmentCount: 1,
  };
}

const importPolicy = {
  allowedExtensions: ["txt", "srt", "vtt"],
  maxPasteBytes: 2 * 1024 * 1024,
  maxFileBytes: 10 * 1024 * 1024,
  maxSegmentsPerSource: 20_000,
  maxSegmentBytes: 64 * 1024,
};

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

describe("transcriptApi command routing", () => {
  const cases: Array<{
    command: keyof typeof transcriptCommandNames;
    run: (api: TranscriptApi) => Promise<unknown>;
    expectedArgs: InvokeArguments | undefined;
    response: unknown;
  }> = [
    {
      command: "createMeetingFromPaste",
      run: (api) =>
        api.createMeetingFromPaste({
          title: "Synthetic transcript meeting",
          primaryLanguage: "en-AU",
          pastedText: "Speaker 1: Synthetic transcript body",
          displayName: "Synthetic pasted transcript",
          language: "en-AU",
          baseStartedAt: null,
        }),
      expectedArgs: {
        request: {
          title: "Synthetic transcript meeting",
          primaryLanguage: "en-AU",
          pastedText: "Speaker 1: Synthetic transcript body",
          displayName: "Synthetic pasted transcript",
          language: "en-AU",
          baseStartedAt: null,
        },
      },
      response: completedImport(),
    },
    {
      command: "selectTranscriptAndCreateMeeting",
      run: (api) =>
        api.selectTranscriptAndCreateMeeting({
          title: "Synthetic transcript meeting",
          primaryLanguage: "en-AU",
          language: null,
        }),
      expectedArgs: {
        request: {
          title: "Synthetic transcript meeting",
          primaryLanguage: "en-AU",
          language: null,
        },
      },
      response: completedImport(),
    },
    {
      command: "addPasteToMeeting",
      run: (api) =>
        api.addPasteToMeeting({
          meetingId,
          pastedText: "Synthetic transcript body",
          displayName: "Synthetic pasted transcript",
        }),
      expectedArgs: {
        request: {
          meetingId,
          pastedText: "Synthetic transcript body",
          displayName: "Synthetic pasted transcript",
        },
      },
      response: completedImport(),
    },
    {
      command: "selectAndAddToMeeting",
      run: (api) =>
        api.selectAndAddToMeeting({ meetingId, baseStartedAt: timestamp }),
      expectedArgs: { request: { meetingId, baseStartedAt: timestamp } },
      response: completedImport(),
    },
    {
      command: "listSources",
      run: (api) => api.listSources(meetingId),
      expectedArgs: { request: { meetingId } },
      response: { sources: [source()] },
    },
    {
      command: "getSource",
      run: (api) => api.getSource(meetingId, sourceId),
      expectedArgs: { request: { meetingId, sourceId } },
      response: source(),
    },
    {
      command: "listSegments",
      run: (api) => api.listSegments(meetingId, sourceId),
      expectedArgs: { request: { meetingId, sourceId } },
      response: { segments: [segment()] },
    },
    {
      command: "updateSegment",
      run: (api) =>
        api.updateSegment({
          meetingId,
          segmentId,
          speakerName: null,
          originalText: "Synthetic corrected body",
          expectedRevision: 1,
        }),
      expectedArgs: {
        request: {
          meetingId,
          segmentId,
          speakerName: null,
          originalText: "Synthetic corrected body",
          expectedRevision: 1,
        },
      },
      response: segment({
        originalText: "Synthetic corrected body",
        revision: 2,
        correctedAt: timestamp,
      }),
    },
    {
      command: "deleteSource",
      run: (api) => api.deleteSource(meetingId, sourceId),
      expectedArgs: { request: { meetingId, sourceId } },
      response: { sourceId, deleted: true },
    },
    {
      command: "getImportPolicy",
      run: (api) => api.getImportPolicy(),
      expectedArgs: undefined,
      response: importPolicy,
    },
  ];

  it.each(cases)("uses the centralized $command command", async (testCase) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = testCase.response;

    await testCase.run(createTranscriptApi(adapter));

    expect(adapter.calls).toEqual([
      {
        command: transcriptCommandNames[testCase.command],
        args: testCase.expectedArgs,
      },
    ]);
  });

  it("preserves omitted, cleared and updated segment fields", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = segment({ revision: 2, correctedAt: timestamp });
    const api = createTranscriptApi(adapter);

    await api.updateSegment({ meetingId, segmentId, expectedRevision: 1 });
    expect(adapter.calls[0]?.args).toEqual({
      request: { meetingId, segmentId, expectedRevision: 1 },
    });

    adapter.response = segment({ revision: 3, correctedAt: timestamp });
    await api.updateSegment({
      meetingId,
      segmentId,
      speakerName: null,
      expectedRevision: 2,
    });
    expect(adapter.calls[1]?.args).toEqual({
      request: {
        meetingId,
        segmentId,
        speakerName: null,
        expectedRevision: 2,
      },
    });
  });

  it("filters unapproved caller properties before invoke", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = completedImport();
    const request = {
      title: "Synthetic transcript meeting",
      primaryLanguage: "en-AU",
      pastedText: "Synthetic body",
      displayName: "Synthetic pasted transcript",
      unapprovedLocation: "private",
    };

    await createTranscriptApi(adapter).createMeetingFromPaste(request);

    expect(adapter.calls[0]?.args).toEqual({
      request: {
        title: "Synthetic transcript meeting",
        primaryLanguage: "en-AU",
        pastedText: "Synthetic body",
        displayName: "Synthetic pasted transcript",
      },
    });
  });
});

describe("transcriptApi response and error boundary", () => {
  it("parses cancellation as a normal no-op result", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = {
      cancelled: true,
      duplicate: false,
      meeting: null,
      source: null,
      segmentCount: 0,
    };

    await expect(
      createTranscriptApi(adapter).selectTranscriptAndCreateMeeting({
        title: "Synthetic transcript meeting",
        primaryLanguage: "en-AU",
      }),
    ).resolves.toEqual(adapter.response);
  });

  it("parses a duplicate without treating it as an error", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = completedImport(true);

    await expect(
      createTranscriptApi(adapter).addPasteToMeeting({
        meetingId,
        pastedText: "Synthetic duplicate",
        displayName: "Synthetic pasted transcript",
      }),
    ).resolves.toMatchObject({ cancelled: false, duplicate: true });
  });

  it("uses stable error codes and ignores backend wording", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.error = {
      code: "transcript_revision_conflict",
      message: "private backend wording",
      retryable: false,
    };

    await expect(
      createTranscriptApi(adapter).updateSegment({
        meetingId,
        segmentId,
        speakerName: "Synthetic speaker",
        expectedRevision: 1,
      }),
    ).rejects.toEqual(
      new TranscriptApiError("transcript_revision_conflict", false),
    );
  });

  it("converts unknown failures without exposing their contents", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.error = {
      privateLocation: "private",
      query: "private database details",
    };

    const error = await createTranscriptApi(adapter)
      .listSources(meetingId)
      .catch((reason: unknown) => reason);

    expect(error).toEqual(new TranscriptApiError("internal_error", false));
    expect(JSON.stringify(error)).not.toContain("private");
  });

  it.each([
    [
      "source private fields",
      () => ({ sources: [{ ...source(), contentSha256: "private" }] }),
    ],
    [
      "source path-like original filename",
      () => ({
        sources: [
          source({
            sourceType: "imported_txt",
            originalFilename: "C:\\Users\\private-transcript.txt",
          }),
        ],
      }),
    ],
    [
      "segment private fields",
      () => ({ segments: [{ ...segment(), createdAt: timestamp }] }),
    ],
    [
      "cross-meeting sources",
      () => ({ sources: [source({ meetingId: otherMeetingId })] }),
    ],
    [
      "backwards offsets",
      () => ({
        segments: [segment({ startOffsetMs: 2_000, endOffsetMs: 1_000 })],
      }),
    ],
    [
      "invalid confidence",
      () => ({ segments: [segment({ confidence: Number.POSITIVE_INFINITY })] }),
    ],
  ] as const)("rejects malformed %s fail-closed", async (_name, response) => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = response();
    const api = createTranscriptApi(adapter);

    const operation = _name.includes("source")
      ? api.listSources(meetingId)
      : api.listSegments(meetingId, sourceId);
    await expect(operation).rejects.toEqual(
      new TranscriptApiError("internal_error", false),
    );
  });

  it("rejects inconsistent import and policy result invariants", async () => {
    const adapter = new FakeInvokeAdapter();
    const api = createTranscriptApi(adapter);

    adapter.response = {
      ...completedImport(),
      source: source({ meetingId: otherMeetingId }),
    };
    await expect(
      api.createMeetingFromPaste({
        title: "Synthetic transcript meeting",
        primaryLanguage: "en-AU",
        pastedText: "Synthetic body",
        displayName: "Synthetic pasted transcript",
      }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      ...completedImport(),
      segmentCount: 2,
    };
    await expect(
      api.createMeetingFromPaste({
        title: "Synthetic transcript meeting",
        primaryLanguage: "en-AU",
        pastedText: "Synthetic body",
        displayName: "Synthetic pasted transcript",
      }),
    ).rejects.toMatchObject({ code: "internal_error" });

    adapter.response = {
      ...importPolicy,
      allowedExtensions: ["txt", "txt", "vtt"],
    };
    await expect(api.getImportPolicy()).rejects.toMatchObject({
      code: "internal_error",
    });

    adapter.response = { ...importPolicy, maxPasteBytes: 1 };
    await expect(api.getImportPolicy()).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("returns the frozen transcript limits", async () => {
    const adapter = new FakeInvokeAdapter();
    adapter.response = importPolicy;

    await expect(
      createTranscriptApi(adapter).getImportPolicy(),
    ).resolves.toEqual(importPolicy);
  });
});

describe("transcriptApi security surface", () => {
  it("exposes exactly the approved ten methods", () => {
    expect(
      Object.keys(createTranscriptApi(new FakeInvokeAdapter())).sort(),
    ).toEqual(
      [
        "createMeetingFromPaste",
        "selectTranscriptAndCreateMeeting",
        "addPasteToMeeting",
        "selectAndAddToMeeting",
        "listSources",
        "getSource",
        "listSegments",
        "updateSegment",
        "deleteSource",
        "getImportPolicy",
      ].sort(),
    );
  });

  it("has no arbitrary-location, browser-persistence or console surface", () => {
    expect(JSON.stringify(transcriptCommandNames)).not.toMatch(
      /open|reveal|arbitrary|location|storage/i,
    );
    expect(transcriptSource).not.toMatch(
      /sourcePath|sourcePaths|storageRelativePath|contentSha256|parserName|parserVersion/,
    );
    expect(transcriptSource).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\./,
    );
  });

  it("contains no explicit any type and works with only an injected adapter", async () => {
    expect(transcriptSource).not.toMatch(/\bany\b/);
    const adapter = new FakeInvokeAdapter();
    adapter.response = { sources: [source()] };

    await expect(
      createTranscriptApi(adapter).listSources(meetingId),
    ).resolves.toEqual(adapter.response);
  });
});
