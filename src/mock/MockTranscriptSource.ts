export type TranscriptPhase = "provisional" | "final";

export interface TranscriptEntry {
  id: string;
  offsetMs: number;
  speaker: string;
  text: string;
  phase: TranscriptPhase;
}

export type TranscriptListener = (entries: readonly TranscriptEntry[]) => void;

export interface TranscriptSource {
  start(): void;
  pause(): void;
  resume(): void;
  stop(): void;
  subscribe(listener: TranscriptListener): () => void;
}

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
    speaker: "Speaker A",
    text: "Welcome to this synthetic MeetingLens product review.",
  },
  {
    speaker: "Speaker B",
    text: "今天我们只验证本地会议流程，不发送任何真实数据。",
  },
  {
    speaker: "Speaker C",
    text: "The desktop navigation and persistence checks are ready.",
  },
  {
    speaker: "Speaker A",
    text: "Decision: keep the prototype local-first for this milestone.",
  },
  {
    speaker: "Speaker B",
    text: "行动项：下一轮单独评审 Provider 与凭据存储边界。",
  },
  {
    speaker: "Speaker C",
    text: "No audio, external AI, or uploaded documents are in this demo.",
  },
];

export class MockTranscriptSource implements TranscriptSource {
  private readonly listeners = new Set<TranscriptListener>();
  private readonly entries: TranscriptEntry[] = [];
  private timerId: number | null = null;
  private scriptIndex = 0;
  private sequence = 0;
  private elapsedMs = 0;
  private state: "idle" | "running" | "paused" | "stopped" = "idle";

  constructor(
    private readonly scheduler: TranscriptScheduler = browserScheduler,
    private readonly intervalMs = 900,
  ) {}

  start(): void {
    if (this.state === "running") return;
    if (this.state === "paused") {
      this.resume();
      return;
    }
    if (this.state === "stopped") return;

    this.state = "running";
    this.advance();
    this.schedule();
  }

  pause(): void {
    if (this.state !== "running") return;
    this.clearTimer();
    this.state = "paused";
  }

  resume(): void {
    if (this.state !== "paused") return;
    this.state = "running";
    this.schedule();
  }

  stop(): void {
    this.clearTimer();
    this.state = "stopped";
  }

  subscribe(listener: TranscriptListener): () => void {
    this.listeners.add(listener);
    listener([...this.entries]);
    return () => this.listeners.delete(listener);
  }

  private schedule(): void {
    this.clearTimer();
    this.timerId = this.scheduler.setInterval(
      () => this.advance(),
      this.intervalMs,
    );
  }

  private advance(): void {
    if (this.state !== "running") return;
    this.elapsedMs += this.intervalMs;
    const last = this.entries[this.entries.length - 1];

    if (last?.phase === "provisional") {
      last.phase = "final";
    } else {
      const line = syntheticScript[this.scriptIndex];
      this.entries.push({
        id: `synthetic-${this.sequence}`,
        offsetMs: this.elapsedMs,
        speaker: line.speaker,
        text: line.text,
        phase: "provisional",
      });
      this.scriptIndex = (this.scriptIndex + 1) % syntheticScript.length;
      this.sequence += 1;
    }

    this.emit();
  }

  private emit(): void {
    const snapshot = this.entries.map((entry) => ({ ...entry }));
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }

  private clearTimer(): void {
    if (this.timerId !== null) {
      this.scheduler.clearInterval(this.timerId);
      this.timerId = null;
    }
  }
}
