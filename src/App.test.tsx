import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import homeSource from "./pages/HomePage.tsx?raw";
import historySource from "./pages/HistoryPage.tsx?raw";
import detailSource from "./pages/MeetingDetailPage.tsx?raw";
import liveSource from "./pages/LiveMeetingPage.tsx?raw";
import newSource from "./pages/NewMeetingPage.tsx?raw";
import App from "./App";
import { MeetingApiError } from "./api/meeting";
import type { AppServices } from "./app/services";
import { formatLocalDate } from "./lib/presentation";
import type {
  TranscriptEntry,
  TranscriptListener,
  TranscriptSource,
} from "./mock/MockTranscriptSource";
import { StatusBadge } from "./components/StatusBadge";
import {
  deferred,
  FakeMeetingApi,
  meetingIds,
  syntheticMeeting,
} from "./test/fakeMeetingApi";

const recordingMeeting = syntheticMeeting({
  id: meetingIds.recording,
  title: "Synthetic recording meeting",
  status: "recording",
  startedAt: "2026-07-30T01:02:00.000Z",
});
const readyMeeting = syntheticMeeting({
  id: meetingIds.ready,
  title: "Synthetic ready meeting",
  status: "ready",
  startedAt: "2026-07-30T01:02:00.000Z",
  endedAt: "2026-07-30T01:03:00.000Z",
  createdAt: "2026-07-30T02:00:00.000Z",
  updatedAt: "2026-07-30T02:03:00.000Z",
});

class ManualTranscriptSource implements TranscriptSource {
  listener: TranscriptListener | null = null;
  startCalls = 0;
  pauseCalls = 0;
  resumeCalls = 0;
  stopCalls = 0;

  start() {
    this.startCalls += 1;
    this.emit([
      {
        id: "synthetic-ui-entry",
        offsetMs: 900,
        speaker: "Speaker A",
        text: "Synthetic UI transcript entry",
        phase: "provisional",
      },
    ]);
  }

  pause() {
    this.pauseCalls += 1;
  }

  resume() {
    this.resumeCalls += 1;
    this.emit([
      {
        id: "synthetic-ui-entry",
        offsetMs: 900,
        speaker: "Speaker A",
        text: "Synthetic UI transcript entry",
        phase: "final",
      },
      {
        id: "synthetic-ui-entry-2",
        offsetMs: 1_800,
        speaker: "Speaker B",
        text: "Synthetic resumed transcript entry",
        phase: "provisional",
      },
    ]);
  }

  stop() {
    this.stopCalls += 1;
  }

  subscribe(listener: TranscriptListener) {
    this.listener = listener;
    listener([]);
    return () => {
      this.listener = null;
    };
  }

  private emit(entries: readonly TranscriptEntry[]) {
    this.listener?.(entries);
  }
}

function appServices(
  api: FakeMeetingApi,
  transcriptSource: TranscriptSource = new ManualTranscriptSource(),
): AppServices {
  return {
    meetingApi: api,
    checkSidecarHealth: async () => ({
      status: "ok",
      service: "meetinglens-sidecar",
      version: "0.1.0",
      api_version: "v1",
    }),
    createTranscriptSource: async () => transcriptSource,
    desktopRuntime: false,
  };
}

function renderAt(
  path: string,
  api: FakeMeetingApi,
  transcriptSource?: TranscriptSource,
) {
  window.history.replaceState(null, "", `#${path}`);
  return render(<App services={appServices(api, transcriptSource)} />);
}

afterEach(() => {
  window.history.replaceState(null, "", "#/");
});

