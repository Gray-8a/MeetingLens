import providerBaseSource from "./mocks/base.ts?raw";
import captureSource from "./mocks/MockCaptureProvider.ts?raw";
import documentSource from "./mocks/MockDocumentParserProvider.ts?raw";
import summarySource from "./mocks/MockSummaryProvider.ts?raw";
import transcriptionSource from "./mocks/MockTranscriptionProvider.ts?raw";
import translationSource from "./mocks/MockTranslationProvider.ts?raw";
import { ProviderError, toProviderError } from "./errors";
import { MockCaptureProvider } from "./mocks/MockCaptureProvider";
import { MockDocumentParserProvider } from "./mocks/MockDocumentParserProvider";
import { MockSummaryProvider } from "./mocks/MockSummaryProvider";
import { MockTranscriptionProvider } from "./mocks/MockTranscriptionProvider";
import { MockTranslationProvider } from "./mocks/MockTranslationProvider";
import { ProviderRegistry } from "./runtime";
import { providerStatusLabel, safeProviderStatus } from "./types";

const transcriptionRequest = {
  meetingId: "11111111-1111-4111-8111-111111111111",
  language: "en-AU",
  startedAt: "2026-07-30T01:02:00.000Z",
};

const translationRequest = {
  segmentId: "mock-segment-a",
  sourceText: "Synthetic source text",
  sourceLanguage: "en-AU",
  targetLanguage: "zh-CN",
};

const summaryRequest = {
  requestId: "mock-summary-a",
  meeting: {
    meetingId: transcriptionRequest.meetingId,
    title: "Synthetic provider review",
    primaryLanguage: "en-AU",
  },
  transcriptSegments: [],
  documentReferences: [],
  screenContext: [],
  manualNotes: "Synthetic note",
};

const documentReference = {
  artifactId: "mock-artifact-a",
  originalFilename: "synthetic-notes.pdf",
  mimeType: "application/pdf",
  sizeBytes: 128,
};

beforeEach(() => vi.useFakeTimers());

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe("ProviderRegistry", () => {
  it("registers all five mock providers by default", () => {
    const registry = new ProviderRegistry();

    expect(registry.snapshots()).toHaveLength(5);
    expect(registry.snapshots().every(({ mode }) => mode === "mock")).toBe(
      true,
    );
  });

  it("accepts an injected replacement for isolated tests", () => {
    const transcription = new MockTranscriptionProvider();
    const registry = new ProviderRegistry({ transcription });

    expect(registry.get("transcription")).toBe(transcription);
  });

  it("replaces one provider and disposes the previous instance", async () => {
    const registry = new ProviderRegistry();
    const previous = registry.get("summary");
    const replacement = new MockSummaryProvider();

    await registry.replace("summary", replacement);

    expect(previous.getStatus()).toBe("disposed");
    expect(registry.get("summary")).toBe(replacement);
  });

  it("resets all providers between tests", async () => {
    const registry = new ProviderRegistry();
    const previous = registry.get("translation");

    await registry.reset();

    expect(previous.getStatus()).toBe("disposed");
    expect(registry.get("translation")).not.toBe(previous);
    expect(registry.get("translation").getStatus()).toBe("ready");
  });

  it("disposes every provider exactly through the owned runtime", async () => {
    const registry = new ProviderRegistry();

    await registry.disposeAll();
    await registry.disposeAll();

    expect(
      registry.snapshots().every(({ status }) => status === "disposed"),
    ).toBe(true);
  });
});

