import type {
  TranscriptionListener,
  TranscriptionProvider,
  TranscriptionSegment,
  TranscriptionSessionRequest,
} from "../contracts";
import { MockProviderBase } from "./base";

export interface TranscriptScheduler {
  setInterval(handler: () => void, delayMs: number): number;
  clearInterval(id: number): void;
}

const browserScheduler: TranscriptScheduler = {
  setInterval: (handler, delayMs) => window.setInterval(handler, delayMs),
  clearInterval: (id) => window.clearInterval(id),
};

const syntheticScript = [
  {
    speakerLabel: "Speaker A",
    language: "en-AU",
    text: "Welcome to this synthetic MeetingLens product review.",
  },
  {
    speakerLabel: "Speaker B",
    language: "zh-CN",
    text: "今天我们只验证本地会议流程，不发送任何真实数据。",
  },
  {
    speakerLabel: "Speaker C",
    language: "en-AU",
    text: "The Provider contract is running with in-memory mock data.",
  },
  {
    speakerLabel: "Speaker A",
    language: "en-AU",
    text: "Decision: keep the prototype local-first for this milestone.",
  },
  {
    speakerLabel: "Speaker B",
    language: "zh-CN",
    text: "行动项：下一轮单独评审凭据存储与本地会话安全。",
  },
  {
    speakerLabel: "Speaker C",
    language: "en-AU",
    text: "No audio, network, external AI, or files are used in this mock.",
  },
] as const;

export class MockTranscriptionProvider
  extends MockProviderBase
  implements TranscriptionProvider
{
  readonly kind = "transcription" as const;
  readonly name = "Mock transcription";
  private readonly listeners = new Set<TranscriptionListener>();
  private timerId: number | null = null;
  private scriptIndex = 0;
  private sequence = 0;
  private elapsedMs = 0;
  private session: TranscriptionSessionRequest | null = null;
  private provisional: TranscriptionSegment | null = null;

  constructor(
    private readonly scheduler: TranscriptScheduler = browserScheduler,
    private readonly intervalMs = 900,
  ) {
    super("idle");
  }

  async start(request: TranscriptionSessionRequest): Promise<void> {
    this.ensureUsable("start");
    if (this.status === "running" || this.status === "paused") {
      throw this.error("provider_already_running", "start");
    }
    if (
      this.status !== "idle" &&
      this.status !== "ready" &&
      this.status !== "stopped"
    ) {
      throw this.error("provider_invalid_state", "start");
    }
    if (
      request.meetingId.trim().length === 0 ||
      request.language.trim().length === 0 ||
      !Number.isFinite(Date.parse(request.startedAt))
    ) {
      throw this.error("provider_operation_failed", "start");
    }

    this.clearTimer();
    this.session = { ...request };
    this.scriptIndex = 0;
    this.sequence = 0;
    this.elapsedMs = 0;
    this.provisional = null;
    this.setStatus("running");
    this.advance();
    this.schedule();
  }

  async pause(): Promise<void> {
    this.ensureUsable("pause");
    if (this.status !== "running") {
      throw this.error("provider_invalid_state", "pause");
    }
    this.clearTimer();
    this.setStatus("paused");
  }

  async resume(): Promise<void> {
    this.ensureUsable("resume");
    if (this.status !== "paused") {
      throw this.error("provider_invalid_state", "resume");
    }
    this.setStatus("running");
    this.schedule();
  }

  async stop(): Promise<void> {
    this.ensureUsable("stop");
    if (this.status !== "running" && this.status !== "paused") {
      throw this.error("provider_invalid_state", "stop");
    }
    this.setStatus("stopping");
    this.clearTimer();
    this.provisional = null;
    this.session = null;
    this.setStatus("stopped");
  }

  subscribe(listener: TranscriptionListener): () => void {
    this.ensureUsable("subscribe");
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async dispose(): Promise<void> {
    if (this.status === "disposed") return;
    this.clearTimer();
    this.listeners.clear();
    this.session = null;
    this.provisional = null;
    this.markDisposed();
  }

  private schedule(): void {
    this.clearTimer();
    this.timerId = this.scheduler.setInterval(
      () => this.advance(),
      this.intervalMs,
    );
  }

  private advance(): void {
    if (this.status !== "running" || this.session === null) return;
    this.elapsedMs += this.intervalMs;

    if (this.provisional !== null) {
      const finalSegment = { ...this.provisional, isFinal: true };
      this.provisional = null;
      this.emit(finalSegment);
      return;
    }

    const line = syntheticScript[this.scriptIndex];
    this.provisional = {
      segmentId: `mock-segment-${this.sequence}`,
      speakerLabel: line.speakerLabel,
      startOffsetMs: this.elapsedMs,
      endOffsetMs: this.elapsedMs + this.intervalMs,
      text: line.text,
      language: line.language,
      confidence: 0.92,
      isFinal: false,
    };
    this.scriptIndex = (this.scriptIndex + 1) % syntheticScript.length;
    this.sequence += 1;
    this.emit(this.provisional);
  }

  private emit(segment: TranscriptionSegment): void {
    const snapshot = { ...segment };
    for (const listener of this.listeners) listener(snapshot);
  }

  private clearTimer(): void {
    if (this.timerId !== null) {
      this.scheduler.clearInterval(this.timerId);
      this.timerId = null;
    }
  }
}
