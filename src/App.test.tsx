import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import homeSource from "./pages/HomePage.tsx?raw";
import historySource from "./pages/HistoryPage.tsx?raw";
import detailSource from "./pages/MeetingDetailPage.tsx?raw";
import liveSource from "./pages/LiveMeetingPage.tsx?raw";
import newSource from "./pages/NewMeetingPage.tsx?raw";
import settingsSource from "./pages/SettingsPage.tsx?raw";
import documentPanelSource from "./components/DocumentPanel.tsx?raw";
import helpSource from "./pages/HelpPage.tsx?raw";
import App from "./App";
import { DocumentApiError } from "./api/document";
import { MeetingApiError } from "./api/meeting";
import { SecretApiError } from "./api/secret";
import { SummaryApiError } from "./api/summary";
import type { TranscriptApi } from "./api/transcript";
import type { AppServices } from "./app/services";
import { formatLocalDate } from "./lib/presentation";
import i18n, { changeUiLocale, UI_LOCALE_STORAGE_KEY } from "./i18n";
import { SidecarApiError } from "./lib/sidecar";
import type {
  TranscriptionListener,
  TranscriptionProvider,
  TranscriptionSegment,
} from "./providers/contracts";
import { ProviderRegistry } from "./providers/runtime";
import type {
  ProviderMode,
  ProviderStatus,
  ProviderStatusListener,
} from "./providers/types";
import { StatusBadge } from "./components/StatusBadge";
import {
  deferred,
  FakeMeetingApi,
  meetingIds,
  syntheticMeeting,
} from "./test/fakeMeetingApi";
import { FakeSecretApi } from "./test/fakeSecretApi";
import { FakeSummaryApi, syntheticDeepSeekStatus } from "./test/fakeSummaryApi";
import { FakeTranscriptApi } from "./test/fakeTranscriptApi";
import {
  documentIds,
  FakeDocumentApi,
  syntheticDocument,
} from "./test/fakeDocumentApi";

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

class ManualTranscriptionProvider implements TranscriptionProvider {
  readonly kind = "transcription" as const;
  readonly mode: ProviderMode = "mock";
  readonly name = "Manual mock transcription";
  listener: TranscriptionListener | null = null;
  status: ProviderStatus = "idle";
  startCalls = 0;
  pauseCalls = 0;
  resumeCalls = 0;
  stopCalls = 0;

  getStatus() {
    return this.status;
  }

  subscribeStatus(_listener: ProviderStatusListener) {
    void _listener;
    return () => undefined;
  }

  async start() {
    this.startCalls += 1;
    this.status = "running";
    this.emit({
      segmentId: "synthetic-ui-entry",
      startOffsetMs: 900,
      endOffsetMs: 1_800,
      speakerLabel: "Speaker A",
      text: "Synthetic UI transcript entry",
      language: "en-AU",
      confidence: 0.9,
      isFinal: false,
    });
  }

  async pause() {
    this.pauseCalls += 1;
    this.status = "paused";
  }

  async resume() {
    this.resumeCalls += 1;
    this.status = "running";
    this.emit({
      segmentId: "synthetic-ui-entry",
      startOffsetMs: 900,
      endOffsetMs: 1_800,
      speakerLabel: "Speaker A",
      text: "Synthetic UI transcript entry",
      language: "en-AU",
      confidence: 0.9,
      isFinal: true,
    });
    this.emit({
      segmentId: "synthetic-ui-entry-2",
      startOffsetMs: 1_800,
      endOffsetMs: 2_700,
      speakerLabel: "Speaker B",
      text: "Synthetic resumed transcript entry",
      language: "en-AU",
      confidence: 0.9,
      isFinal: false,
    });
  }

  async stop() {
    this.stopCalls += 1;
    this.status = "stopped";
  }

  subscribe(listener: TranscriptionListener) {
    this.listener = listener;
    return () => {
      this.listener = null;
    };
  }

  async dispose() {
    this.status = "disposed";
    this.listener = null;
  }

  private emit(segment: TranscriptionSegment) {
    this.listener?.(segment);
  }
}

