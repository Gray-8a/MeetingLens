import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import type {
  RenameSpeakerInSourceResponse,
  TranscriptImportResult,
  TranscriptSegmentResponse,
} from "./api/transcript";
import type { AppServices } from "./app/services";
import { ProviderRegistry } from "./providers/runtime";
import i18n from "./i18n";
import { FakeDocumentApi } from "./test/fakeDocumentApi";
import {
  deferred,
  FakeMeetingApi,
  meetingIds,
  syntheticMeeting,
} from "./test/fakeMeetingApi";
import { FakeSecretApi } from "./test/fakeSecretApi";
import {
  FakeTranscriptApi,
  syntheticTranscriptSegment,
  syntheticTranscriptSource,
} from "./test/fakeTranscriptApi";

const sourceId = "55555555-5555-4555-8555-555555555551";
const secondSourceId = "55555555-5555-4555-8555-555555555552";

function renderHarness(
  path: string,
  meetings: ReturnType<typeof syntheticMeeting>[] = [],
  seed?: (transcriptApi: FakeTranscriptApi) => void,
) {
  const meetingApi = new FakeMeetingApi(meetings);
  const transcriptApi = new FakeTranscriptApi(meetingApi);
  seed?.(transcriptApi);
  const services: AppServices = {
    documentApi: new FakeDocumentApi(),
    meetingApi,
    secretApi: new FakeSecretApi(),
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
    providers: new ProviderRegistry(),
    desktopRuntime: false,
  };
  window.history.replaceState(null, "", `#${path}`);
  const view = render(<App services={services} />);
  return { meetingApi, services, transcriptApi, view };
}

function seedSource(
  api: FakeTranscriptApi,
  overrides: Parameters<typeof syntheticTranscriptSource>[0] = {},
  segments = [
    syntheticTranscriptSegment({
      id: "66666666-6666-4666-8666-666666666661",
      sourceId,
      meetingId: meetingIds.draft,
      speakerLabel: "Speaker 1",
      speakerName: "Alex",
      startOffsetMs: 0,
      endOffsetMs: 5_000,
      originalText: "First synthetic segment",
    }),
    syntheticTranscriptSegment({
      id: "66666666-6666-4666-8666-666666666662",
      sourceId,
      meetingId: meetingIds.draft,
      ordinal: 1,
      speakerLabel: "Speaker 2",
      startOffsetMs: 5_000,
      endOffsetMs: 10_000,
      originalText: "Second synthetic segment",
    }),
  ],
) {
  api.seed(
    syntheticTranscriptSource({
      id: sourceId,
      meetingId: meetingIds.draft,
      displayName: "Synthetic source",
      ...overrides,
    }),
    segments,
  );
}

afterEach(async () => {
  vi.restoreAllMocks();
  window.history.replaceState(null, "", "#/");
  await i18n.changeLanguage("en-US");
});

