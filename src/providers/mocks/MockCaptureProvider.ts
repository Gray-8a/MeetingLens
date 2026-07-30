import type {
  CaptureProvider,
  CaptureSession,
  CaptureSessionRequest,
  CaptureSource,
} from "../contracts";
import type { ProviderOperation } from "../types";
import { MockProviderBase } from "./base";

export interface MockCaptureOptions {
  failOperations?: ReadonlySet<ProviderOperation>;
  now?: () => Date;
}

const mockSources: readonly CaptureSource[] = [
  {
    id: "mock-microphone",
    kind: "microphone",
    label: "Mock microphone (no device access)",
    isMock: true,
  },
  {
    id: "mock-system-audio",
    kind: "systemAudio",
    label: "Mock system audio (no device access)",
    isMock: true,
  },
  {
    id: "mock-window",
    kind: "window",
    label: "Mock window (no screen access)",
    isMock: true,
  },
  {
    id: "mock-screen",
    kind: "screen",
    label: "Mock screen (no screen access)",
    isMock: true,
  },
];

export class MockCaptureProvider
  extends MockProviderBase
  implements CaptureProvider
{
  readonly kind = "capture" as const;
  readonly name = "Mock capture";
  private readonly failOperations: ReadonlySet<ProviderOperation>;
  private readonly now: () => Date;
  private sessionSequence = 0;

  constructor(options: MockCaptureOptions = {}) {
    super("idle");
    this.failOperations = options.failOperations ?? new Set();
    this.now = options.now ?? (() => new Date());
  }

  async initialize(): Promise<void> {
    this.ensureUsable("initialize");
    if (this.status !== "idle") {
      throw this.error("provider_invalid_state", "initialize");
    }
    this.setStatus("initializing");
    if (this.shouldFail("initialize")) {
      this.setStatus("error");
      throw this.error("provider_initialization_failed", "initialize", true);
    }
    this.setStatus("ready");
  }

  async listSources(): Promise<readonly CaptureSource[]> {
    this.ensureUsable("list_sources");
    if (this.status !== "ready" && this.status !== "stopped") {
      throw this.error("provider_not_ready", "list_sources");
    }
    if (this.shouldFail("list_sources")) {
      throw this.error("provider_operation_failed", "list_sources", true);
    }
    return mockSources.map((source) => ({ ...source }));
  }

  async startSession(request: CaptureSessionRequest): Promise<CaptureSession> {
    this.ensureUsable("start");
    if (this.status === "running" || this.status === "paused") {
      throw this.error("provider_already_running", "start");
    }
    if (this.status !== "ready" && this.status !== "stopped") {
      throw this.error("provider_not_ready", "start");
    }
    if (
      request.meetingId.trim().length === 0 ||
      request.sourceIds.length === 0 ||
      request.sourceIds.some(
        (id) => !mockSources.some((source) => source.id === id),
      )
    ) {
      throw this.error("unsupported_capability", "start");
    }
    if (this.shouldFail("start")) {
      this.setStatus("error");
      throw this.error("provider_operation_failed", "start", true);
    }

    this.sessionSequence += 1;
    this.setStatus("running");
    return {
      sessionId: `mock-capture-${this.sessionSequence}`,
      meetingId: request.meetingId,
      startedAt: this.now().toISOString(),
      isMock: true,
    };
  }

  async pauseSession(): Promise<void> {
    this.ensureUsable("pause");
    if (this.status !== "running") {
      throw this.error("provider_invalid_state", "pause");
    }
    this.setStatus("paused");
  }

  async resumeSession(): Promise<void> {
    this.ensureUsable("resume");
    if (this.status !== "paused") {
      throw this.error("provider_invalid_state", "resume");
    }
    this.setStatus("running");
  }

  async stopSession(): Promise<void> {
    this.ensureUsable("stop");
    if (this.status !== "running" && this.status !== "paused") {
      throw this.error("provider_invalid_state", "stop");
    }
    this.setStatus("stopping");
    this.setStatus("stopped");
  }

  async dispose(): Promise<void> {
    if (this.status === "disposed") return;
    this.markDisposed();
  }

  private shouldFail(operation: ProviderOperation): boolean {
    return this.failOperations.has(operation);
  }
}