function appServices(
  api: FakeMeetingApi,
  transcriptionProvider: TranscriptionProvider = new ManualTranscriptionProvider(),
  documentApi: FakeDocumentApi = new FakeDocumentApi(),
  transcriptApi: TranscriptApi = new FakeTranscriptApi(api),
): AppServices {
  return {
    documentApi,
    meetingApi: api,
    secretApi: new FakeSecretApi(),
    summaryApi: new FakeSummaryApi(),
    transcriptApi,
    checkSidecarHealth: async () => ({
      status: "authenticated",
      binding: "loopback",
      session: "current",
      secretStorage: "not_implemented",
      service: "meetinglens-sidecar",
      version: "0.1.0",
      apiVersion: "v1",
    }),
    providers: new ProviderRegistry({ transcription: transcriptionProvider }),
    desktopRuntime: false,
  };
}

function renderAt(
  path: string,
  api: FakeMeetingApi,
  transcriptionProvider?: TranscriptionProvider,
  documentApi?: FakeDocumentApi,
) {
  window.history.replaceState(null, "", `#${path}`);
  return render(
    <App services={appServices(api, transcriptionProvider, documentApi)} />,
  );
}

function renderSettings(
  secretApi: FakeSecretApi,
  summaryApi: FakeSummaryApi = new FakeSummaryApi(),
) {
  const services = appServices(new FakeMeetingApi());
  services.secretApi = secretApi;
  services.summaryApi = summaryApi;
  window.history.replaceState(null, "", "#/settings");
  return render(<App services={services} />);
}