describe("Summary mode entry", () => {
  it("shows all three modes on Home", async () => {
    renderHarness("/");
    expect(
      await screen.findByRole("heading", { name: "Summary mode" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Translation mode" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Meeting mode" }),
    ).toBeInTheDocument();
  });

  it("makes Summary mode available", async () => {
    renderHarness("/");
    expect(await screen.findByText("Available now")).toBeInTheDocument();
    expect(
      screen.getAllByRole("link", { name: "Add a transcript" }),
    ).toHaveLength(2);
  });

  it("keeps Translation mode disabled", async () => {
    renderHarness("/");
    const card = (
      await screen.findByRole("heading", { name: "Translation mode" })
    ).closest("article");
    expect(
      within(card as HTMLElement).getByRole("button", { name: "Coming soon" }),
    ).toBeDisabled();
  });

  it("keeps Meeting mode disabled", async () => {
    renderHarness("/");
    const card = (
      await screen.findByRole("heading", { name: "Meeting mode" })
    ).closest("article");
    expect(
      within(card as HTMLElement).getByRole("button", { name: "Coming soon" }),
    ).toBeDisabled();
  });

  it("retains the lower-priority advanced meeting entry", async () => {
    renderHarness("/");
    expect(
      await screen.findByRole("link", { name: "Create a meeting manually" }),
    ).toHaveAttribute("href", "#/new");
  });

  it("retains meeting history", async () => {
    renderHarness("/", [syntheticMeeting()]);
    expect(
      await screen.findByText("Synthetic planning meeting"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "View all meetings" }),
    ).toHaveAttribute("href", "#/history");
  });

  it("opens the Summary mode route", async () => {
    const user = userEvent.setup();
    renderHarness("/");
    await user.click(
      (await screen.findAllByRole("link", { name: "Add a transcript" }))[0],
    );
    expect(
      await screen.findByRole("heading", {
        name: "Add a meeting transcript",
        level: 1,
      }),
    ).toBeInTheDocument();
    expect(window.location.hash).toBe("#/summary");
  });

  it("reports database readiness on a direct Summary mode route", async () => {
    renderHarness("/summary");
    const runtimeStatus = await screen.findByLabelText("Runtime status");
    expect(await within(runtimeStatus).findByText("Ready")).toBeInTheDocument();
  });

  it("counts actual UTF-8 bytes", async () => {
    const user = userEvent.setup();
    renderHarness("/summary");
    await user.type(await screen.findByLabelText("Transcript text"), "你好");
    expect(screen.getByText("6 bytes entered")).toBeInTheDocument();
  });

  it("rejects empty pasted text before calling the API", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness("/summary");
    await user.click(
      await screen.findByRole("button", { name: "Save and review" }),
    );
    expect(
      await screen.findByText("The transcript is empty."),
    ).toBeInTheDocument();
    expect(transcriptApi.createPasteCalls).toHaveLength(0);
  });

  it("rejects pasted text over the byte limit", async () => {
    const { transcriptApi } = renderHarness("/summary");
    const input = await screen.findByLabelText("Transcript text");
    fireEvent.change(input, {
      target: { value: "x".repeat(2 * 1024 * 1024 + 1) },
    });
    expect(
      screen.getByRole("button", { name: "Save and review" }),
    ).toBeDisabled();
    expect(transcriptApi.createPasteCalls).toHaveLength(0);
  });

  it("prevents duplicate paste submissions while pending", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness("/summary");
    let resolve!: (result: TranscriptImportResult) => void;
    const pending = new Promise<TranscriptImportResult>((done) => {
      resolve = done;
    });
    const spy = vi
      .spyOn(transcriptApi, "createMeetingFromPaste")
      .mockReturnValue(pending);
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Synthetic pending text",
    );
    const submit = screen.getByRole("button", { name: "Save and review" });
    await user.click(submit);
    expect(submit).toBeDisabled();
    await user.click(submit);
    expect(spy).toHaveBeenCalledTimes(1);
    await act(async () => resolve(cancelledResult()));
  });

  it("clears pasted text and enters the Workspace after success", async () => {
    const user = userEvent.setup();
    renderHarness("/summary");
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Speaker 1: Synthetic success",
    );
    await user.click(screen.getByRole("button", { name: "Save and review" }));
    expect(await screen.findByText("Synthetic success")).toBeInTheDocument();
    expect(window.location.hash).toMatch(/^#\/meetings\/.+\/transcript$/);
    expect(
      screen.queryByDisplayValue("Speaker 1: Synthetic success"),
    ).not.toBeInTheDocument();
  });

  it("clears unsubmitted text on cancel", async () => {
    const user = userEvent.setup();
    renderHarness("/summary");
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Private draft text",
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(
      await screen.findByRole("heading", {
        name: "What would you like to do?",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByDisplayValue("Private draft text"),
    ).not.toBeInTheDocument();
  });

  it("treats native picker cancellation as a no-op", async () => {
    const user = userEvent.setup();
    const { meetingApi, transcriptApi } = renderHarness("/summary");
    transcriptApi.pickerCancelled = true;
    await user.click(
      await screen.findByRole("button", { name: "Choose TXT, SRT or VTT" }),
    );
    await waitFor(() =>
      expect(transcriptApi.createImportCalls).toHaveLength(1),
    );
    expect(meetingApi.meetings).toHaveLength(0);
    expect(window.location.hash).toBe("#/summary");
  });

  it.each([
    ["imported_txt", "TXT transcript"],
    ["imported_srt", "SRT subtitles"],
    ["imported_vtt", "VTT subtitles"],
  ] as const)(
    "imports %s with the native picker",
    async (sourceType, expectedLabel) => {
      const user = userEvent.setup();
      const { transcriptApi } = renderHarness("/summary");
      transcriptApi.nextSelectedType = sourceType;
      await user.click(
        await screen.findByRole("button", { name: "Choose TXT, SRT or VTT" }),
      );
      expect(await screen.findByText(expectedLabel)).toBeInTheDocument();
      expect(transcriptApi.sources[0]?.sourceType).toBe(sourceType);
    },
  );

  it("generates a non-empty localized default title", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness("/summary");
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Synthetic title input",
    );
    await user.click(screen.getByRole("button", { name: "Save and review" }));
    await waitFor(() => expect(transcriptApi.createPasteCalls).toHaveLength(1));
    expect(transcriptApi.createPasteCalls[0].title).toMatch(/^Summary mode · /);
  });

  it("keeps the meeting language baseline independent from Chinese UI locale", async () => {
    await act(async () => i18n.changeLanguage("zh-CN"));
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness("/summary");
    const input = await screen.findByLabelText("转录文本");
    await user.type(input, "合成会议内容");
    await user.click(screen.getByRole("button", { name: "保存并检查" }));
    await waitFor(() => expect(transcriptApi.createPasteCalls).toHaveLength(1));
    expect(transcriptApi.createPasteCalls[0].primaryLanguage).toBe("en-AU");
  });

  it("does not write unsubmitted text to browser storage", async () => {
    const user = userEvent.setup();
    renderHarness("/summary");
    const text = "Sensitive synthetic browser-storage probe";
    await user.type(await screen.findByLabelText("Transcript text"), text);
    expect(JSON.stringify(window.localStorage)).not.toContain(text);
    expect(JSON.stringify(window.sessionStorage)).not.toContain(text);
  });

  it("does not log pasted text", async () => {
    const log = vi.spyOn(console, "log").mockImplementation(() => undefined);
    const user = userEvent.setup();
    renderHarness("/summary");
    const text = "Synthetic console privacy probe";
    await user.type(await screen.findByLabelText("Transcript text"), text);
    expect(log.mock.calls.flat().join(" ")).not.toContain(text);
  });

  it("releases unsubmitted text when the page unmounts", async () => {
    const user = userEvent.setup();
    const { view } = renderHarness("/summary");
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Unmount-only synthetic text",
    );
    view.unmount();
    expect(
      screen.queryByDisplayValue("Unmount-only synthetic text"),
    ).not.toBeInTheDocument();
  });
});