describe("MockCaptureProvider", () => {
  it("initializes without accessing a device", async () => {
    const provider = new MockCaptureProvider();

    await provider.initialize();

    expect(provider.getStatus()).toBe("ready");
  });

  it("lists explicit synthetic sources", async () => {
    const provider = new MockCaptureProvider();
    await provider.initialize();

    const sources = await provider.listSources();

    expect(sources.map(({ kind }) => kind)).toEqual([
      "microphone",
      "systemAudio",
      "window",
      "screen",
    ]);
    expect(sources.every(({ isMock }) => isMock)).toBe(true);
  });

  it("supports initialize, start, pause, resume, and stop", async () => {
    const provider = new MockCaptureProvider({
      now: () => new Date("2026-07-30T01:02:03.000Z"),
    });
    await provider.initialize();
    const session = await provider.startSession({
      meetingId: transcriptionRequest.meetingId,
      sourceIds: ["mock-microphone"],
    });
    expect(provider.getStatus()).toBe("running");
    expect(session.startedAt).toBe("2026-07-30T01:02:03.000Z");

    await provider.pauseSession();
    expect(provider.getStatus()).toBe("paused");
    await provider.resumeSession();
    expect(provider.getStatus()).toBe("running");
    await provider.stopSession();
    expect(provider.getStatus()).toBe("stopped");
  });

  it("rejects a duplicate start", async () => {
    const provider = new MockCaptureProvider();
    await provider.initialize();
    const request = {
      meetingId: transcriptionRequest.meetingId,
      sourceIds: ["mock-microphone"],
    };
    await provider.startSession(request);

    await expect(provider.startSession(request)).rejects.toMatchObject({
      code: "provider_already_running",
      provider: "capture",
      operation: "start",
    });
  });

  it("rejects pause before a session starts", async () => {
    const provider = new MockCaptureProvider();
    await provider.initialize();

    await expect(provider.pauseSession()).rejects.toMatchObject({
      code: "provider_invalid_state",
    });
  });

  it("rejects resume after the session has stopped", async () => {
    const provider = new MockCaptureProvider();
    await provider.initialize();
    await provider.startSession({
      meetingId: transcriptionRequest.meetingId,
      sourceIds: ["mock-microphone"],
    });
    await provider.stopSession();

    await expect(provider.resumeSession()).rejects.toMatchObject({
      code: "provider_invalid_state",
    });
  });

  it("uses a deterministic initialization failure without leaking details", async () => {
    const provider = new MockCaptureProvider({
      failOperations: new Set(["initialize"]),
    });

    await expect(provider.initialize()).rejects.toMatchObject({
      code: "provider_initialization_failed",
      retryable: true,
    });
    expect(provider.getStatus()).toBe("error");
  });

  it("treats disposed as a terminal state", async () => {
    const provider = new MockCaptureProvider();
    await provider.dispose();

    await expect(provider.initialize()).rejects.toMatchObject({
      code: "provider_disposed",
    });
  });
});

describe("MockTranscriptionProvider", () => {
  it("emits a provisional segment with the stable event contract", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    provider.subscribe(listener);

    await provider.start(transcriptionRequest);

    expect(listener).toHaveBeenCalledWith(
      expect.objectContaining({
        segmentId: "mock-segment-0",
        speakerLabel: "Speaker A",
        language: "en-AU",
        confidence: 0.92,
        isFinal: false,
      }),
    );
  });

  it("emits the final revision with UTC ISO timestamps", async () => {
    const provider = new MockTranscriptionProvider();
    const events: Array<{ isFinal: boolean; startedAt: string }> = [];
    provider.subscribe((event) => events.push(event));
    await provider.start(transcriptionRequest);

    vi.advanceTimersByTime(900);

    const lastEvent = events[events.length - 1];
    expect(lastEvent?.isFinal).toBe(true);
    expect(lastEvent?.startedAt).toMatch(/Z$/);
  });

  it("pauses and resumes without emitting while paused", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    provider.subscribe(listener);
    await provider.start(transcriptionRequest);
    await provider.pause();
    const pausedCount = listener.mock.calls.length;

    vi.advanceTimersByTime(5_000);
    expect(listener).toHaveBeenCalledTimes(pausedCount);

    await provider.resume();
    vi.advanceTimersByTime(900);
    expect(listener.mock.calls.length).toBeGreaterThan(pausedCount);
  });

  it("stops and clears its timer", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    provider.subscribe(listener);
    await provider.start(transcriptionRequest);
    await provider.stop();
    const stoppedCount = listener.mock.calls.length;

    vi.advanceTimersByTime(5_000);

    expect(listener).toHaveBeenCalledTimes(stoppedCount);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("does not notify an unsubscribed listener", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    const unsubscribe = provider.subscribe(listener);
    unsubscribe();

    await provider.start(transcriptionRequest);

    expect(listener).not.toHaveBeenCalled();
  });

  it("cleans listeners and timers when disposed", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    provider.subscribe(listener);
    await provider.start(transcriptionRequest);

    await provider.dispose();
    vi.advanceTimersByTime(5_000);

    expect(provider.getStatus()).toBe("disposed");
    expect(vi.getTimerCount()).toBe(0);
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("starts a clean second session after stop", async () => {
    const provider = new MockTranscriptionProvider();
    const listener = vi.fn();
    provider.subscribe(listener);
    await provider.start(transcriptionRequest);
    await provider.stop();

    await provider.start({
      ...transcriptionRequest,
      meetingId: "22222222-2222-4222-8222-222222222222",
    });

    expect(listener).toHaveBeenLastCalledWith(
      expect.objectContaining({ segmentId: "mock-segment-0" }),
    );
    expect(vi.getTimerCount()).toBe(1);
  });
});

