import sourceCode from "./MockTranscriptSource.ts?raw";
import { MockTranscriptSource } from "./MockTranscriptSource";

describe("MockTranscriptSource", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("starts with a provisional item and finalizes it", () => {
    const source = new MockTranscriptSource();
    const snapshots: Array<readonly { phase: string }[]> = [];
    source.subscribe((entries) => snapshots.push(entries));

    source.start();
    expect(
      snapshots[snapshots.length - 1]?.[
        snapshots[snapshots.length - 1].length - 1
      ]?.phase,
    ).toBe("provisional");

    vi.advanceTimersByTime(900);
    expect(
      snapshots[snapshots.length - 1]?.[
        snapshots[snapshots.length - 1].length - 1
      ]?.phase,
    ).toBe("final");
    source.stop();
  });

  it("pauses, resumes, and stops deterministic timers", () => {
    const source = new MockTranscriptSource();
    let emissions = 0;
    source.subscribe(() => {
      emissions += 1;
    });

    source.start();
    source.pause();
    const pausedEmissions = emissions;
    vi.advanceTimersByTime(5_000);
    expect(emissions).toBe(pausedEmissions);

    source.resume();
    vi.advanceTimersByTime(900);
    expect(emissions).toBeGreaterThan(pausedEmissions);
    source.stop();
    const stoppedEmissions = emissions;
    vi.advanceTimersByTime(5_000);
    expect(emissions).toBe(stoppedEmissions);
  });

  it("contains only synthetic bilingual content", () => {
    const source = new MockTranscriptSource();
    let latest: readonly { text: string; speaker: string }[] = [];
    source.subscribe((entries) => {
      latest = entries;
    });

    source.start();
    vi.advanceTimersByTime(10_000);
    source.stop();

    expect(latest.some((entry) => /Decision:/.test(entry.text))).toBe(true);
    expect(latest.some((entry) => /行动项/.test(entry.text))).toBe(true);
    expect(new Set(latest.map((entry) => entry.speaker)).size).toBeGreaterThan(
      1,
    );
  });

  it("does not include network or persistence calls", () => {
    expect(sourceCode).not.toMatch(/fetch\(|XMLHttpRequest|WebSocket|invoke\(/);
    expect(sourceCode).not.toMatch(/meetingApi|localStorage|indexedDB/);
  });
});