describe("MeetingLens desktop UI", () => {
  it("loads real meetingApi data on the home page", async () => {
    renderAt("/", new FakeMeetingApi([syntheticMeeting()]));

    expect(
      await screen.findByText("Synthetic planning meeting"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Ready to start").previousSibling,
    ).toHaveTextContent("1");
  });

  it("shows a useful home empty state", async () => {
    renderAt("/", new FakeMeetingApi());

    expect(await screen.findByText("No meetings yet")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Create the first meeting" }),
    ).toBeInTheDocument();
  });

  it("shows a safe home error state", async () => {
    const api = new FakeMeetingApi();
    api.listError = new MeetingApiError("database_unavailable", true);
    renderAt("/", api);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Local meeting storage is unavailable",
    );
    expect(screen.getByRole("alert")).toHaveTextContent("database_unavailable");
  });

  it("creates a trimmed draft and navigates to its details", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi();
    renderAt("/new", api);

    await user.type(screen.getByLabelText("Meeting title"), "  Demo review  ");
    await user.click(screen.getByRole("button", { name: "Create meeting" }));

    expect(
      await screen.findByRole("heading", { name: "Demo review", level: 1 }),
    ).toBeInTheDocument();
    expect(api.createCalls).toEqual([
      { title: "Demo review", primaryLanguage: "en-AU" },
    ]);
  });

  it("rejects an empty meeting title before calling the API", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi();
    renderAt("/new", api);

    await user.type(screen.getByLabelText("Meeting title"), "   ");
    await user.click(screen.getByRole("button", { name: "Create meeting" }));

    expect(screen.getByText("Enter a meeting title.")).toBeInTheDocument();
    expect(api.createCalls).toHaveLength(0);
  });

  it("disables creation while the request is pending", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi();
    const pending = deferred<ReturnType<typeof syntheticMeeting>>();
    const created = syntheticMeeting({
      id: "44444444-4444-4444-8444-444444444444",
      title: "Pending synthetic meeting",
    });
    api.createDeferred = pending;
    renderAt("/new", api);

    await user.type(screen.getByLabelText("Meeting title"), created.title);
    await user.click(screen.getByRole("button", { name: "Create meeting" }));
    expect(screen.getByRole("button", { name: "Creating…" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Creating…" }));
    expect(api.createCalls).toHaveLength(1);

    api.meetings.push(created);
    await act(async () => pending.resolve(created));
    expect(
      await screen.findByRole("heading", { name: created.title, level: 1 }),
    ).toBeInTheDocument();
  });

  it("lists history in newest-first order", async () => {
    const older = syntheticMeeting({ title: "Older synthetic meeting" });
    const newer = readyMeeting;
    renderAt("/history", new FakeMeetingApi([older, newer]));

    await screen.findByText(newer.title);
    const headings = screen.getAllByRole("heading", { level: 3 });
    expect(headings.map((heading) => heading.textContent)).toEqual([
      newer.title,
      older.title,
    ]);
  });

  it("renders accessible status labels", async () => {
    renderAt(
      "/history",
      new FakeMeetingApi([syntheticMeeting(), recordingMeeting, readyMeeting]),
    );

    await screen.findByText("Synthetic planning meeting");
    expect(screen.getAllByText("Draft").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Recording").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Ready").length).toBeGreaterThan(0);
  });

  it("deletes a draft after explicit confirmation", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    renderAt("/history", api);
    await screen.findByText("Synthetic planning meeting");

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() =>
      expect(
        screen.queryByText("Synthetic planning meeting"),
      ).not.toBeInTheDocument(),
    );
    expect(api.deleteCalls).toEqual([meetingIds.draft]);
  });

  it("deletes a ready meeting after explicit confirmation", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([readyMeeting]);
    renderAt("/history", api);
    await screen.findByText(readyMeeting.title);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() =>
      expect(screen.queryByText(readyMeeting.title)).not.toBeInTheDocument(),
    );
    expect(api.deleteCalls).toEqual([meetingIds.ready]);
  });

  it("blocks recording deletion and explains meeting_invalid_state", async () => {
    renderAt("/history", new FakeMeetingApi([recordingMeeting]));
    await screen.findByText(recordingMeeting.title);

    expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();
    expect(screen.getByText(/meeting_invalid_state/)).toBeInTheDocument();
  });

  it("cancels deletion without changing the meeting", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    renderAt("/history", api);
    await screen.findByText("Synthetic planning meeting");

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.getByText("Synthetic planning meeting")).toBeInTheDocument();
    expect(api.deleteCalls).toHaveLength(0);
  });

  it("loads meeting details from meetingApi", async () => {
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
    );

    expect(
      await screen.findByRole("heading", {
        name: "Synthetic planning meeting",
        level: 1,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(`ID: ${meetingIds.draft}`)).toBeInTheDocument();
    expect(screen.getByText("en-AU")).toBeInTheDocument();
  });

  it("handles meeting_not_found for a stale meeting link", async () => {
    renderAt(`/meetings/${meetingIds.draft}`, new FakeMeetingApi());

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This meeting no longer exists",
    );
    expect(
      screen.getByRole("link", { name: "Back to history" }),
    ).toBeInTheDocument();
  });

  it("edits a draft title through meetingApi", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    renderAt(`/meetings/${meetingIds.draft}`, api);
    await screen.findByRole("heading", {
      name: "Synthetic planning meeting",
      level: 1,
    });

    await user.click(screen.getByRole("button", { name: "Edit title" }));
    const input = screen.getByLabelText("Meeting title");
    await user.clear(input);
    await user.type(input, "Updated synthetic title");
    await user.click(screen.getByRole("button", { name: "Save title" }));

    expect(
      await screen.findByRole("heading", {
        name: "Updated synthetic title",
        level: 1,
      }),
    ).toBeInTheDocument();
    expect(api.updateCalls).toEqual([
      { id: meetingIds.draft, title: "Updated synthetic title" },
    ]);
  });

  it("starts a draft through the real meetingApi boundary", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    renderAt(`/meetings/${meetingIds.draft}`, api);
    await screen.findByRole("button", { name: "Start meeting" });

    await user.click(screen.getByRole("button", { name: "Start meeting" }));

    expect(await screen.findByText("Live transcript")).toBeInTheDocument();
    expect(api.startCalls).toEqual([meetingIds.draft]);
  });

  it("shows a stable error when a stale start is rejected", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    api.startError = new MeetingApiError("meeting_invalid_state", false);
    renderAt(`/meetings/${meetingIds.draft}`, api);
    await screen.findByRole("button", { name: "Start meeting" });

    await user.click(screen.getByRole("button", { name: "Start meeting" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "meeting_invalid_state",
    );
  });

  it("ends a recording meeting from its detail page", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([recordingMeeting]);
    renderAt(`/meetings/${meetingIds.recording}`, api);
    await screen.findByRole("button", { name: "End meeting" });

    await user.click(screen.getByRole("button", { name: "End meeting" }));

    expect(
      await screen.findByText(
        "This synthetic meeting is complete and stored locally.",
      ),
    ).toBeInTheDocument();
    expect(api.endCalls).toEqual([meetingIds.recording]);
  });

  it("does not allow a ready meeting to start again", async () => {
    renderAt(
      `/meetings/${meetingIds.ready}`,
      new FakeMeetingApi([readyMeeting]),
    );

    expect(
      await screen.findByRole("button", { name: "Start meeting" }),
    ).toBeDisabled();
  });

  it("starts the synthetic transcript only for a recording meeting", async () => {
    const source = new ManualTranscriptSource();
    renderAt(
      `/meetings/${meetingIds.recording}/live`,
      new FakeMeetingApi([recordingMeeting]),
      source,
    );

    expect(
      await screen.findByText("Synthetic UI transcript entry"),
    ).toBeInTheDocument();
    expect(source.startCalls).toBe(1);
    expect(screen.getByText("Synthetic data")).toBeInTheDocument();
  });

  it("pauses and resumes the synthetic transcript", async () => {
    const user = userEvent.setup();
    const source = new ManualTranscriptSource();
    renderAt(
      `/meetings/${meetingIds.recording}/live`,
      new FakeMeetingApi([recordingMeeting]),
      source,
    );
    await screen.findByText("Synthetic UI transcript entry");

    await user.click(screen.getByRole("button", { name: "Pause transcript" }));
    expect(source.pauseCalls).toBe(1);
    expect(screen.getByText("Paused")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Resume transcript" }));
    expect(source.resumeCalls).toBe(1);
    expect(
      await screen.findByText("Synthetic resumed transcript entry"),
    ).toBeInTheDocument();
  });

  it("stops the transcript when the meeting ends", async () => {
    const user = userEvent.setup();
    const source = new ManualTranscriptSource();
    const api = new FakeMeetingApi([recordingMeeting]);
    renderAt(`/meetings/${meetingIds.recording}/live`, api, source);
    await screen.findByText("Synthetic UI transcript entry");

    await user.click(screen.getByRole("button", { name: "End meeting" }));

    await screen.findByText(
      "This synthetic meeting is complete and stored locally.",
    );
    expect(source.stopCalls).toBeGreaterThan(0);
  });

  it("cleans the transcript source up when the page unmounts", async () => {
    const source = new ManualTranscriptSource();
    const view = renderAt(
      `/meetings/${meetingIds.recording}/live`,
      new FakeMeetingApi([recordingMeeting]),
      source,
    );
    await screen.findByText("Synthetic UI transcript entry");

    view.unmount();

    expect(source.stopCalls).toBe(1);
    expect(source.listener).toBeNull();
  });

  it("navigates to ready details after ending a live meeting", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([recordingMeeting]);
    renderAt(`/meetings/${meetingIds.recording}/live`, api);
    await screen.findByText("Live transcript");

    await user.click(screen.getByRole("button", { name: "End meeting" }));

    expect(
      await screen.findByRole("heading", {
        name: recordingMeeting.title,
        level: 1,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "This synthetic meeting is complete and stored locally.",
      ),
    ).toBeInTheDocument();
  });

  it("reloads persistent meetings from meetingApi instead of UI memory", async () => {
    const api = new FakeMeetingApi([readyMeeting]);
    const first = renderAt("/", api);
    expect(await screen.findByText(readyMeeting.title)).toBeInTheDocument();
    first.unmount();

    renderAt("/", api);
    expect(await screen.findByText(readyMeeting.title)).toBeInTheDocument();
  });

  it("keeps Tauri invoke out of page components", () => {
    const pageSources = [
      homeSource,
      historySource,
      detailSource,
      liveSource,
      newSource,
    ].join("\n");
    expect(pageSources).not.toMatch(/@tauri-apps|\binvoke\s*\(/);
    expect(pageSources).toContain("meetingApi");
  });

  it("converts unknown failures into a safe UI message", async () => {
    const api = new FakeMeetingApi();
    api.listError = new Error("C:\\private\\meeting.sqlite3 SELECT secret");
    renderAt("/", api);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "MeetingLens could not complete that action",
    );
    expect(alert).not.toHaveTextContent("private");
    expect(alert).not.toHaveTextContent("SELECT");
  });

  it("formats API timestamps in the user's local timezone", () => {
    const source = "2026-07-30T01:02:00.000Z";
    const formatted = formatLocalDate(source);
    expect(formatted).not.toBe(source);
    expect(formatted).not.toContain("T01:02:00.000Z");
  });

  it("does not expose a complete database path in settings", async () => {
    renderAt("/settings", new FakeMeetingApi());

    expect(
      screen.getByText(
        "Private MeetingLens application storage on this device",
      ),
    ).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\Users\\|\.sqlite3/i);
  });

  it("shows an unknown status fallback without crashing", () => {
    render(<StatusBadge status="future-state" />);
    expect(screen.getByText("Unknown status")).toBeInTheDocument();
  });

  it("does not show recording UI for a draft opened through a live link", async () => {
    renderAt(
      `/meetings/${meetingIds.draft}/live`,
      new FakeMeetingApi([syntheticMeeting()]),
    );

    expect(await screen.findByText("Meeting not started")).toBeInTheDocument();
    expect(screen.queryByText("Live transcript")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start meeting" })).toBeEnabled();
  });

  it("provides labelled navigation and form controls", async () => {
    const user = userEvent.setup();
    renderAt("/", new FakeMeetingApi());
    expect(
      screen.getByRole("navigation", { name: "Primary navigation" }),
    ).toBeInTheDocument();

    await user.click(screen.getAllByRole("link", { name: "New meeting" })[0]);
    expect(screen.getByLabelText("Meeting title")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "New meeting" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("keeps a failed delete in the history list", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([syntheticMeeting()]);
    api.deleteError = new MeetingApiError("persistence_error", true);
    renderAt("/history", api);
    await screen.findByText("Synthetic planning meeting");

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "persistence_error",
    );
    expect(screen.getByText("Synthetic planning meeting")).toBeInTheDocument();
  });

  it("adds a temporary important mark without persisting it", async () => {
    const user = userEvent.setup();
    const api = new FakeMeetingApi([recordingMeeting]);
    renderAt(`/meetings/${meetingIds.recording}/live`, api);
    await screen.findByText("Live transcript");

    const facts = screen.getByText("Important marks").parentElement;
    expect(facts).not.toBeNull();
    expect(within(facts as HTMLElement).getByText("0")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Add important mark" }),
    );
    expect(within(facts as HTMLElement).getByText("1")).toBeInTheDocument();
    expect(api.updateCalls).toHaveLength(0);
  });
});