describe("Existing meeting transcript input", () => {
  it("shows transcript actions on Meeting details", async () => {
    renderHarness(`/meetings/${meetingIds.draft}`, [syntheticMeeting()]);
    expect(
      (await screen.findAllByText("Transcript sources")).length,
    ).toBeGreaterThan(0);
    expect(screen.getByLabelText("Transcript text")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Choose TXT, SRT or VTT" }),
    ).toBeInTheDocument();
  });

  it("adds pasted text to an existing meeting", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(`/meetings/${meetingIds.draft}`, [
      syntheticMeeting(),
    ]);
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Speaker 1: Existing meeting text",
    );
    await user.click(screen.getByRole("button", { name: "Save and review" }));
    await waitFor(() => expect(transcriptApi.addPasteCalls).toHaveLength(1));
    expect(transcriptApi.addPasteCalls[0].meetingId).toBe(meetingIds.draft);
    expect(await screen.findByText("1 source")).toBeInTheDocument();
  });

  it("imports into an existing meeting", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(`/meetings/${meetingIds.draft}`, [
      syntheticMeeting(),
    ]);
    transcriptApi.nextSelectedType = "imported_srt";
    await user.click(
      await screen.findByRole("button", { name: "Choose TXT, SRT or VTT" }),
    );
    await waitFor(() => expect(transcriptApi.addImportCalls).toHaveLength(1));
    expect(transcriptApi.sources[0]?.sourceType).toBe("imported_srt");
  });

  it("shows duplicate imports without adding another source", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    transcriptApi.nextDuplicate = true;
    await user.type(
      await screen.findByLabelText("Transcript text"),
      "Duplicate synthetic text",
    );
    await user.click(screen.getByRole("button", { name: "Save and review" }));
    expect(await screen.findByText("Already added")).toBeInTheDocument();
    expect(transcriptApi.sources).toHaveLength(1);
  });

  it.each(["recording", "ready"] as const)(
    "allows transcript input for %s meetings",
    async (status) => {
      renderHarness(`/meetings/${meetingIds.draft}`, [
        syntheticMeeting({ status }),
      ]);
      expect(await screen.findByLabelText("Transcript text")).toBeEnabled();
    },
  );

  it.each([
    "processing",
    "failed",
    "archived",
    "deleting",
    "delete_failed",
  ] as const)(
    "disables transcript mutation for %s meetings",
    async (status) => {
      renderHarness(`/meetings/${meetingIds.draft}`, [
        syntheticMeeting({ status }),
      ]);
      expect(
        await screen.findByText(
          "Transcripts cannot be changed in the current meeting state.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.queryByLabelText("Transcript text"),
      ).not.toBeInTheDocument();
    },
  );
});

