import { describe, expect, it } from "vitest";
import {
  AudioTranscriptionApiError,
  audioTranscriptionCommandNames,
  createAudioTranscriptionApi,
  type AudioProgressChannelAdapter,
  type AudioProgressChannelHandle,
} from "./audioTranscription";
import type { InvokeAdapter, InvokeArguments } from "./meeting";

const meetingId = "11111111-1111-4111-8111-111111111111";
const otherMeetingId = "22222222-2222-4222-8222-222222222222";
const sourceId = "33333333-3333-4333-8333-333333333333";
const jobId = "44444444-4444-4444-8444-444444444444";
const timestamp = "2026-08-26T00:00:00.000Z";

const policy = {
  supportedExtensions: ["wav", "mp3", "m4a", "mp4"],
  maxFileBytes: 500 * 1024 * 1024,
  maxDurationMs: 4 * 60 * 60 * 1_000,
  maxSegments: 20_000,
  device: "cpu",
  computeType: "int8",
  model: "small",
  modelRevision: "synthetic-revision",
  workerAvailable: true,
  modelAvailable: true,
  busy: false,
};

const meeting = {
  id: meetingId,
  title: "Synthetic audio meeting",
  status: "draft",
  primaryLanguage: "en-AU",
  selectedWindowJson: null,
  startedAt: null,
  endedAt: null,
  createdAt: timestamp,
  updatedAt: timestamp,
  deletionErrorCode: null,
};

const source = {
  id: sourceId,
  meetingId,
  sourceType: "imported_audio",
  displayName: "Synthetic audio transcription",
  originalFilename: "synthetic.wav",
  language: "en-AU",
  baseStartedAt: null,
  segmentCount: 2,
  startOffsetMs: 0,
  endOffsetMs: 10_000,
  speakerCount: 0,
  importedAt: timestamp,
  createdAt: timestamp,
  updatedAt: timestamp,
};

