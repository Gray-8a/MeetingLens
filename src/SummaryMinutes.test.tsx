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
import { DocumentApiError } from "./api/document";
import type { MeetingResponse } from "./api/meeting";
import { SummaryApiError, type SummaryRunResponse } from "./api/summary";
import { ServicesProvider, type AppServices } from "./app/services";
import { SummaryResultView } from "./components/SummaryResultView";
import i18n from "./i18n";
import { ProviderRegistry } from "./providers/runtime";
import { FakeAudioTranscriptionApi } from "./test/fakeAudioTranscriptionApi";
import { FakeDocumentApi, syntheticDocument } from "./test/fakeDocumentApi";
import {
  FakeMeetingApi,
  meetingIds,
  syntheticMeeting,
} from "./test/fakeMeetingApi";
import { FakeSecretApi } from "./test/fakeSecretApi";
import {
  FakeSummaryApi,
  syntheticDeepSeekStatus,
  syntheticSummaryContent,
  syntheticSummaryEvidence,
  syntheticSummaryRun,
} from "./test/fakeSummaryApi";
import {
  FakeTranscriptApi,
  syntheticTranscriptSegment,
  syntheticTranscriptSource,
} from "./test/fakeTranscriptApi";

const sourceId = "55555555-5555-4555-8555-555555555555";
const queuedId = "66666666-6666-4666-8666-666666666666";
const newerId = "66666666-6666-4666-8666-666666666667";

function queuedRun(overrides: Partial<SummaryRunResponse> = {}) {
  return syntheticSummaryRun({
    status: "queued",
    content: null,
    contentVersion: null,
    startedAt: null,
    finishedAt: null,
    ...overrides,
  });
}