describe("Transcript Workspace", () => {
  it("lists safe source metadata", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, { originalFilename: "synthetic.txt" }),
    );
    expect(
      (await screen.findAllByText("Synthetic source")).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText("synthetic.txt")).toBeInTheDocument();
    expect(screen.getAllByText("2 segments").length).toBeGreaterThan(0);
    expect(screen.getByText("2 speaker labels")).toBeInTheDocument();
  });

  it.each([
    ["pasted_text", "Pasted notes"],
    ["imported_txt", "TXT transcript"],
    ["imported_srt", "SRT subtitles"],
    ["imported_vtt", "VTT subtitles"],
    ["live_asr", "Live transcript"],
    ["platform_export", "Platform transcript"],
  ] as const)("localizes %s source metadata", async (sourceType, label) => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, { sourceType }),
    );
    expect(await screen.findByText(label)).toBeInTheDocument();
  });

  it("shows a real source time range", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect(await screen.findByText("00:00:00 – 00:00:10")).toBeInTheDocument();
  });

  it("does not fabricate a missing start bound", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) =>
        seedSource(api, {}, [
          syntheticTranscriptSegment({
            id: segmentUuid(10),
            sourceId,
            meetingId: meetingIds.draft,
            startOffsetMs: null,
            endOffsetMs: 10_000,
          }),
        ]),
    );
    expect(await screen.findByText("00:00:10")).toBeInTheDocument();
  });

  it("renders hostile filenames as plain text", async () => {
    const hostile = "<img src=x onerror=alert(1)>.txt";
    const { view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, { originalFilename: hostile }),
    );
    expect(await screen.findByText(hostile)).toBeInTheDocument();
    expect(view.container.querySelector("img")).toBeNull();
  });

  it("cancels source deletion", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      await screen.findByRole("button", { name: "Delete source" }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(transcriptApi.deleteCalls).toHaveLength(0);
    expect(screen.getAllByText("Synthetic source").length).toBeGreaterThan(0);
  });

  it("deletes a source after confirmation", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      await screen.findByRole("button", { name: "Delete source" }),
    );
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));
    await waitFor(() => expect(transcriptApi.deleteCalls).toHaveLength(1));
    expect(await screen.findByText("No transcript input")).toBeInTheDocument();
  });

  it("keeps a source visible when deletion fails", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      await screen.findByRole("button", { name: "Delete source" }),
    );
    await screen.findByText("First synthetic segment");
    await screen.findAllByLabelText("Speaker name");
    vi.spyOn(transcriptApi, "deleteSource").mockRejectedValue(
      new Error("private backend detail"),
    );
    await user.click(screen.getByRole("button", { name: "Confirm delete" }));
    expect(
      await screen.findByText("MeetingLens could not complete that action."),
    ).toBeInTheDocument();
    expect(screen.getAllByText("Synthetic source").length).toBeGreaterThan(0);
    expect(
      screen.queryByText("private backend detail"),
    ).not.toBeInTheDocument();
  });

  it("loads segments in bounded pages of 100", async () => {
    const segments = Array.from({ length: 205 }, (_, ordinal) =>
      syntheticTranscriptSegment({
        id: segmentUuid(ordinal),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal,
        originalText: `Synthetic page segment ${ordinal}`,
      }),
    );
    const { transcriptApi, view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, {}, segments),
    );
    await screen.findByText("Synthetic page segment 0");
    expect(view.container.querySelectorAll(".transcript-segment")).toHaveLength(
      100,
    );
    expect(transcriptApi.listSegmentCalls[0].options?.limit).toBe(100);
    expect(
      screen.queryByText("Synthetic page segment 200"),
    ).not.toBeInTheDocument();
  });

  it("shows the selected source name in every segment metadata row", async () => {
    const { view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await screen.findByText("First synthetic segment");
    const rows = [
      ...view.container.querySelectorAll(".transcript-segment__meta"),
    ];
    expect(rows).toHaveLength(2);
    for (const row of rows) {
      expect(
        within(row as HTMLElement).getByText("Synthetic source"),
      ).toBeInTheDocument();
    }
  });

  it("moves to the next bounded segment page", async () => {
    const user = userEvent.setup();
    const segments = Array.from({ length: 101 }, (_, ordinal) =>
      syntheticTranscriptSegment({
        id: segmentUuid(ordinal),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal,
        originalText: `Paged text ${ordinal}`,
      }),
    );
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, {}, segments),
    );
    await screen.findByText("Paged text 0");
    const nextButton = await waitFor(() => {
      const button = screen
        .getAllByRole("button", { name: "Next page" })
        .find((candidate) => !candidate.hasAttribute("disabled"));
      expect(button).toBeDefined();
      return button as HTMLButtonElement;
    });
    await user.click(nextButton);
    expect(await screen.findByText("Paged text 100")).toBeInTheDocument();
    expect(screen.getAllByText("Page 2").length).toBeGreaterThan(0);
  });

  it("returns to the previous segment page", async () => {
    const user = userEvent.setup();
    const segments = Array.from({ length: 101 }, (_, ordinal) =>
      syntheticTranscriptSegment({
        id: segmentUuid(ordinal),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal,
        originalText: `Back page text ${ordinal}`,
      }),
    );
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, {}, segments),
    );
    await screen.findByText("Back page text 0");
    const nextButton = await waitFor(() => {
      const button = screen
        .getAllByRole("button", { name: "Next page" })
        .find((candidate) => !candidate.hasAttribute("disabled"));
      expect(button).toBeDefined();
      return button as HTMLButtonElement;
    });
    await user.click(nextButton);
    await screen.findByText("Back page text 100");
    const previousButton = screen
      .getAllByRole("button", { name: "Previous page" })
      .find((candidate) => !candidate.hasAttribute("disabled"));
    expect(previousButton).toBeDefined();
    await user.click(previousButton as HTMLButtonElement);
    expect(await screen.findByText("Back page text 0")).toBeInTheDocument();
  });

  it("formats segment offsets without inventing absolute time", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect((await screen.findAllByText("00:00:00")).length).toBeGreaterThan(0);
  });

  it("uses paragraph order when a segment has no timestamp", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) =>
        seedSource(api, {}, [
          syntheticTranscriptSegment({
            id: segmentUuid(8),
            sourceId,
            meetingId: meetingIds.draft,
            ordinal: 8,
            startOffsetMs: null,
            endOffsetMs: null,
          }),
        ]),
    );
    expect(await screen.findByText("Paragraph 9")).toBeInTheDocument();
  });

  it("prefers speakerName over speakerLabel", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect(await screen.findByText("Alex")).toBeInTheDocument();
  });

  it("falls back to speakerLabel", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect((await screen.findAllByText("Speaker 2")).length).toBeGreaterThan(0);
  });

  it("shows Unknown speaker when both names are absent", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) =>
        seedSource(api, {}, [
          syntheticTranscriptSegment({
            id: segmentUuid(2),
            sourceId,
            meetingId: meetingIds.draft,
            speakerLabel: null,
            speakerName: null,
          }),
        ]),
    );
    expect(await screen.findByText("Unknown speaker")).toBeInTheDocument();
  });

  it("opens and cancels segment editing", async () => {
    const user = userEvent.setup();
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      (await screen.findAllByRole("button", { name: "Edit segment" }))[0],
    );
    expect(
      screen.getByDisplayValue("First synthetic segment"),
    ).toBeInTheDocument();
    await user.click(screen.getAllByRole("button", { name: "Cancel" })[0]);
    expect(
      screen.queryByDisplayValue("First synthetic segment"),
    ).not.toBeInTheDocument();
  });

  it("saves text and speaker corrections with expected revision", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      (await screen.findAllByRole("button", { name: "Edit segment" }))[0],
    );
    const text = screen.getByDisplayValue("First synthetic segment");
    const editForm = text.closest("form");
    expect(editForm).not.toBeNull();
    const speaker = within(editForm as HTMLFormElement).getByLabelText(
      "Speaker name",
    );
    await user.clear(speaker);
    await user.type(speaker, "Taylor");
    await user.clear(text);
    await user.type(text, "Corrected synthetic segment");
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(
      await screen.findByText("Corrected synthetic segment"),
    ).toBeInTheDocument();
    expect(transcriptApi.updateCalls[0]).toMatchObject({
      expectedRevision: 1,
      speakerName: "Taylor",
    });
  });

  it("locks source selection and segment pagination while an update is pending", async () => {
    const user = userEvent.setup();
    const segments = Array.from({ length: 101 }, (_, ordinal) =>
      syntheticTranscriptSegment({
        id: segmentUuid(ordinal),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal,
        speakerLabel: `Speaker ${ordinal.toString().padStart(3, "0")}`,
        originalText: `Locked update segment ${ordinal}`,
      }),
    );
    const { transcriptApi, view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => {
        api.seed(
          syntheticTranscriptSource({
            id: secondSourceId,
            meetingId: meetingIds.draft,
            displayName: "Other synthetic source",
          }),
          [
            syntheticTranscriptSegment({
              id: segmentUuid(900),
              sourceId: secondSourceId,
              meetingId: meetingIds.draft,
              originalText: "Other source segment",
            }),
          ],
        );
        seedSource(api, {}, segments);
      },
    );
    await screen.findByText("Locked update segment 0");
    const pending = deferred<TranscriptSegmentResponse>();
    vi.spyOn(transcriptApi, "updateSegment").mockReturnValueOnce(
      pending.promise,
    );
    await user.click(
      view.container.querySelector(
        '[data-acceptance="edit-transcript-segment"]',
      ) as HTMLButtonElement,
    );
    const editor = screen.getByDisplayValue("Locked update segment 0");
    fireEvent.change(editor, {
      target: { value: "Deferred corrected segment" },
    });
    await user.click(
      view.container.querySelector(
        '[data-acceptance="save-transcript-segment"]',
      ) as HTMLButtonElement,
    );

    const sourceButtons = [
      ...view.container.querySelectorAll<HTMLButtonElement>(
        '[data-acceptance="view-transcript-source"]',
      ),
    ];
    await waitFor(() =>
      expect(
        sourceButtons.every((button) => button.hasAttribute("disabled")),
      ).toBe(true),
    );
    const nextPage = view.container.querySelector(
      '[data-acceptance="transcript-segments-next"]',
    ) as HTMLButtonElement;
    expect(nextPage).toBeDisabled();
    expect(
      view.container.querySelector("#transcript-paste-existing"),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Choose TXT, SRT or VTT" }),
    ).toBeDisabled();
    const otherSource = sourceButtons.find(
      (button) => button.getAttribute("aria-pressed") === "false",
    );
    expect(otherSource).toBeDefined();
    await user.click(otherSource as HTMLButtonElement);
    await user.click(nextPage);
    expect(screen.getAllByText("Page 1").length).toBeGreaterThan(0);
    expect(screen.queryByText("Other source segment")).not.toBeInTheDocument();

    await act(async () => {
      pending.resolve({
        ...segments[0],
        originalText: "Deferred corrected segment",
        revision: 2,
      });
    });
    expect(
      await screen.findByText("Deferred corrected segment"),
    ).toBeInTheDocument();
  }, 15_000);

  it("locks source selection and speaker pagination while a rename is pending", async () => {
    const user = userEvent.setup();
    const segments = Array.from({ length: 101 }, (_, ordinal) =>
      syntheticTranscriptSegment({
        id: segmentUuid(ordinal),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal,
        speakerLabel: `Speaker ${ordinal.toString().padStart(3, "0")}`,
        speakerName: ordinal === 0 ? "Alex" : null,
        originalText: `Locked rename segment ${ordinal}`,
      }),
    );
    const { transcriptApi, view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => {
        api.seed(
          syntheticTranscriptSource({
            id: secondSourceId,
            meetingId: meetingIds.draft,
            displayName: "Other synthetic source",
          }),
          [
            syntheticTranscriptSegment({
              id: segmentUuid(901),
              sourceId: secondSourceId,
              meetingId: meetingIds.draft,
              originalText: "Other source segment",
            }),
          ],
        );
        seedSource(api, {}, segments);
      },
    );
    await screen.findByText("Locked rename segment 0");
    const firstSpeaker = view.container.querySelector(
      ".speaker-group-row input",
    ) as HTMLInputElement;
    fireEvent.change(firstSpeaker, { target: { value: "Jordan" } });
    await user.click(
      view.container.querySelector(
        '[data-acceptance="rename-transcript-speaker"]',
      ) as HTMLButtonElement,
    );
    const pending = deferred<RenameSpeakerInSourceResponse>();
    vi.spyOn(transcriptApi, "renameSpeakerInSource").mockReturnValueOnce(
      pending.promise,
    );
    await user.click(
      view.container.querySelector(
        '[data-acceptance="confirm-transcript-speaker-rename"]',
      ) as HTMLButtonElement,
    );

    const sourceButtons = [
      ...view.container.querySelectorAll<HTMLButtonElement>(
        '[data-acceptance="view-transcript-source"]',
      ),
    ];
    await waitFor(() =>
      expect(
        sourceButtons.every((button) => button.hasAttribute("disabled")),
      ).toBe(true),
    );
    const nextSpeakers = view.container.querySelector(
      '[data-acceptance="transcript-speakers-next"]',
    ) as HTMLButtonElement;
    expect(nextSpeakers).toBeDisabled();
    const otherSource = sourceButtons.find(
      (button) => button.getAttribute("aria-pressed") === "false",
    );
    expect(otherSource).toBeDefined();
    await user.click(otherSource as HTMLButtonElement);
    await user.click(nextSpeakers);
    expect(screen.queryByText("Other source segment")).not.toBeInTheDocument();

    await act(async () => {
      pending.resolve({ sourceId, updatedCount: 1 });
    });
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Confirm" }),
      ).not.toBeInTheDocument(),
    );
    const selectedSourceButton = [
      ...view.container.querySelectorAll<HTMLButtonElement>(
        '[data-acceptance="view-transcript-source"]',
      ),
    ].find((button) => button.getAttribute("aria-pressed") === "true");
    expect(selectedSourceButton?.closest("li")).toHaveTextContent(
      "Synthetic source",
    );
  }, 15_000);

  it("reloads after a revision conflict before saving the new revision", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      (await screen.findAllByRole("button", { name: "Edit segment" }))[0],
    );
    const stored = transcriptApi.segments.get(sourceId)?.[0];
    if (stored) stored.revision = 2;
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(
      await screen.findByText(
        "This content was updated elsewhere. Reload it before trying again.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByDisplayValue("First synthetic segment"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reload segment" }));
    await waitFor(() =>
      expect(
        screen.queryByDisplayValue("First synthetic segment"),
      ).not.toBeInTheDocument(),
    );
    await user.click(
      (await screen.findAllByRole("button", { name: "Edit segment" }))[0],
    );
    const reloadedText = screen.getByDisplayValue("First synthetic segment");
    await user.clear(reloadedText);
    await user.type(reloadedText, "Resolved synthetic segment");
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(
      await screen.findByText("Resolved synthetic segment"),
    ).toBeInTheDocument();
    expect(
      transcriptApi.updateCalls[transcriptApi.updateCalls.length - 1],
    ).toMatchObject({
      expectedRevision: 2,
      originalText: "Resolved synthetic segment",
    });
  });

  it("rejects an empty corrected segment", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    await user.click(
      (await screen.findAllByRole("button", { name: "Edit segment" }))[0],
    );
    await user.clear(screen.getByDisplayValue("First synthetic segment"));
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(
      await screen.findByText("The transcript is empty."),
    ).toBeInTheDocument();
    expect(transcriptApi.updateCalls).toHaveLength(0);
  });

  it("shows the affected count for a speaker label", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect(
      (await screen.findAllByText("Affects 1 segment")).length,
    ).toBeGreaterThan(0);
  });

  it("cancels a batch speaker rename", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    const input = (await screen.findAllByLabelText("Speaker name"))[0];
    await waitFor(() => expect(input).toHaveValue("Alex"));
    await user.clear(input);
    await user.type(input, "Jordan");
    await user.click(screen.getAllByRole("button", { name: "Apply name" })[0]);
    const cancelButtons = screen.getAllByRole("button", { name: "Cancel" });
    await user.click(cancelButtons[cancelButtons.length - 1]);
    expect(transcriptApi.renameCalls).toHaveLength(0);
  });

  it("renames matching speakers only inside the selected source", async () => {
    const user = userEvent.setup();
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    const input = (await screen.findAllByLabelText("Speaker name"))[0];
    await waitFor(() => expect(input).toHaveValue("Alex"));
    await user.clear(input);
    await user.type(input, "Jordan");
    await user.click(screen.getAllByRole("button", { name: "Apply name" })[0]);
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(transcriptApi.renameCalls).toHaveLength(1));
    expect(transcriptApi.renameCalls[0]).toMatchObject({
      sourceId,
      speakerLabel: "Speaker 1",
      speakerName: "Jordan",
    });
  });

  it("allows clearing a mixed speaker-name group", async () => {
    const segments = [
      syntheticTranscriptSegment({
        id: segmentUuid(1),
        sourceId,
        meetingId: meetingIds.draft,
        speakerLabel: "Speaker 1",
        speakerName: "Alex",
      }),
      syntheticTranscriptSegment({
        id: segmentUuid(2),
        sourceId,
        meetingId: meetingIds.draft,
        ordinal: 1,
        speakerLabel: "Speaker 1",
        speakerName: "Taylor",
      }),
    ];
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, {}, segments),
    );
    expect(
      await screen.findByText(/currently contain different display names/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear name" })).toBeEnabled();
  });

  it("shows empty input readiness", async () => {
    renderHarness(`/meetings/${meetingIds.draft}/transcript`, [
      syntheticMeeting(),
    ]);
    expect(
      await screen.findByText("No usable transcript input yet."),
    ).toBeInTheDocument();
    expect(screen.getByText("No transcript input")).toBeInTheDocument();
  });

  it("shows ready input status without claiming AI ran", async () => {
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    expect(
      await screen.findByText(
        "The transcript is prepared for the upcoming minutes workflow.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /transcript is ready/i }),
    ).toBeDisabled();
  });

  it("does not call external network from the disabled summary action", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValue(new Error("network disabled"));
    renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api),
    );
    const button = await screen.findByRole("button", {
      name: /transcript is ready/i,
    });
    fireEvent.click(button);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("ignores a stale segment response after changing sources", async () => {
    const { transcriptApi } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => {
        seedSource(api);
        api.seed(
          syntheticTranscriptSource({
            id: secondSourceId,
            meetingId: meetingIds.draft,
            displayName: "Slow source",
          }),
          [
            syntheticTranscriptSegment({
              id: segmentUuid(50),
              sourceId: secondSourceId,
              meetingId: meetingIds.draft,
              originalText: "Stale slow text",
            }),
          ],
        );
      },
    );
    const original = transcriptApi.listSegments.bind(transcriptApi);
    let resolveSlow!: (
      value: Awaited<ReturnType<typeof transcriptApi.listSegments>>,
    ) => void;
    vi.spyOn(transcriptApi, "listSegments").mockImplementation(
      (meetingId, requestedSourceId, options) => {
        if (requestedSourceId === secondSourceId) {
          return new Promise((resolve) => {
            resolveSlow = resolve;
          });
        }
        return original(meetingId, requestedSourceId, options);
      },
    );
    await userEvent.click(
      (await screen.findAllByRole("button", { name: "View" }))[0],
    );
    expect(
      await screen.findByText("First synthetic segment"),
    ).toBeInTheDocument();
    await act(async () =>
      resolveSlow({
        items: [
          syntheticTranscriptSegment({ originalText: "Stale slow text" }),
        ],
        nextCursor: null,
        hasMore: false,
        totalCount: 1,
      }),
    );
    expect(screen.queryByText("Stale slow text")).not.toBeInTheDocument();
  });
});