const runningProgress = {
  jobId,
  phase: "transcribing",
  status: "running",
  bytesProcessed: 512,
  totalBytes: 1024,
  processedMs: 5_000,
  durationMs: 10_000,
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

class FakeProgressAdapter implements AudioProgressChannelAdapter {
  handler: ((value: unknown) => void) | null = null;
  disposed = false;
  readonly argument = { syntheticChannel: true };

  create(handler: (value: unknown) => void): AudioProgressChannelHandle {
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

function completedResult(overrides: Record<string, unknown> = {}) {
  return {
    cancelled: false,
    jobId,
    meeting,
    source,
    segmentCount: 2,
    ...overrides,
  };
}

describe("audioTranscriptionApi", () => {
  it("parses the fixed local CPU policy through the approved command", async () => {
    const invoke = new FakeInvokeAdapter();
    invoke.response = policy;
    const api = createAudioTranscriptionApi(invoke, new FakeProgressAdapter());

    await expect(api.getPolicy()).resolves.toEqual(policy);
    expect(invoke.calls).toEqual([
      {
        command: audioTranscriptionCommandNames.getPolicy,
        args: undefined,
      },
    ]);
  });

  it("selects audio with only create metadata and a typed progress channel", async () => {
    const invoke = new FakeInvokeAdapter();
    const channel = new FakeProgressAdapter();
    const api = createAudioTranscriptionApi(invoke, channel);
    const received: unknown[] = [];
    invoke.response = completedResult();
    invoke.beforeResolve = () => channel.handler?.(runningProgress);

    await expect(
      api.selectAudioAndCreateMeeting(
        {
          title: "Synthetic audio meeting",
          primaryLanguage: "en-AU",
        },
        (progress) => received.push(progress),
      ),
    ).resolves.toMatchObject({
      cancelled: false,
      jobId,
      source: { sourceType: "imported_audio" },
      segmentCount: 2,
    });

    expect(invoke.calls).toEqual([
      {
        command: audioTranscriptionCommandNames.selectAndCreateMeeting,
        args: {
          request: {
            title: "Synthetic audio meeting",
            primaryLanguage: "en-AU",
          },
          onProgress: channel.argument,
        },
      },
    ]);
    expect(received).toEqual([runningProgress]);
    expect(channel.disposed).toBe(true);
    expect(JSON.stringify(invoke.calls)).not.toMatch(
      /(?:source|storage|file)?path|segments|originalText/i,
    );
  });

  it("routes add, status and cancellation with job or meeting IDs only", async () => {
    const invoke = new FakeInvokeAdapter();
    const channel = new FakeProgressAdapter();
    const api = createAudioTranscriptionApi(invoke, channel);

    invoke.response = completedResult();
    await api.selectAndAddAudioToMeeting({ meetingId }, () => undefined);
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: audioTranscriptionCommandNames.selectAndAddToMeeting,
      args: {
        request: { meetingId },
        onProgress: channel.argument,
      },
    });

    invoke.response = runningProgress;
    await expect(api.getStatus(jobId)).resolves.toEqual(runningProgress);
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: audioTranscriptionCommandNames.getStatus,
      args: { request: { jobId } },
    });

    invoke.response = {
      ...runningProgress,
      phase: "cancelled",
      status: "cancelled",
      errorCode: "audio_cancelled",
    };
    await expect(api.cancel(jobId)).resolves.toMatchObject({
      status: "cancelled",
    });
    expect(invoke.calls[invoke.calls.length - 1]).toEqual({
      command: audioTranscriptionCommandNames.cancel,
      args: { request: { jobId } },
    });
  });

  it("accepts picker cancellation without meeting or transcript side effects", async () => {
    const invoke = new FakeInvokeAdapter();
    const channel = new FakeProgressAdapter();
    const api = createAudioTranscriptionApi(invoke, channel);
    invoke.response = {
      cancelled: true,
      jobId: null,
      meeting: null,
      source: null,
      segmentCount: 0,
    };

    await expect(
      api.selectAudioAndCreateMeeting(
        { title: "Synthetic", primaryLanguage: "en-AU" },
        () => undefined,
      ),
    ).resolves.toEqual(invoke.response);
    expect(channel.disposed).toBe(true);
  });

  it("accepts a cross-format transcript duplicate returned by Rust", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createAudioTranscriptionApi(invoke, new FakeProgressAdapter());
    invoke.response = completedResult({
      source: { ...source, sourceType: "imported_srt" },
    });

    await expect(
      api.selectAndAddAudioToMeeting({ meetingId }, () => undefined),
    ).resolves.toMatchObject({
      cancelled: false,
      source: { sourceType: "imported_srt" },
    });
  });

  it("rejects malformed progress, paths and cross-meeting results", async () => {
    const invoke = new FakeInvokeAdapter();
    const channel = new FakeProgressAdapter();
    const api = createAudioTranscriptionApi(invoke, channel);

    invoke.response = completedResult();
    invoke.beforeResolve = () =>
      channel.handler?.({ ...runningProgress, sourcePath: "C:\\private.wav" });
    await expect(
      api.selectAudioAndCreateMeeting(
        { title: "Synthetic", primaryLanguage: "en-AU" },
        () => undefined,
      ),
    ).rejects.toMatchObject({ code: "internal_error" });

    invoke.beforeResolve = null;
    invoke.response = completedResult({
      meeting: { ...meeting, id: otherMeetingId },
    });
    await expect(
      api.selectAudioAndCreateMeeting(
        { title: "Synthetic", primaryLanguage: "en-AU" },
        () => undefined,
      ),
    ).rejects.toMatchObject({ code: "internal_error" });

    invoke.response = { ...policy, localModelPath: "C:\\private" };
    await expect(api.getPolicy()).rejects.toMatchObject({
      code: "internal_error",
    });

    invoke.response = {
      ...policy,
      supportedExtensions: ["mp3", "wav", "m4a", "mp4"],
    };
    await expect(api.getPolicy()).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("keeps backend wording and unknown failures out of UI errors", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createAudioTranscriptionApi(invoke, new FakeProgressAdapter());
    invoke.error = {
      code: "audio_worker_crashed",
      message: "private path and worker stderr",
      retryable: true,
    };

    await expect(api.getPolicy()).rejects.toMatchObject({
      code: "audio_worker_crashed",
      retryable: true,
      message: "The local transcription worker stopped unexpectedly.",
    });

    invoke.error = { path: "C:\\private", stderr: "meeting body" };
    await expect(api.getPolicy()).rejects.toEqual(
      new AudioTranscriptionApiError("internal_error", false),
    );
  });

  it("rejects invalid IDs before invoking Rust", async () => {
    const invoke = new FakeInvokeAdapter();
    const api = createAudioTranscriptionApi(invoke, new FakeProgressAdapter());

    await expect(api.getStatus("not-a-job-id")).rejects.toMatchObject({
      code: "audio_invalid_input",
    });
    await expect(api.cancel("not-a-job-id")).rejects.toMatchObject({
      code: "audio_invalid_input",
    });
    expect(invoke.calls).toEqual([]);
  });
});