afterEach(() => {
  vi.restoreAllMocks();
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
    expect(screen.getAllByText("Preparing").length).toBeGreaterThan(0);
    expect(screen.getAllByText("In meeting").length).toBeGreaterThan(0);
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
      await screen.findByText("This meeting is complete and stored locally."),
    ).toBeInTheDocument();
    expect(api.endCalls).toEqual([meetingIds.recording]);
  });

  it("does not allow a ready meeting to start again and offers minutes", async () => {
    renderAt(
      `/meetings/${meetingIds.ready}`,
      new FakeMeetingApi([readyMeeting]),
    );

    expect(
      await screen.findByRole("link", { name: "Open meeting minutes" }),
    ).toHaveAttribute("href", `#/meetings/${meetingIds.ready}/minutes`);
    expect(
      screen.queryByRole("button", { name: "Start meeting" }),
    ).not.toBeInTheDocument();
  });

  it("starts the synthetic transcript only for a recording meeting", async () => {
    const source = new ManualTranscriptionProvider();
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
    const source = new ManualTranscriptionProvider();
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
    const source = new ManualTranscriptionProvider();
    const api = new FakeMeetingApi([recordingMeeting]);
    renderAt(`/meetings/${meetingIds.recording}/live`, api, source);
    await screen.findByText("Synthetic UI transcript entry");

    await user.click(screen.getByRole("button", { name: "End meeting" }));

    await screen.findByText("This meeting is complete and stored locally.");
    expect(source.stopCalls).toBeGreaterThan(0);
  });

  it("cleans the transcript source up when the page unmounts", async () => {
    const source = new ManualTranscriptionProvider();
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
      screen.getByText("This meeting is complete and stored locally."),
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
      settingsSource,
    ].join("\n");
    expect(pageSources).not.toMatch(/@tauri-apps|\binvoke\s*\(/);
    expect(pageSources).toContain("meetingApi");
    expect(liveSource).not.toContain("MockTranscriptSource");
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
    await screen.findByText("Not configured");

    expect(
      screen.getByText(
        "Private MeetingLens application storage on this device",
      ),
    ).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\Users\\|\.sqlite3/i);
  });

  it("shows an unconfigured DeepSeek secret without revealing a value", async () => {
    renderSettings(new FakeSecretApi());

    expect(await screen.findByText("Not configured")).toBeInTheDocument();
    const input = screen.getByLabelText("API key");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("autocomplete", "off");
    expect(input).toHaveValue("");
    expect(screen.queryByRole("button", { name: /reveal|copy/i })).toBeNull();
    expect(
      screen.queryByRole("button", { name: /test connection/i }),
    ).toBeNull();
  });

  it("shows configured status without prefilling the saved secret", async () => {
    const api = new FakeSecretApi();
    api.configured = true;
    renderSettings(api);

    expect(await screen.findByText("Configured")).toBeInTheDocument();
    expect(screen.getByLabelText("API key")).toHaveValue("");
    expect(
      screen.getByRole("button", { name: "Delete stored key" }),
    ).toBeEnabled();
  });

  it("shows SecretStore unavailable and disables secret actions", async () => {
    const api = new FakeSecretApi();
    api.available = false;
    renderSettings(api);

    expect(
      await screen.findByText("SecretStore unavailable"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("API key")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save API key" })).toBeDisabled();
  });

  it("rejects ASCII-whitespace input before calling secretApi", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    renderSettings(api);
    await screen.findByText("Not configured");

    await user.type(screen.getByLabelText("API key"), "   ");
    await user.click(screen.getByRole("button", { name: "Save API key" }));

    expect(screen.getByRole("status")).toHaveTextContent(
      "Enter a non-empty API key.",
    );
    expect(api.setCallCount).toBe(0);
  });

  it("blocks duplicate saves while a secret write is pending", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    const pending = deferred<void>();
    api.setPending = pending.promise;
    renderSettings(api);
    await screen.findByText("Not configured");

    await user.type(screen.getByLabelText("API key"), crypto.randomUUID());
    await user.click(screen.getByRole("button", { name: "Save API key" }));
    expect(screen.getByRole("button", { name: "Saving…" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Saving…" }));
    expect(api.setCallCount).toBe(1);

    pending.resolve();
    expect(await screen.findByText("Configured")).toBeInTheDocument();
  });

  it("clears the input after save and retains only configured status", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    renderSettings(api);
    await screen.findByText("Not configured");
    const input = screen.getByLabelText("API key");

    await user.type(input, crypto.randomUUID());
    await user.click(screen.getByRole("button", { name: "Save API key" }));

    expect(await screen.findByText("Configured")).toBeInTheDocument();
    expect(input).toHaveValue("");
    expect(screen.getByRole("status")).toHaveTextContent(
      "API key saved to secure local storage.",
    );
  });

  it("cancels deletion without changing configured status", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    api.configured = true;
    renderSettings(api);
    await screen.findByText("Configured");

    await user.click(screen.getByRole("button", { name: "Delete stored key" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.getByText("Configured")).toBeInTheDocument();
    expect(api.deleteCallCount).toBe(0);
  });

  it("deletes a configured secret after explicit confirmation", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    api.configured = true;
    renderSettings(api);
    await screen.findByText("Configured");

    await user.click(screen.getByRole("button", { name: "Delete stored key" }));
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));

    expect(await screen.findByText("Not configured")).toBeInTheDocument();
    expect(api.deleteCallCount).toBe(1);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Stored API key deleted.",
    );
  });

  it("treats a missing secret delete as an idempotent unconfigured result", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    api.configured = true;
    const originalDelete = api.delete.bind(api);
    api.delete = async (kind) => {
      api.configured = false;
      const result = await originalDelete(kind);
      return { ...result, deleted: false };
    };
    renderSettings(api);
    await screen.findByText("Configured");

    await user.click(screen.getByRole("button", { name: "Delete stored key" }));
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));

    expect(await screen.findByText("Not configured")).toBeInTheDocument();
  });

  it("renders a stable secret error without backend wording", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    api.setError = new SecretApiError("secret_access_denied", false);
    renderSettings(api);
    await screen.findByText("Not configured");

    await user.type(screen.getByLabelText("API key"), crypto.randomUUID());
    await user.click(screen.getByRole("button", { name: "Save API key" }));

    expect(await screen.findByText("Secure storage error")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(
      "Secure local storage denied access.",
    );
  });

  it("maps unknown secret failures without exposing the original object", async () => {
    const user = userEvent.setup();
    const api = new FakeSecretApi();
    api.setError = new Error("private native credential details");
    renderSettings(api);
    await screen.findByText("Not configured");

    await user.type(screen.getByLabelText("API key"), crypto.randomUUID());
    await user.click(screen.getByRole("button", { name: "Save API key" }));

    expect(screen.getByRole("status")).toHaveTextContent(
      "MeetingLens could not complete that action.",
    );
    expect(document.body.textContent).not.toContain("private native");
  });

  it("clears the password input when settings unmounts", async () => {
    const user = userEvent.setup();
    const view = renderSettings(new FakeSecretApi());
    await screen.findByText("Not configured");
    const input = screen.getByLabelText("API key") as HTMLInputElement;
    await user.type(input, crypto.randomUUID());

    view.unmount();

    expect(input.value).toBe("");
  });

  it("does not write secret input to browser storage or console", async () => {
    const user = userEvent.setup();
    const storageSet = vi.spyOn(Storage.prototype, "setItem");
    const consoleLog = vi
      .spyOn(console, "log")
      .mockImplementation(() => undefined);
    const consoleInfo = vi
      .spyOn(console, "info")
      .mockImplementation(() => undefined);
    const consoleWarn = vi
      .spyOn(console, "warn")
      .mockImplementation(() => undefined);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    renderSettings(new FakeSecretApi());
    await screen.findByText("Not configured");

    await user.type(screen.getByLabelText("API key"), crypto.randomUUID());
    await user.click(screen.getByRole("button", { name: "Save API key" }));
    await screen.findByText("Configured");

    expect(storageSet).not.toHaveBeenCalled();
    expect(consoleLog).not.toHaveBeenCalled();
    expect(consoleInfo).not.toHaveBeenCalled();
    expect(consoleWarn).not.toHaveBeenCalled();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("shows the fixed DeepSeek policy and remembered sharing status", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      consentRemembered: true,
      consentedAt: "2026-08-17T02:03:04.000Z",
      executionAvailable: true,
    });
    const secretApi = new FakeSecretApi();
    secretApi.configured = true;
    renderSettings(secretApi, summaryApi);

    expect(await screen.findByText("api.deepseek.com")).toBeInTheDocument();
    expect(screen.getByText("deepseek-v4-pro")).toBeInTheDocument();
    expect(screen.getByText("Remembered")).toBeInTheDocument();
    expect(
      screen.getByText(formatLocalDate("2026-08-17T02:03:04.000Z")),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Revoke sharing consent" }),
    ).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /authorize|consent in advance/i }),
    ).toBeNull();
    expect(screen.queryByLabelText(/base url|model/i)).toBeNull();
  });

  it("revokes remembered DeepSeek consent without touching the API key", async () => {
    const user = userEvent.setup();
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      consentRemembered: true,
      consentedAt: "2026-08-17T02:03:04.000Z",
      executionAvailable: true,
    });
    const secretApi = new FakeSecretApi();
    secretApi.configured = true;
    renderSettings(secretApi, summaryApi);
    await screen.findByText("Remembered");

    await user.click(
      screen.getByRole("button", { name: "Revoke sharing consent" }),
    );

    expect(summaryApi.revokeCallCount).toBe(1);
    expect(await screen.findByText("Not remembered")).toBeInTheDocument();
    expect(screen.getByText("Configured")).toBeInTheDocument();
    expect(secretApi.deleteCallCount).toBe(0);
    expect(document.body).toHaveTextContent(
      "Future sends require confirmation again.",
    );
  });

  it("revokes unused one-time consent even when no choice is remembered", async () => {
    const user = userEvent.setup();
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      consentRemembered: false,
      consentedAt: null,
    });
    renderSettings(new FakeSecretApi(), summaryApi);
    await screen.findByText("Not remembered");

    const button = screen.getByRole("button", {
      name: "Revoke sharing consent",
    });
    expect(button).toBeEnabled();
    await user.click(button);

    expect(summaryApi.revokeCallCount).toBe(1);
    expect(
      await screen.findByText(
        "Sharing consent was revoked. Future sends require confirmation again.",
      ),
    ).toBeInTheDocument();
  });

  it("blocks duplicate consent revocation while the command is pending", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      consentRemembered: true,
      consentedAt: "2026-08-17T02:03:04.000Z",
    });
    const pending = deferred<void>();
    summaryApi.revokePending = pending.promise;
    renderSettings(new FakeSecretApi(), summaryApi);
    await screen.findByText("Remembered");

    const button = screen.getByRole("button", {
      name: "Revoke sharing consent",
    });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(summaryApi.revokeCallCount).toBe(1);
    expect(screen.getByRole("button", { name: "Revoking…" })).toBeDisabled();
    await act(async () => pending.resolve());
  });

  it("renders a stable consent error without backend wording", async () => {
    const user = userEvent.setup();
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      consentRemembered: true,
      consentedAt: "2026-08-17T02:03:04.000Z",
    });
    summaryApi.revokeError = new SummaryApiError(
      "deepseek_network_failed",
      true,
    );
    renderSettings(new FakeSecretApi(), summaryApi);
    await screen.findByText("Remembered");
    await user.click(
      screen.getByRole("button", { name: "Revoke sharing consent" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "MeetingLens could not reach DeepSeek.",
    );
    expect(document.body).not.toHaveTextContent("private backend wording");
  });

  it("shows the authenticated Sidecar boundary without internal details", async () => {
    const services = appServices(new FakeMeetingApi());
    services.desktopRuntime = true;
    window.history.replaceState(null, "", "#/settings");
    render(<App services={services} />);

    expect(
      await screen.findByText("Local Sidecar authenticated"),
    ).toBeInTheDocument();
    expect(screen.getByText("This device only")).toBeInTheDocument();
    expect(screen.getByText("Current application session")).toBeInTheDocument();
    expect(screen.getByText("Disabled")).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(
      /Authorization|Bearer|127\.0\.0\.1|8765|MEETINGLENS_SESSION_TOKEN/i,
    );
  });

  it("shows a distinct unavailable Sidecar state", async () => {
    const services = appServices(new FakeMeetingApi());
    services.desktopRuntime = true;
    services.checkSidecarHealth = async () => {
      throw new SidecarApiError("sidecar_unavailable", true);
    };
    window.history.replaceState(null, "", "#/settings");
    render(<App services={services} />);

    expect(
      await screen.findByText("Local service unavailable"),
    ).toBeInTheDocument();
  });

  it("maps an unknown Sidecar failure to a safe error state", async () => {
    const services = appServices(new FakeMeetingApi());
    services.desktopRuntime = true;
    services.checkSidecarHealth = async () => {
      throw new Error("private runtime details");
    };
    window.history.replaceState(null, "", "#/settings");
    render(<App services={services} />);

    expect(await screen.findByText("Local Sidecar error")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("private runtime details");
  });

  it("shows all five providers as explicit mocks in settings", async () => {
    renderAt("/settings", new FakeMeetingApi());
    await screen.findByText("Not configured");

    expect(screen.getByText("Capture")).toBeInTheDocument();
    expect(screen.getByText("Transcription")).toBeInTheDocument();
    expect(screen.getByText("Translation")).toBeInTheDocument();
    expect(screen.getByText("Summary")).toBeInTheDocument();
    expect(screen.getByText("Document parser")).toBeInTheDocument();
    expect(screen.getAllByText(/Mock · .* · Not enabled/)).toHaveLength(5);
  });

  it("disposes all providers when the desktop page is closed", async () => {
    const services = appServices(new FakeMeetingApi());
    window.history.replaceState(null, "", "#/settings");
    render(<App services={services} />);

    await act(async () => {
      window.dispatchEvent(new Event("pagehide"));
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(
        services.providers
          .snapshots()
          .every(({ status }) => status === "disposed"),
      ).toBe(true),
    );
  });

  it("shows a safe fallback when a provider reports a future status", async () => {
    const services = appServices(new FakeMeetingApi());
    vi.spyOn(services.providers.get("capture"), "getStatus").mockReturnValue(
      "future-provider-state" as ProviderStatus,
    );
    window.history.replaceState(null, "", "#/settings");

    render(<App services={services} />);
    await screen.findByText("Not configured");

    expect(
      screen.getByText(/Mock · Unknown status · Not enabled/),
    ).toBeInTheDocument();
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

  it("shows the document policy and an empty state on meeting details", async () => {
    const documents = new FakeDocumentApi();
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );

    expect(await screen.findByText("No files added")).toBeInTheDocument();
    expect(screen.getByText("0 / 20 files")).toBeInTheDocument();
    expect(screen.getByText(/\.pdf, \.docx/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add files" })).toBeEnabled();
  });

  it("loads safe document metadata without showing hashes or paths", async () => {
    const documents = new FakeDocumentApi([
      syntheticDocument({ meetingId: meetingIds.draft }),
    ]);
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );

    expect(await screen.findByText("synthetic-notes.txt")).toBeInTheDocument();
    expect(screen.getByText(/TXT · 128 B/)).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("storageRelativePath");
    expect(document.body).not.toHaveTextContent("sha256");
    expect(document.body).not.toHaveTextContent("C:\\");
  });

  it("shows a stable document list error", async () => {
    const documents = new FakeDocumentApi();
    documents.listError = new DocumentApiError(
      "document_storage_unavailable",
      true,
    );
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "document_storage_unavailable",
    );
    expect(document.body).not.toHaveTextContent("private path");
  });

  it.each([
    ["draft", true],
    ["recording", true],
    ["ready", true],
    ["processing", false],
    ["deleting", false],
    ["delete_failed", false],
  ] as const)(
    "sets document import availability for %s",
    async (status, enabled) => {
      const meeting = syntheticMeeting({ status });
      renderAt(
        `/meetings/${meeting.id}`,
        new FakeMeetingApi([meeting]),
        undefined,
        new FakeDocumentApi(),
      );

      const button = await screen.findByRole("button", { name: "Add files" });
      if (enabled) expect(button).toBeEnabled();
      else expect(button).toBeDisabled();
    },
  );

  it("shows progress, duplicates and partial failures safely", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi([
      syntheticDocument({ meetingId: meetingIds.draft }),
    ]);
    documents.progressEvents = [
      {
        batchId: "77777777-7777-4777-8777-777777777777",
        fileIndex: 1,
        totalFiles: 2,
        documentId: documentIds.staged,
        originalFilename: "synthetic-notes.txt",
        phase: "copying",
        bytesProcessed: 64,
        totalBytes: 128,
        status: "in_progress",
        errorCode: null,
      },
      {
        batchId: "77777777-7777-4777-8777-777777777777",
        fileIndex: 2,
        totalFiles: 2,
        documentId: null,
        originalFilename: "bad.pdf",
        phase: "failed",
        bytesProcessed: 0,
        totalBytes: 8,
        status: "failed",
        errorCode: "document_type_mismatch",
      },
    ];
    documents.selectResult = {
      batchId: "77777777-7777-4777-8777-777777777777",
      cancelled: false,
      imported: [],
      duplicates: [
        {
          documentId: documentIds.staged,
          originalFilename: "duplicate.txt",
          status: "duplicate",
          errorCode: "document_already_imported",
        },
      ],
      failed: [
        {
          documentId: "88888888-8888-4888-8888-888888888888",
          originalFilename: "bad.pdf",
          status: "failed",
          errorCode: "document_type_mismatch",
        },
      ],
    };
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await screen.findByText("synthetic-notes.txt");

    await user.click(screen.getByRole("button", { name: "Add files" }));

    expect(await screen.findByText("50%")).toBeInTheDocument();
    expect(screen.getByText(/duplicate\.txt is already/)).toBeInTheDocument();
    expect(
      screen.getAllByText(/document_type_mismatch/).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/does not match its filename type/).length,
    ).toBeGreaterThan(0);
    expect(
      document.querySelectorAll("[data-acceptance='document-list'] > li"),
    ).toHaveLength(1);
  });

  it("treats native picker cancellation as a quiet no-op", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi();
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await screen.findByText("No files added");

    await user.click(screen.getByRole("button", { name: "Add files" }));

    await waitFor(() => expect(documents.selectCalls).toHaveLength(1));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("No files added")).toBeInTheDocument();
  });

  it("prevents duplicate picker launches while one import is active", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi();
    const pending = deferred<typeof documents.selectResult>();
    documents.selectPromise = pending.promise;
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    const add = await screen.findByRole("button", { name: "Add files" });

    await user.click(add);
    expect(add).toBeDisabled();
    await user.click(add);
    expect(documents.selectCalls).toHaveLength(1);

    await act(async () => pending.resolve(documents.selectResult));
    await waitFor(() => expect(add).toBeEnabled());
  });

  it("cancels and confirms document deletion without touching the source", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi([
      syntheticDocument({ meetingId: meetingIds.draft }),
    ]);
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await screen.findByText("synthetic-notes.txt");

    await user.click(
      screen.getByRole("button", { name: "Delete synthetic-notes.txt" }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(documents.requestDeleteCalls).toHaveLength(0);

    await user.click(
      screen.getByRole("button", { name: "Delete synthetic-notes.txt" }),
    );
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() =>
      expect(documents.requestDeleteCalls).toEqual([documentIds.staged]),
    );
    expect(screen.queryByText("synthetic-notes.txt")).not.toBeInTheDocument();
  });

  it("keeps pending document deletion visible instead of claiming success", async () => {
    const user = userEvent.setup();
    const document = syntheticDocument({ meetingId: meetingIds.draft });
    const documents = new FakeDocumentApi([document]);
    documents.deleteResponse = {
      documentId: document.id,
      status: "deleting",
      deleted: false,
      pending: true,
      retryable: true,
    };
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await screen.findByText("synthetic-notes.txt");

    await user.click(
      screen.getByRole("button", { name: "Delete synthetic-notes.txt" }),
    );
    await user.click(screen.getByRole("button", { name: "Confirm" }));

    expect(await screen.findByText("Deleting")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "document_delete_pending",
    );
  });

  it("retries a failed document deletion", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi([
      syntheticDocument({
        meetingId: meetingIds.draft,
        status: "delete_failed",
        failureCode: "document_delete_failed",
      }),
    ]);
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );

    await user.click(
      await screen.findByRole("button", { name: "Retry deletion" }),
    );
    await waitFor(() =>
      expect(documents.retryDeleteCalls).toEqual([documentIds.staged]),
    );
    expect(screen.queryByText("synthetic-notes.txt")).not.toBeInTheDocument();
  });

  it("renders hostile filenames as text and does not use dangerous HTML", async () => {
    const documents = new FakeDocumentApi([
      syntheticDocument({
        meetingId: meetingIds.draft,
        originalFilename: "<img src=x onerror=alert(1)>\u0000.txt",
      }),
    ]);
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );

    expect(
      await screen.findByText("<img src=x onerror=alert(1)>�.txt"),
    ).toBeInTheDocument();
    expect(document.querySelector("img[src='x']")).toBeNull();
    expect(documentPanelSource).not.toContain("dangerouslySetInnerHTML");
  });

  it("ignores progress callbacks after the document panel unmounts", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi();
    const pending = deferred<typeof documents.selectResult>();
    documents.selectPromise = pending.promise;
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await user.click(await screen.findByRole("button", { name: "Add files" }));
    const handler = documents.progressHandler;
    expect(handler).not.toBeNull();

    await user.click(screen.getByRole("link", { name: "Home" }));
    await screen.findByText("What would you like to do?");
    act(() => {
      handler?.({
        batchId: "77777777-7777-4777-8777-777777777777",
        fileIndex: 1,
        totalFiles: 1,
        documentId: null,
        originalFilename: "late.txt",
        phase: "copying",
        bytesProcessed: 1,
        totalBytes: 2,
        status: "in_progress",
        errorCode: null,
      });
    });
    await act(async () => pending.resolve(documents.selectResult));
    expect(screen.queryByText("late.txt")).not.toBeInTheDocument();
  });

  it("imports from the recording page without disrupting the mock transcript", async () => {
    const user = userEvent.setup();
    const documents = new FakeDocumentApi();
    renderAt(
      `/meetings/${meetingIds.recording}/live`,
      new FakeMeetingApi([recordingMeeting]),
      undefined,
      documents,
    );
    await screen.findByText("Synthetic UI transcript entry");

    await user.click(screen.getByRole("button", { name: "Add files" }));

    await waitFor(() =>
      expect(documents.selectCalls).toEqual([meetingIds.recording]),
    );
    expect(
      screen.getByText("Synthetic UI transcript entry"),
    ).toBeInTheDocument();
  });

  it("shows and retries a meeting delete_failed tombstone", async () => {
    const user = userEvent.setup();
    const failed = syntheticMeeting({
      status: "delete_failed",
      deletionErrorCode: "meeting_delete_failed",
    });
    const api = new FakeMeetingApi([failed]);
    renderAt("/history", api);
    await screen.findByText(failed.title);

    await user.click(screen.getByRole("button", { name: "Retry deletion" }));

    await waitFor(() => expect(api.deleteCalls).toEqual([failed.id]));
    expect(screen.queryByText(failed.title)).not.toBeInTheDocument();
  });

  it("freezes a deleting meeting and shows pending instead of mutable actions", async () => {
    const deleting = syntheticMeeting({ status: "deleting" });
    renderAt("/history", new FakeMeetingApi([deleting]));
    await screen.findByText(deleting.title);

    expect(
      screen.getByText(/Meeting deletion is still pending/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete" })).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: "Retry deletion" }),
    ).not.toBeInTheDocument();
  });

  it("keeps document access centralized and out of browser storage", () => {
    expect(documentPanelSource).not.toContain("invoke(");
    expect(documentPanelSource).not.toContain("localStorage");
    expect(documentPanelSource).not.toContain("sessionStorage");
    expect(documentPanelSource).not.toContain("indexedDB");
    expect(
      [homeSource, historySource, detailSource, liveSource, newSource].join(
        "\n",
      ),
    ).not.toContain("select_and_import_documents");
  });

  it("switches the settings page language immediately", async () => {
    const user = userEvent.setup();
    renderAt("/settings", new FakeMeetingApi());
    await screen.findByRole("heading", { name: "Settings", level: 1 });

    await user.selectOptions(
      screen.getByRole("combobox", { name: "Select interface language" }),
      "zh-CN",
    );

    expect(
      await screen.findByRole("heading", { name: "设置", level: 1 }),
    ).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "选择界面语言" })).toHaveValue(
      "zh-CN",
    );
  });

  it("switches all primary navigation labels together", async () => {
    renderAt("/", new FakeMeetingApi());
    await act(async () => changeUiLocale("zh-CN", undefined));

    const navigation = screen.getByRole("navigation", { name: "主导航" });
    for (const label of ["首页", "新建会议", "历史记录", "设置", "帮助"]) {
      expect(
        within(navigation).getByRole("link", { name: label }),
      ).toBeInTheDocument();
    }
  });

  it("renders the Help page in Simplified Chinese", async () => {
    await i18n.changeLanguage("zh-CN");
    renderAt("/help", new FakeMeetingApi());

    expect(
      await screen.findByRole("heading", { name: "帮助", level: 1 }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("开发预览版").length).toBeGreaterThan(0);
    expect(
      screen.getByRole("heading", { name: "隐私与本地存储" }),
    ).toBeInTheDocument();
  });

  it("renders the Help page in English", async () => {
    renderAt("/help", new FakeMeetingApi());

    expect(
      await screen.findByRole("heading", { name: "Help", level: 1 }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Privacy and local storage" }),
    ).toBeInTheDocument();
  });

  it("describes the available minutes flow and trusted provider boundary", async () => {
    renderAt("/help", new FakeMeetingApi());
    await screen.findByRole("heading", { name: "What is not available yet" });

    expect(
      screen.getByText(/generate structured meeting minutes/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Docling is not connected to this UI yet/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Rust-only DeepSeek provider consumes it only/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/WebView never receives the key/),
    ).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent(
      "no real DeepSeek Provider uses it",
    );
    expect(helpSource).not.toContain("dangerouslySetInnerHTML");
  });

  it("does not reset meeting data when the interface language changes", async () => {
    const meeting = syntheticMeeting({ title: "Persistent synthetic meeting" });
    const api = new FakeMeetingApi([meeting]);
    renderAt("/history", api);
    await screen.findByText(meeting.title);

    await act(async () => changeUiLocale("zh-CN", undefined));

    expect(screen.getByText(meeting.title)).toBeInTheDocument();
    expect(api.meetings).toEqual([meeting]);
    expect(api.deleteCalls).toEqual([]);
  });

  it("does not change Provider state when the interface language changes", async () => {
    const services = appServices(new FakeMeetingApi());
    const before = services.providers.snapshots();
    window.history.replaceState(null, "", "#/settings");
    render(<App services={services} />);
    await screen.findByRole("heading", { name: "Preview provider adapters" });

    await act(async () => changeUiLocale("zh-CN", undefined));

    expect(services.providers.snapshots()).toEqual(before);
    expect(
      screen.getByRole("heading", { name: "预览版 Provider 适配器" }),
    ).toBeInTheDocument();
  });

  it("does not change imported file metadata when the interface language changes", async () => {
    const imported = syntheticDocument({ originalFilename: "synthetic.txt" });
    const documents = new FakeDocumentApi([imported]);
    renderAt(
      `/meetings/${meetingIds.draft}`,
      new FakeMeetingApi([syntheticMeeting()]),
      undefined,
      documents,
    );
    await screen.findByText(imported.originalFilename);

    await act(async () => changeUiLocale("zh-CN", undefined));

    expect(screen.getByText(imported.originalFilename)).toBeInTheDocument();
    expect(documents.documents).toEqual([imported]);
    expect(documents.selectCalls).toEqual([]);
  });

  it("keeps UI locale separate from meeting and future summary languages", async () => {
    const meeting = syntheticMeeting({ primaryLanguage: "en-AU" });
    renderAt(`/meetings/${meeting.id}`, new FakeMeetingApi([meeting]));
    await screen.findByText("en-AU");

    await act(async () => changeUiLocale("zh-CN", undefined));

    expect(screen.getByText("en-AU")).toBeInTheDocument();
    expect(screen.queryByLabelText(/纪要输出语言/)).not.toBeInTheDocument();
    expect(window.localStorage?.getItem?.(UI_LOCALE_STORAGE_KEY)).not.toBe(
      "en-AU",
    );
  });

  it("adds no mixed component exports that increase Fast Refresh warnings", () => {
    expect(helpSource).toMatch(/export function HelpPage/);
    expect(helpSource).not.toMatch(/export (const|let|class) /);
    expect(settingsSource).not.toMatch(/export (const|let|class) /);
  });
});