describe("Summary mode localization and security surface", () => {
  it("renders the Summary flow in Simplified Chinese", async () => {
    await act(async () => i18n.changeLanguage("zh-CN"));
    renderHarness("/summary");
    expect(
      await screen.findByRole("heading", { name: "添加会议转录", level: 1 }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "选择 TXT、SRT 或 VTT" }),
    ).toBeInTheDocument();
  });

  it("updates Help to describe transcript input and the AI limitation", async () => {
    renderHarness("/help");
    expect(
      await screen.findByText(
        /paste or import UTF-8 TXT, SRT and VTT transcripts/,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Real AI minutes, DeepSeek calls/),
    ).toBeInTheDocument();
  });

  it("does not expose source paths or hashes in the Workspace", async () => {
    const { view } = renderHarness(
      `/meetings/${meetingIds.draft}/transcript`,
      [syntheticMeeting()],
      (api) => seedSource(api, { originalFilename: "synthetic.txt" }),
    );
    await screen.findByText("synthetic.txt");
    expect(view.container.textContent).not.toContain("C:\\");
    expect(view.container.textContent).not.toMatch(/[a-f0-9]{64}/);
  });

  it("keeps Tauri invoke out of Summary and Workspace pages", async () => {
    const [summarySource, workspaceSource, inputSource] = await Promise.all([
      import("./pages/SummaryModeInputPage.tsx?raw").then(
        (module) => module.default,
      ),
      import("./pages/TranscriptWorkspacePage.tsx?raw").then(
        (module) => module.default,
      ),
      import("./components/TranscriptInputPanel.tsx?raw").then(
        (module) => module.default,
      ),
    ]);
    expect(`${summarySource}${workspaceSource}${inputSource}`).not.toContain(
      "invoke(",
    );
  });

  it("does not use dangerous HTML for transcript content", async () => {
    const sources = await Promise.all([
      import("./components/TranscriptSourceList.tsx?raw").then(
        (module) => module.default,
      ),
      import("./components/TranscriptSegmentPanel.tsx?raw").then(
        (module) => module.default,
      ),
    ]);
    expect(sources.join("\n")).not.toContain("dangerouslySetInnerHTML");
  });
});

function cancelledResult(): TranscriptImportResult {
  return {
    cancelled: true,
    duplicate: false,
    meeting: null,
    source: null,
    segmentCount: 0,
  };
}

function segmentUuid(ordinal: number): string {
  return `66666666-6666-4666-8666-${(ordinal + 100).toString(16).padStart(12, "0")}`;
}
