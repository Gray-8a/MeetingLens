import {
  AudioTranscriptionApiError,
  type AudioTranscriptionApi,
  type AudioTranscriptionPolicyResponse,
  type AudioTranscriptionProgress,
  type AudioTranscriptionResult,
  type SelectAndAddAudioToMeetingRequest,
  type SelectAudioAndCreateMeetingRequest,
} from "../api/audioTranscription";
import type { FakeMeetingApi } from "./fakeMeetingApi";
import {
  type FakeTranscriptApi,
  syntheticTranscriptSegment,
  syntheticTranscriptSource,
} from "./fakeTranscriptApi";

const defaultPolicy: AudioTranscriptionPolicyResponse = {
  supportedExtensions: ["wav", "mp3", "m4a", "mp4"],
  maxFileBytes: 2 * 1024 * 1024 * 1024,
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

const jobId = "88888888-8888-4888-8888-888888888888";

export class FakeAudioTranscriptionApi implements AudioTranscriptionApi {
  readonly createCalls: SelectAudioAndCreateMeetingRequest[] = [];
  readonly addCalls: SelectAndAddAudioToMeetingRequest[] = [];
  readonly statusCalls: string[] = [];
  readonly cancelCalls: string[] = [];
  policy: AudioTranscriptionPolicyResponse = clonePolicy(defaultPolicy);
  pickerCancelled = false;
  nextError: unknown = null;
  activeStatus: AudioTranscriptionProgress | null = null;
  private sequence = 200;

  constructor(
    private readonly meetings: FakeMeetingApi,
    private readonly transcripts: FakeTranscriptApi,
  ) {}

  async getPolicy(): Promise<AudioTranscriptionPolicyResponse> {
    this.throwNextError();
    return clonePolicy(this.policy);
  }

  async getStatus(requestedJobId: string): Promise<AudioTranscriptionProgress> {
    this.statusCalls.push(requestedJobId);
    this.throwNextError();
    if (!this.activeStatus || this.activeStatus.jobId !== requestedJobId) {
      throw new AudioTranscriptionApiError("audio_invalid_input", false);
    }
    return { ...this.activeStatus };
  }

  async selectAudioAndCreateMeeting(
    request: SelectAudioAndCreateMeetingRequest,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
  ): Promise<AudioTranscriptionResult> {
    this.createCalls.push({ ...request });
    this.throwNextError();
    if (this.pickerCancelled) return cancelledResult();
    const meeting = await this.meetings.create(request);
    return this.complete(meeting.id, progressHandler);
  }

  async selectAndAddAudioToMeeting(
    request: SelectAndAddAudioToMeetingRequest,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
  ): Promise<AudioTranscriptionResult> {
    this.addCalls.push({ ...request });
    this.throwNextError();
    if (this.pickerCancelled) return cancelledResult();
    await this.meetings.get(request.meetingId);
    return this.complete(request.meetingId, progressHandler);
  }

  async cancel(requestedJobId: string): Promise<AudioTranscriptionProgress> {
    this.cancelCalls.push(requestedJobId);
    this.throwNextError();
    if (!this.activeStatus || this.activeStatus.jobId !== requestedJobId) {
      throw new AudioTranscriptionApiError("audio_invalid_input", false);
    }
    const cancelled: AudioTranscriptionProgress = {
      ...this.activeStatus,
      phase: "cancelled",
      status: "cancelled",
      errorCode: "audio_cancelled",
    };
    this.activeStatus = cancelled;
    return { ...cancelled };
  }

  private async complete(
    meetingId: string,
    progressHandler: (progress: AudioTranscriptionProgress) => void,
  ): Promise<AudioTranscriptionResult> {
    const selected = progress("selected", 0, 0);
    this.activeStatus = selected;
    progressHandler({ ...selected });

    const transcribing = progress("transcribing", 5_000, 10_000);
    this.activeStatus = transcribing;
    progressHandler({ ...transcribing });

    const sourceId = this.uuid();
    const source = syntheticTranscriptSource({
      id: sourceId,
      meetingId,
      sourceType: "imported_audio",
      displayName: "Synthetic audio transcription",
      originalFilename: "synthetic.wav",
      segmentCount: 2,
      startOffsetMs: 0,
      endOffsetMs: 10_000,
      speakerCount: 0,
    });
    this.transcripts.seed(source, [
      syntheticTranscriptSegment({
        id: this.uuid(),
        sourceId,
        meetingId,
        ordinal: 0,
        speakerLabel: null,
        startOffsetMs: 0,
        endOffsetMs: 5_000,
        originalText: "First synthetic audio segment",
      }),
      syntheticTranscriptSegment({
        id: this.uuid(),
        sourceId,
        meetingId,
        ordinal: 1,
        speakerLabel: null,
        startOffsetMs: 5_000,
        endOffsetMs: 10_000,
        originalText: "Second synthetic audio segment",
      }),
    ]);

    const completed = progress("completed", 10_000, 10_000);
    this.activeStatus = completed;
    progressHandler({ ...completed });
    return {
      cancelled: false,
      jobId,
      meeting: await this.meetings.get(meetingId),
      source: { ...source },
      segmentCount: 2,
    };
  }

  private throwNextError(): void {
    if (!this.nextError) return;
    const error = this.nextError;
    this.nextError = null;
    throw error;
  }

  private uuid(): string {
    this.sequence += 1;
    return `99999999-9999-4999-8999-${this.sequence.toString(16).padStart(12, "0")}`;
  }
}

function progress(
  phase: AudioTranscriptionProgress["phase"],
  processedMs: number,
  durationMs: number,
): AudioTranscriptionProgress {
  return {
    jobId,
    phase,
    status: phase === "completed" ? "completed" : "running",
    bytesProcessed: phase === "selected" ? 0 : 1024,
    totalBytes: 1024,
    processedMs,
    durationMs,
    errorCode: null,
  };
}

function cancelledResult(): AudioTranscriptionResult {
  return {
    cancelled: true,
    jobId: null,
    meeting: null,
    source: null,
    segmentCount: 0,
  };
}

function clonePolicy(
  policy: AudioTranscriptionPolicyResponse,
): AudioTranscriptionPolicyResponse {
  return {
    ...policy,
    supportedExtensions: [...policy.supportedExtensions],
  };
}