function renderMinutes({
  summaryApi = new FakeSummaryApi(),
  documentApi = new FakeDocumentApi(),
  path = `/meetings/${meetingIds.draft}/minutes`,
  meetings = [syntheticMeeting()],
}: {
  summaryApi?: FakeSummaryApi;
  documentApi?: FakeDocumentApi;
  path?: string;
  meetings?: MeetingResponse[];
} = {}) {
  const meetingApi = new FakeMeetingApi(meetings);
  const transcriptApi = new FakeTranscriptApi(meetingApi);
  meetings.forEach((meeting, index) => {
    const meetingSourceId =
      index === 0
        ? sourceId
        : `55555555-5555-4555-8555-${String(index + 1).padStart(12, "0")}`;
    transcriptApi.seed(
      syntheticTranscriptSource({
        id: meetingSourceId,
        meetingId: meeting.id,
      }),
      [
        syntheticTranscriptSegment({
          meetingId: meeting.id,
          sourceId: meetingSourceId,
          originalText: "Synthetic transcript input",
        }),
      ],
    );
  });
  const services: AppServices = {
    audioTranscriptionApi: new FakeAudioTranscriptionApi(
      meetingApi,
      transcriptApi,
    ),
    documentApi,
    meetingApi,
    secretApi: new FakeSecretApi(),
    summaryApi,
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
  return {
    documentApi,
    meetingApi,
    services,
    summaryApi,
    transcriptApi,
    view,
  };
}

afterEach(async () => {
  vi.restoreAllMocks();
  window.history.replaceState(null, "", "#/");
  await i18n.changeLanguage("en-US");
});

describe("Summary Minutes workspace", () => {
  it("lists parsed reference documents separately, defaults them off and sends only selected IDs", async () => {
    const parsed = syntheticDocument({
      id: "88888888-8888-4888-8888-888888888888",
      meetingId: meetingIds.draft,
      originalFilename: "synthetic-reference.docx",
      safeExtension: "docx",
      detectedMediaType:
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      status: "parsed",
      parserVersion: "2.117.0",
      pageCount: 3,
    });
    const staged = syntheticDocument({
      id: "99999999-9999-4999-8999-999999999999",
      meetingId: meetingIds.draft,
      originalFilename: "not-parsed.pdf",
      safeExtension: "pdf",
      detectedMediaType: "application/pdf",
    });
    const documentApi = new FakeDocumentApi([parsed, staged]);
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    const user = userEvent.setup();
    renderMinutes({ documentApi, summaryApi });

    const parsedOption = await screen.findByRole("checkbox", {
      name: /synthetic-reference\.docx/i,
    });
    const stagedOption = screen.getByRole("checkbox", {
      name: /not-parsed\.pdf/i,
    });
    expect(parsedOption).toBeEnabled();
    expect(parsedOption).not.toBeChecked();
    expect(stagedOption).toBeDisabled();

    await user.click(parsedOption);
    await user.click(
      screen.getByRole("button", { name: "Generate meeting minutes" }),
    );
    await waitFor(() => expect(summaryApi.createCalls).toHaveLength(1));
    expect(summaryApi.createCalls[0]?.documentIds).toEqual([parsed.id]);
    expect(summaryApi.createCalls[0]?.sourceIds).toEqual([sourceId]);
  });

  it("shows the exact parsed-reference scope in the external disclosure", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    summaryApi.disclosure = {
      ...summaryApi.disclosure,
      documentCount: 1,
      documentChunkCount: 4,
      documentInputBytes: 256,
      inputBytes: summaryApi.disclosure.inputBytes + 256,
    };
    const user = userEvent.setup();
    renderMinutes({ summaryApi });

    await user.click(
      await screen.findByRole("button", { name: "Generate meeting minutes" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Reference documents")).toBeInTheDocument();
    expect(within(dialog).getByText("Reference chunks")).toBeInTheDocument();
    expect(
      within(dialog).getByText("Frozen parsed reference-document text"),
    ).toBeInTheDocument();
    expect(dialog).not.toHaveTextContent("C:\\");
  });

  it("fails closed before creating a run when no DeepSeek key is configured", async () => {
    const { summaryApi } = renderMinutes();

    expect(
      await screen.findByText(/Save a DeepSeek API key/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Generate meeting minutes" }),
    ).toBeDisabled();
    expect(summaryApi.createCalls).toHaveLength(0);
    expect(summaryApi.executeCalls).toHaveLength(0);
  });

  it("keeps completed local results readable when provider status lookup fails", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatusError = new SummaryApiError(
      "internal_error",
      false,
    );
    summaryApi.seedRun(syntheticSummaryRun());
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    expect(await screen.findByText("Synthetic overview")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Settings" })).toBeEnabled();
    expect(
      screen.getByRole("button", { name: "Generate meeting minutes" }),
    ).toBeDisabled();
  });

  it("keeps completed results readable when the optional document list fails", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const documentApi = new FakeDocumentApi();
    documentApi.listError = new DocumentApiError(
      "document_storage_unavailable",
      true,
    );
    renderMinutes({
      documentApi,
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    expect(await screen.findByText("Synthetic overview")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "document_storage_unavailable",
    );
    expect(screen.getAllByText("Revision 1")).toHaveLength(2);
  });

  it("refreshes parsing reference documents without reloading the workspace", async () => {
    const parsing = syntheticDocument({
      id: "88888888-8888-4888-8888-888888888888",
      meetingId: meetingIds.draft,
      originalFilename: "synthetic-parsing.txt",
      status: "parsing",
    });
    const documentApi = new FakeDocumentApi([parsing]);
    renderMinutes({ documentApi });

    expect(
      await screen.findByRole("checkbox", { name: /synthetic-parsing\.txt/i }),
    ).toBeDisabled();
    documentApi.documents = [
      syntheticDocument({
        ...parsing,
        status: "parsed",
        parserVersion: "2.117.0",
        pageCount: 1,
      }),
    ];

    await waitFor(
      () =>
        expect(
          screen.getByRole("checkbox", {
            name: /synthetic-parsing\.txt/i,
          }),
        ).toBeEnabled(),
      { timeout: 2_500 },
    );
  });

  it("shows the combined transcript and document bytes in history", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(
      syntheticSummaryRun({
        documentCount: 1,
        inputDocumentChunkCount: 1,
        documentInputBytes: 64,
      }),
    );
    renderMinutes({ summaryApi });

    expect(await screen.findByText(/192 B/)).toBeInTheDocument();
  });

  it("preserves meeting history when a deep-linked run no longer exists", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const missingId = "77777777-7777-4777-8777-777777777779";
    const getRun = summaryApi.getRun.bind(summaryApi);
    vi.spyOn(summaryApi, "getRun").mockImplementation((request) =>
      request.summaryId === missingId
        ? Promise.reject(new SummaryApiError("summary_run_not_found", false))
        : getRun(request),
    );
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${missingId}`,
    });

    expect(
      await screen.findByRole("heading", {
        name: "Synthetic planning meeting",
      }),
    ).toBeInTheDocument();
    expect(await screen.findByText(/run was not found/i)).toBeInTheDocument();
    expect(screen.getByText("Revision 1")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Open newest loaded run" }),
    );
    expect(await screen.findByText("Synthetic overview")).toBeInTheDocument();
  });

  it("cancels a newly-created queued run when the disclosure is dismissed", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    const user = userEvent.setup();
    renderMinutes({ summaryApi });

    await user.click(
      await screen.findByRole("button", { name: "Generate meeting minutes" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Send this frozen meeting input to DeepSeek?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(summaryApi.cancelCalls).toHaveLength(1));
    expect(summaryApi.executeCalls).toHaveLength(0);
    expect(summaryApi.cancelCalls[0]?.summaryId).toBe(
      summaryApi.createCalls.length === 1 ? summaryApi.runs[0]?.id : "",
    );
  });

  it("cancels a newly-created queued run when disclosure loading fails", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    summaryApi.disclosureError = new SummaryApiError(
      "deepseek_consent_invalid",
      false,
    );
    const user = userEvent.setup();
    renderMinutes({ summaryApi });

    await user.click(
      await screen.findByRole("button", { name: "Generate meeting minutes" }),
    );

    await waitFor(() => expect(summaryApi.createCalls).toHaveLength(1));
    await waitFor(() => expect(summaryApi.cancelCalls).toHaveLength(1));
    expect(summaryApi.executeCalls).toHaveLength(0);
  });

  it("keeps a new run alive across its same-meeting route change while disclosure loads", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    const disclosurePending = deferred<typeof summaryApi.disclosure>();
    vi.spyOn(summaryApi, "getExternalDisclosure").mockReturnValue(
      disclosurePending.promise,
    );
    const user = userEvent.setup();
    renderMinutes({ summaryApi });

    await user.click(
      await screen.findByRole("button", { name: "Generate meeting minutes" }),
    );
    await waitFor(() => expect(summaryApi.createCalls).toHaveLength(1));
    const created = summaryApi.runs.find((run) => run.status === "queued");
    expect(window.location.hash).toContain(created?.id ?? "missing");

    await act(async () => disclosurePending.resolve(summaryApi.disclosure));
    expect(
      await screen.findByRole("dialog", {
        name: "Send this frozen meeting input to DeepSeek?",
      }),
    ).toBeInTheDocument();
    expect(summaryApi.cancelCalls).toHaveLength(0);
  });

  it("cancels a newly-created consent run before navigating to another meeting", async () => {
    const meetingB = syntheticMeeting({
      id: meetingIds.ready,
      title: "Synthetic meeting B",
    });
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    const confirmPending = deferred<typeof summaryApi.disclosure>();
    const confirm = vi
      .spyOn(summaryApi, "confirmExternalDisclosure")
      .mockReturnValue(confirmPending.promise);
    const user = userEvent.setup();
    renderMinutes({
      meetings: [syntheticMeeting(), meetingB],
      summaryApi,
    });

    await user.click(
      await screen.findByRole("button", { name: "Generate meeting minutes" }),
    );
    const dialog = await screen.findByRole("dialog");
    await user.click(
      within(dialog).getByRole("button", {
        name: "Consent this time and continue",
      }),
    );
    expect(confirm).toHaveBeenCalledTimes(1);
    await act(async () => {
      window.history.pushState(null, "", `#/meetings/${meetingB.id}/minutes`);
      window.dispatchEvent(new PopStateEvent("popstate"));
    });

    await waitFor(() => expect(summaryApi.cancelCalls).toHaveLength(1));
    expect(
      await screen.findByRole("heading", { name: "Synthetic meeting B" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
    await act(async () => confirmPending.resolve(summaryApi.disclosure));
    expect(summaryApi.executeCalls).toHaveLength(0);
  });

  it("closes restored-run consent without cancelling it on same-meeting Back navigation", async () => {
    const summaryApi = new FakeSummaryApi();
    const queued = queuedRun();
    const completed = syntheticSummaryRun({
      id: newerId,
      seriesId: "99999999-9999-4999-8999-999999999998",
      createdAt: "2026-08-17T01:00:01.000Z",
      updatedAt: "2026-08-17T01:00:01.000Z",
    });
    summaryApi.seedRun(queued);
    summaryApi.seedRun(completed);
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queued.id}`,
    });

    await user.click(
      await screen.findByRole("button", { name: "Continue generation" }),
    );
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await act(async () => {
      window.history.pushState(
        null,
        "",
        `#/meetings/${meetingIds.draft}/minutes/${completed.id}`,
      );
      window.dispatchEvent(new PopStateEvent("popstate"));
    });

    expect(await screen.findByText("Synthetic overview")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(summaryApi.cancelCalls).toHaveLength(0);
    expect(summaryApi.executeCalls).toHaveLength(0);
  });

  it("never auto-executes a queued run restored from a deep link", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
      consentRemembered: true,
      consentedAt: "2026-08-17T01:00:00.000Z",
    });
    summaryApi.seedRun(queuedRun());
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    expect(
      await screen.findByText(/queued run will never send automatically/i),
    ).toBeInTheDocument();
    await new Promise((resolve) => window.setTimeout(resolve, 25));
    expect(summaryApi.executeCalls).toHaveLength(0);
    expect(summaryApi.getCalls.length).toBeGreaterThanOrEqual(0);
  });

  it("reconciles persisted status after execute rejects", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.providerStatus = syntheticDeepSeekStatus({
      keyConfigured: true,
      executionAvailable: true,
    });
    summaryApi.seedRun(queuedRun());
    summaryApi.executeError = new SummaryApiError("deepseek_timeout", true);
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(
      await screen.findByRole("button", { name: "Continue generation" }),
    );
    const dialog = await screen.findByRole("dialog");
    await user.click(
      within(dialog).getByRole("button", {
        name: "Consent this time and continue",
      }),
    );

    await waitFor(() => expect(summaryApi.executeCalls).toHaveLength(1));
    await waitFor(() => expect(summaryApi.getCalls.length).toBeGreaterThan(0));
    expect(
      await screen.findByText(/did not respond in time/i),
    ).toBeInTheDocument();
  });

  it("does not write a delayed execution result into another meeting page", async () => {
    const meetingB = syntheticMeeting({
      id: meetingIds.ready,
      title: "Synthetic meeting B",
    });
    const summaryApi = new FakeSummaryApi();
    summaryApi.disclosure = {
      ...summaryApi.disclosure,
      requiresConfirmation: false,
    };
    summaryApi.seedRun(queuedRun());
    summaryApi.seedRun(
      syntheticSummaryRun({
        id: newerId,
        meetingId: meetingB.id,
        seriesId: "99999999-9999-4999-8999-999999999998",
        content: syntheticSummaryContent({
          overview: { text: "Meeting B overview", evidenceIds: [] },
        }),
      }),
    );
    const executePending = deferred<SummaryRunResponse>();
    vi.spyOn(summaryApi, "executeRun").mockImplementation((request) => {
      summaryApi.executeCalls.push({ ...request });
      return executePending.promise;
    });
    const user = userEvent.setup();
    renderMinutes({
      meetings: [syntheticMeeting(), meetingB],
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(
      await screen.findByRole("button", { name: "Continue generation" }),
    );
    await waitFor(() => expect(summaryApi.executeCalls).toHaveLength(1));
    await act(async () => {
      window.history.pushState(
        null,
        "",
        `#/meetings/${meetingB.id}/minutes/${newerId}`,
      );
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    expect(
      await screen.findByRole("heading", { name: "Synthetic meeting B" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Meeting B overview")).toBeInTheDocument();

    await act(async () =>
      executePending.resolve(
        syntheticSummaryRun({
          content: syntheticSummaryContent({
            overview: {
              text: "Meeting A delayed overview",
              evidenceIds: [],
            },
          }),
        }),
      ),
    );
    expect(screen.queryByText("Meeting A delayed overview")).toBeNull();
    expect(screen.getByText("Meeting B overview")).toBeInTheDocument();
  });

  it("keeps old revisions and historical Mock runs read-only", async () => {
    const summaryApi = new FakeSummaryApi();
    const oldRun = syntheticSummaryRun();
    const latest = syntheticSummaryRun({
      id: newerId,
      previousSummaryId: oldRun.id,
      revision: 2,
      createdAt: "2026-08-17T01:00:01.000Z",
      updatedAt: "2026-08-17T01:00:01.000Z",
    });
    summaryApi.seedRun(oldRun);
    summaryApi.seedRun(latest);
    const user = userEvent.setup();
    const firstView = renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${oldRun.id}`,
    });

    expect(await screen.findByText(/older revision/i)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Regenerate whole summary" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Open latest revision" }),
    );
    expect(window.location.hash).toBe(
      `#/meetings/${meetingIds.draft}/minutes/${newerId}`,
    );
    firstView.view.unmount();

    const mockApi = new FakeSummaryApi();
    mockApi.seedRun(
      syntheticSummaryRun({
        provider: "deterministic_mock",
        providerMode: "mock",
        model: "meetinglens-deterministic-mock-v1",
        promptVersion: "summary-core-mock-v1",
      }),
    );
    renderMinutes({
      summaryApi: mockApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });
    expect(
      await screen.findByText(/historical deterministic Mock/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Edit text" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Regenerate whole summary" }),
    ).not.toBeInTheDocument();
  });

  it("checks provider readiness before creating a revision", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(
      await screen.findByRole("button", {
        name: "Regenerate whole summary",
      }),
    );

    expect(summaryApi.revisionCalls).toHaveLength(0);
    expect(
      await screen.findByText(/Configure the DeepSeek API key/i),
    ).toBeInTheDocument();
  });

  it("does not treat an old deep-linked run outside the first page as latest", async () => {
    const summaryApi = new FakeSummaryApi();
    const oldRun = syntheticSummaryRun({
      createdAt: "2026-08-16T00:00:00.000Z",
      updatedAt: "2026-08-16T00:00:00.000Z",
    });
    summaryApi.seedRun(oldRun);
    for (let index = 0; index < 21; index += 1) {
      const suffix = String(index + 100).padStart(12, "0");
      summaryApi.seedRun(
        syntheticSummaryRun({
          id: `aaaaaaaa-aaaa-4aaa-8aaa-${suffix}`,
          seriesId: `bbbbbbbb-bbbb-4bbb-8bbb-${suffix}`,
          createdAt: `2026-08-17T01:00:${String(index).padStart(2, "0")}.000Z`,
          updatedAt: `2026-08-17T01:00:${String(index).padStart(2, "0")}.000Z`,
        }),
      );
    }
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${oldRun.id}`,
    });

    expect(
      await screen.findByText(/outside the loaded history/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Regenerate whole summary" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Load more to find latest" }),
    ).toBeEnabled();
  });

  it("disables copy and save while edits are dirty and reports version conflicts", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    summaryApi.updateError = new SummaryApiError(
      "summary_edit_conflict",
      false,
    );
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(await screen.findByRole("button", { name: "Edit text" }));
    const overview = screen.getByRole("textbox", { name: "Overview" });
    await user.clear(overview);
    await user.type(overview, "Edited synthetic overview");
    expect(
      screen.getByRole("button", { name: "Copy Markdown" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Save Markdown" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(
      await screen.findByText(/changed before your edit was saved/i),
    ).toBeInTheDocument();
    expect(summaryApi.updateCalls[0]?.expectedContentVersion).toBe(
      "a".repeat(64),
    );
  });

  it("preserves internal spaces in editable owner and due-date metadata", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(await screen.findByRole("button", { name: "Edit text" }));
    const owner = screen.getByRole("textbox", {
      name: "Owner for action item 1",
    });
    const dueDate = screen.getByRole("textbox", {
      name: "Due date for action item 1",
    });
    await user.clear(owner);
    await user.type(owner, "Jane Doe");
    await user.clear(dueDate);
    await user.type(dueDate, "Next Friday");
    expect(owner).toHaveValue("Jane Doe");
    expect(dueDate).toHaveValue("Next Friday");

    await user.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(summaryApi.updateCalls).toHaveLength(1));
    expect(summaryApi.updateCalls[0]?.editableContent.actionItems[0]).toEqual({
      text: "Synthetic action",
      owner: "Jane Doe",
      dueDate: "Next Friday",
    });
  });

  it("protects a dirty draft from revision, page and browser navigation", async () => {
    const summaryApi = new FakeSummaryApi();
    const firstRun = syntheticSummaryRun({
      createdAt: "2026-08-17T01:00:00.000Z",
      updatedAt: "2026-08-17T01:00:00.000Z",
    });
    const latestRun = syntheticSummaryRun({
      id: newerId,
      previousSummaryId: firstRun.id,
      revision: 2,
      createdAt: "2026-08-17T01:00:01.000Z",
      updatedAt: "2026-08-17T01:00:01.000Z",
    });
    summaryApi.seedRun(firstRun);
    summaryApi.seedRun(latestRun);
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${latestRun.id}`,
    });

    await user.click(await screen.findByRole("button", { name: "Edit text" }));
    const overview = screen.getByRole("textbox", { name: "Overview" });
    await user.clear(overview);
    await user.type(overview, "Protected dirty draft");

    await user.click(screen.getByRole("button", { name: /Revision 1/ }));
    let dialog = await screen.findByRole("alertdialog", {
      name: "Discard unsaved minutes edits?",
    });
    expect(window.location.hash).toContain(latestRun.id);
    await user.click(
      within(dialog).getByRole("button", { name: "Keep editing" }),
    );
    expect(screen.getByRole("textbox", { name: "Overview" })).toHaveValue(
      "Protected dirty draft",
    );

    await user.click(
      screen.getByRole("button", { name: "Back to transcript" }),
    );
    dialog = await screen.findByRole("alertdialog");
    await user.click(
      within(dialog).getByRole("button", { name: "Keep editing" }),
    );
    expect(window.location.hash).toContain(latestRun.id);

    await act(async () => {
      window.history.pushState(null, "", "#/history");
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    dialog = await screen.findByRole("alertdialog");
    const heading = within(dialog).getByRole("heading", {
      name: "Discard unsaved minutes edits?",
    });
    fireEvent.keyDown(heading, { key: "Tab", shiftKey: true });
    expect(
      within(dialog).getByRole("button", { name: "Keep editing" }),
    ).toHaveFocus();
    await user.click(
      within(dialog).getByRole("button", { name: "Discard and continue" }),
    );
    await waitFor(() => expect(window.location.hash).toBe("#/history"));
  });

  it("does not create a new run while result edits are dirty", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const user = userEvent.setup();
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(await screen.findByRole("button", { name: "Edit text" }));
    const overview = screen.getByRole("textbox", { name: "Overview" });
    await user.type(overview, " dirty");

    expect(
      screen.getByRole("button", { name: "Generate meeting minutes" }),
    ).toBeDisabled();
    expect(summaryApi.createCalls).toHaveLength(0);
  });

  it("renders hostile provider text as text and uses exact evidence lookup", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(
      syntheticSummaryRun({
        content: syntheticSummaryContent({
          overview: {
            text: '<img src=x onerror="alert(1)"> Synthetic',
            evidenceIds: ["77777777-7777-4777-8777-777777777777"],
          },
        }),
      }),
    );
    summaryApi.seedEvidence(syntheticSummaryEvidence());
    const user = userEvent.setup();
    const { view } = renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    expect(await screen.findByText(/<img src=x/)).toBeInTheDocument();
    expect(view.container.querySelector("img")).toBeNull();
    await user.click(
      screen.getAllByRole("button", { name: "Open evidence 1" })[0]!,
    );
    const evidence = await screen.findByRole("complementary", {
      name: "Synthetic evidence",
    });
    expect(evidence).toHaveTextContent("Synthetic transcript evidence");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("renders document evidence locators as text without exposing files or trusted HTML", async () => {
    const evidenceId = "77777777-7777-4777-8777-777777777777";
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(
      syntheticSummaryRun({
        content: syntheticSummaryContent({
          overview: {
            text: "Synthetic document-backed overview",
            evidenceIds: [evidenceId],
          },
        }),
      }),
    );
    summaryApi.seedEvidence({
      id: evidenceId,
      summaryId: queuedId,
      meetingId: meetingIds.draft,
      kind: "document",
      documentId: "88888888-8888-4888-8888-888888888888",
      documentChunkId: "99999999-9999-4999-8999-999999999999",
      label: "Synthetic reference evidence",
      parserVersion: "2.117.0",
      pageStart: 2,
      pageEnd: 3,
      sectionPath: ['<img src=x onerror="alert(1)">', "Decisions"],
      tableOrdinal: 0,
      ordinal: 1,
      globalOrdinal: 0,
      excerpt: "Synthetic parsed reference excerpt",
      sourceDeleted: true,
      createdAt: "2026-08-17T00:00:00.000Z",
    });
    const user = userEvent.setup();
    const { view } = renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(
      (await screen.findAllByRole("button", { name: "Open evidence 1" }))[0]!,
    );
    const evidence = await screen.findByRole("complementary", {
      name: "Synthetic reference evidence",
    });
    expect(evidence).toHaveTextContent("Reference document");
    expect(evidence).toHaveTextContent("2–3");
    expect(evidence).toHaveTextContent("<img src=x");
    expect(evidence).toHaveTextContent(
      "original reference document has been deleted",
    );
    expect(view.container.querySelector("img[src='x']")).toBeNull();
    expect(evidence).not.toHaveTextContent("storageRelativePath");
  });

  it("uses one global evidence label map and displays immutable item statuses", async () => {
    const secondEvidenceId = "77777777-7777-4777-8777-777777777778";
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(
      syntheticSummaryRun({
        content: syntheticSummaryContent({
          risks: [
            {
              text: "Synthetic risk",
              evidenceIds: [secondEvidenceId],
            },
          ],
        }),
      }),
    );
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    expect(await screen.findAllByText("[E1]")).toHaveLength(3);
    expect(screen.getByText("[E2]")).toBeInTheDocument();
    expect(screen.getByText("Confirmed")).toBeInTheDocument();
    expect(
      screen.getByText(/Unconfirmed · Synthetic owner/),
    ).toBeInTheDocument();
  });

  it("clears metadata-sensitive preview and evidence without discarding a dirty draft", async () => {
    const summaryApi = new FakeSummaryApi();
    const run = syntheticSummaryRun();
    summaryApi.seedRun(run);
    summaryApi.seedEvidence(syntheticSummaryEvidence());
    const harness = renderMinutes({ summaryApi });
    harness.view.unmount();
    const user = userEvent.setup();
    const callbacks = {
      onCancel: vi.fn().mockResolvedValue(undefined),
      onContinue: vi.fn().mockResolvedValue(undefined),
      onOpenLatest: vi.fn(),
      onRegenerate: vi.fn().mockResolvedValue(undefined),
      onRunUpdated: vi.fn(),
      onDirtyChange: vi.fn(),
    };
    const view = render(
      <ServicesProvider value={harness.services}>
        <SummaryResultView
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={run}
          {...callbacks}
        />
      </ServicesProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Preview Markdown" }));
    await screen.findByRole("heading", { name: "Canonical Markdown preview" });
    await user.click(
      screen.getAllByRole("button", { name: "Open evidence 1" })[0]!,
    );
    await screen.findByText("Synthetic transcript evidence");
    await user.click(screen.getByRole("button", { name: "Edit text" }));
    const overview = screen.getByRole("textbox", { name: "Overview" });
    await user.clear(overview);
    await user.type(overview, "Dirty synthetic draft");

    view.rerender(
      <ServicesProvider value={harness.services}>
        <SummaryResultView
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={{
            ...run,
            isStale: true,
            staleReason: "segment_revised",
            staleAt: "2026-08-17T01:00:01.000Z",
            updatedAt: "2026-08-17T01:00:01.000Z",
          }}
          {...callbacks}
        />
      </ServicesProvider>,
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("heading", {
          name: "Canonical Markdown preview",
        }),
      ).not.toBeInTheDocument(),
    );
    expect(
      screen.queryByText("Synthetic transcript evidence"),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Overview" })).toHaveValue(
      "Dirty synthetic draft",
    );
  });

  it("ignores a delayed edit response after switching result instances", async () => {
    const summaryApi = new FakeSummaryApi();
    const firstRun = syntheticSummaryRun();
    const secondRun = syntheticSummaryRun({
      id: newerId,
      seriesId: "99999999-9999-4999-8999-999999999998",
      content: syntheticSummaryContent({
        overview: { text: "Second run overview", evidenceIds: [] },
      }),
    });
    const updatePending = deferred<SummaryRunResponse>();
    vi.spyOn(summaryApi, "updateContent").mockReturnValue(
      updatePending.promise,
    );
    const onRunUpdated = vi.fn();
    const callbacks = {
      onCancel: vi.fn().mockResolvedValue(undefined),
      onContinue: vi.fn().mockResolvedValue(undefined),
      onDirtyChange: vi.fn(),
      onOpenLatest: vi.fn(),
      onRegenerate: vi.fn().mockResolvedValue(undefined),
      onRunUpdated,
    };
    const user = userEvent.setup();
    const harness = renderMinutes({ summaryApi });
    const services = harness.services;
    harness.view.unmount();
    const view = render(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`${firstRun.id}:${firstRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={firstRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Edit text" }));
    const overview = screen.getByRole("textbox", { name: "Overview" });
    await user.clear(overview);
    await user.type(overview, "Delayed first-run edit");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    view.rerender(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`${secondRun.id}:${secondRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={secondRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );
    await act(async () => updatePending.resolve(firstRun));
    expect(onRunUpdated).not.toHaveBeenCalled();
    expect(screen.getByText("Second run overview")).toBeInTheDocument();
    expect(screen.queryByText("Delayed first-run edit")).toBeNull();
  });

  it("ignores delayed evidence and Markdown after switching result instances", async () => {
    const summaryApi = new FakeSummaryApi();
    const firstRun = syntheticSummaryRun();
    const secondRun = syntheticSummaryRun({
      id: newerId,
      seriesId: "99999999-9999-4999-8999-999999999998",
      content: syntheticSummaryContent({
        overview: { text: "Second run overview", evidenceIds: [] },
      }),
    });
    const evidencePending =
      deferred<ReturnType<typeof syntheticSummaryEvidence>>();
    const markdownPending = deferred<{
      summaryId: string;
      contentVersion: string;
      markdown: string;
      bytes: number;
    }>();
    vi.spyOn(summaryApi, "getEvidence").mockReturnValue(
      evidencePending.promise,
    );
    vi.spyOn(summaryApi, "getMarkdown").mockReturnValue(
      markdownPending.promise,
    );
    const callbacks = {
      onCancel: vi.fn().mockResolvedValue(undefined),
      onContinue: vi.fn().mockResolvedValue(undefined),
      onDirtyChange: vi.fn(),
      onOpenLatest: vi.fn(),
      onRegenerate: vi.fn().mockResolvedValue(undefined),
      onRunUpdated: vi.fn(),
    };
    const harness = renderMinutes({ summaryApi });
    const services = harness.services;
    harness.view.unmount();
    const view = render(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`${firstRun.id}:${firstRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={firstRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );
    const user = userEvent.setup();
    await user.click(
      screen.getAllByRole("button", { name: "Open evidence 1" })[0]!,
    );
    view.rerender(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`${secondRun.id}:${secondRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={secondRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );
    await act(async () => evidencePending.resolve(syntheticSummaryEvidence()));
    expect(screen.queryByText("Synthetic transcript evidence")).toBeNull();

    view.rerender(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`markdown:${firstRun.id}:${firstRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={firstRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Preview Markdown" }));
    view.rerender(
      <ServicesProvider value={services}>
        <SummaryResultView
          key={`markdown:${secondRun.id}:${secondRun.contentVersion}`}
          actionBusy={false}
          isLatestRevision
          latestRevisionKnown
          run={secondRun}
          {...callbacks}
        />
      </ServicesProvider>,
    );
    await act(async () =>
      markdownPending.resolve({
        summaryId: firstRun.id,
        contentVersion: firstRun.contentVersion!,
        markdown: "# Delayed first run\n",
        bytes: 20,
      }),
    );
    expect(screen.queryByText("# Delayed first run")).toBeNull();
    expect(
      screen.queryByRole("heading", { name: "Canonical Markdown preview" }),
    ).toBeNull();
  });

  it("copies and saves canonical Markdown only after explicit clicks", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(syntheticSummaryRun());
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    renderMinutes({
      summaryApi,
      path: `/meetings/${meetingIds.draft}/minutes/${queuedId}`,
    });

    await user.click(
      await screen.findByRole("button", { name: "Copy Markdown" }),
    );
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith(summaryApi.markdown),
    );
    expect(summaryApi.markdownCalls).toHaveLength(1);
    expect(summaryApi.saveCalls).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Save Markdown" }));
    await waitFor(() => expect(summaryApi.saveCalls).toHaveLength(1));
    expect(await screen.findAllByText("Markdown saved.")).toHaveLength(2);
  });

  it("lists multiple active series without treating them as one meeting-wide run", async () => {
    const summaryApi = new FakeSummaryApi();
    summaryApi.seedRun(queuedRun());
    summaryApi.seedRun(
      queuedRun({
        id: newerId,
        seriesId: "99999999-9999-4999-8999-999999999998",
        status: "running",
        startedAt: "2026-08-17T01:00:00.000Z",
      }),
    );
    renderMinutes({ summaryApi });

    const history = await screen.findByRole("heading", {
      name: "Revisions and runs",
    });
    expect(
      within(history.closest("section") as HTMLElement).getAllByText(
        /Revision 1/,
      ),
    ).toHaveLength(2);
    expect(
      within(history.closest("section") as HTMLElement).getAllByText(
        /Detailed · English · 1 source/,
      ),
    ).toHaveLength(2);
  });
});

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, reject, resolve };
}