describe("MockTranslationProvider", () => {
  it("returns an explicit deterministic mock translation", async () => {
    const provider = new MockTranslationProvider();
    const resultPromise = provider.translateSegment(translationRequest);

    await vi.runAllTimersAsync();

    await expect(resultPromise).resolves.toEqual({
      segmentId: translationRequest.segmentId,
      translatedText: "[Mock zh-CN] Synthetic source text",
      sourceLanguage: "en-AU",
      targetLanguage: "zh-CN",
      isFinal: true,
      isMock: true,
    });
  });

  it("cancels an in-flight translation", async () => {
    const provider = new MockTranslationProvider({ delayMs: 1_000 });
    const resultPromise = provider.translateSegment(translationRequest);
    const rejection = expect(resultPromise).rejects.toMatchObject({
      code: "provider_cancelled",
    });

    await provider.cancel(translationRequest.segmentId);

    await rejection;
    expect(provider.getStatus()).toBe("ready");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("supports a deterministic failure and remains in error", async () => {
    const provider = new MockTranslationProvider({
      failSegmentIds: new Set([translationRequest.segmentId]),
    });
    const resultPromise = provider.translateSegment(translationRequest);
    const rejection = expect(resultPromise).rejects.toMatchObject({
      code: "provider_operation_failed",
      retryable: true,
    });

    await vi.runAllTimersAsync();

    await rejection;
    expect(provider.getStatus()).toBe("error");
    expect(() => provider.translateSegment(translationRequest)).toThrow(
      expect.objectContaining({ code: "provider_unavailable" }),
    );
  });
});

describe("MockSummaryProvider", () => {
  it("returns a result that cannot be mistaken for formal AI output", async () => {
    const provider = new MockSummaryProvider();
    const resultPromise = provider.generate(summaryRequest);

    await vi.runAllTimersAsync();

    await expect(resultPromise).resolves.toEqual(
      expect.objectContaining({
        requestId: summaryRequest.requestId,
        isMock: true,
        overview: expect.stringContaining("Mock summary"),
      }),
    );
  });

  it("cancels an in-flight summary", async () => {
    const provider = new MockSummaryProvider({ delayMs: 1_000 });
    const resultPromise = provider.generate(summaryRequest);
    const rejection = expect(resultPromise).rejects.toMatchObject({
      code: "provider_cancelled",
    });

    await provider.cancel(summaryRequest.requestId);

    await rejection;
  });
});

describe("MockDocumentParserProvider", () => {
  it("validates only a safe document reference", async () => {
    const provider = new MockDocumentParserProvider();

    await expect(provider.validate(documentReference)).resolves.toMatchObject({
      valid: true,
    });
    await expect(
      provider.validate({
        ...documentReference,
        originalFilename: "..\\private.pdf",
      }),
    ).resolves.toMatchObject({ valid: false });
  });

  it("parses deterministic in-memory content without opening the file", async () => {
    const provider = new MockDocumentParserProvider();
    const resultPromise = provider.parse("mock-parse-a", documentReference);

    await vi.runAllTimersAsync();

    await expect(resultPromise).resolves.toEqual(
      expect.objectContaining({
        requestId: "mock-parse-a",
        isMock: true,
        metadata: { source: "mock-memory" },
      }),
    );
  });

  it("cancels an in-flight in-memory parse", async () => {
    const provider = new MockDocumentParserProvider({ delayMs: 1_000 });
    const resultPromise = provider.parse(
      "mock-parse-cancel",
      documentReference,
    );
    const rejection = expect(resultPromise).rejects.toMatchObject({
      code: "provider_cancelled",
    });

    await provider.cancel("mock-parse-cancel");

    await rejection;
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe("provider safety boundary", () => {
  it("converts unknown objects to a safe internal error", () => {
    const error = toProviderError(
      { message: "private path and token" },
      "summary",
      "generate",
    );

    expect(error).toMatchObject({
      code: "internal_error",
      provider: "summary",
      operation: "generate",
      retryable: false,
    });
    expect(error.message).not.toContain("private");
    expect(error.message).not.toContain("token");
  });

  it("uses stable messages rather than caller-supplied details", () => {
    const error = new ProviderError(
      "provider_operation_failed",
      "document-parser",
      "parse",
      true,
    );

    expect(error.message).toBe(
      "The provider could not complete that operation.",
    );
  });

  it("uses a safe fallback for unknown future status values", () => {
    expect(safeProviderStatus("future-provider-state")).toBe("unknown");
    expect(providerStatusLabel("future-provider-state")).toBe("Unknown status");
  });

  it("contains no real network, device, Tauri, or file-system calls", () => {
    const sources = [
      providerBaseSource,
      captureSource,
      transcriptionSource,
      translationSource,
      summarySource,
      documentSource,
    ].join("\n");

    expect(sources).not.toMatch(
      /\bfetch\s*\(|getUserMedia|@tauri-apps|\binvoke\s*\(|readFile|writeFile|node:fs/,
    );
  });
});
