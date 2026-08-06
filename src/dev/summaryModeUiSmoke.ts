import { MeetingApiError, meetingApi } from "../api/meeting";
import { TranscriptApiError, transcriptApi } from "../api/transcript";
import i18n from "../i18n";

const smokeStateKey = "meetinglens.summary-mode-ui-smoke.state.v1";
const meetingTitlePrefix = "Synthetic summary UI smoke";
const segmentCount = 205;
const firstPageSize = 100;
const individualSpeakerName = "Synthetic Individual Speaker";
const batchSpeakerName = "Synthetic Batch Speaker";
const correctedSegmentText = "Corrected synthetic summary UI segment";
const concurrentSpeakerName = "Synthetic Concurrent Writer";
const concurrentSegmentText = "Concurrent synthetic summary UI segment";
const recoveredSpeakerName = "Synthetic Reloaded Speaker";
const recoveredSegmentText = "Recovered synthetic summary UI segment";
const chineseSegmentText = "这是明显的合成中文会议片段";
const recordingLifecycleTranscript =
  "Speaker Recording: Synthetic recording lifecycle source";
const readyLifecycleTranscript =
  "Speaker Ready: Synthetic ready lifecycle source";
const pollIntervalMs = 50;
const defaultTimeoutMs = 30_000;

interface SmokeState {
  runId: string;
  phase: "running" | "restart";
  meetingId: string | null;
}

interface NetworkRecorder {
  assertNoExternalRequests: () => void;
  restore: () => void;
}

export async function runSummaryModeUiSmoke(): Promise<void> {
  const networkRecorder = installNetworkRecorder();
  let state = readState();
  try {
    if (state?.phase === "restart") {
      await runRestartPass(state);
      await assertNoSyntheticBrowserPersistence();
      networkRecorder.assertNoExternalRequests();
      clearState();
      mark("passed");
      return;
    }

    if (state !== null) {
      await cleanUpOwnedMeeting(state);
      clearState();
    }

    state = {
      runId: crypto.randomUUID(),
      phase: "running",
      meetingId: null,
    };
    writeState(state);
    await runFirstPass(state, networkRecorder);
  } catch (error: unknown) {
    if (state !== null) {
      try {
        await cleanUpOwnedMeeting(state);
        clearState();
      } catch {
        // Keep only the run marker, phase and meeting UUID for a safe retry.
      }
    }
    mark(`failed:${safeErrorCode(error)}`);
  } finally {
    networkRecorder.restore();
  }
}

async function runFirstPass(
  state: SmokeState,
  networkRecorder: NetworkRecorder,
): Promise<void> {
  const title = meetingTitle(state.runId);
  await verifyReadOnlySettingsAndLocales();
  await openHomeThroughNavigation();
  verifyThreeModeHome();

  click(await enabledElement('[data-acceptance="summary-mode-card-action"]'));
  await element('[data-acceptance="summary-mode-input"]');

  setControlValue(await input("#summary-meeting-title"), title);
  setControlValue(
    await textarea("#transcript-paste-create"),
    syntheticTranscript(),
  );
  await nextFrame();
  click(await enabledElement('[data-acceptance="submit-paste-create"]'));

  const meetingId = await waitFor(
    () => meetingIdFromHash(),
    "summary_ui_meeting_route_missing",
  );
  state.meetingId = meetingId;
  writeState(state);

  await verifyWorkspace(title, "Synthetic UI segment 001");
  await verifyPagination();
  await editFirstSegment();
  await renameSpeakerGroup();
  await verifyDuplicatePaste(meetingId);
  await verifyRevisionConflictRecovery(meetingId);
  await verifyLifecyclePastes(meetingId);
  await assertNoSyntheticBrowserPersistence();
  networkRecorder.assertNoExternalRequests();

  state.phase = "restart";
  writeState(state);
  mark("restart");
}

async function runRestartPass(state: SmokeState): Promise<void> {
  requireCondition(state.meetingId !== null, "summary_ui_meeting_id_missing");
  const title = meetingTitle(state.runId);

  await openHistoryThroughNavigation();
  const row = await element<HTMLElement>(
    `[data-testid="meeting-row-${state.meetingId}"]`,
  );
  requireCondition(
    row.querySelector("h3")?.textContent?.trim() === title,
    "summary_ui_owned_title_mismatch",
  );
  click(await enabledElement("a", row));

  const detail = await element<HTMLElement>(
    '[data-acceptance="meeting-detail"]',
  );
  requireCondition(
    detail.querySelector("h1")?.textContent?.trim() === title,
    "summary_ui_detail_title_mismatch",
  );
  const transcriptPanel = await element<HTMLElement>(
    '[data-acceptance="meeting-transcript-panel"]',
  );
  click(await enabledElement("a", transcriptPanel));

  await element('[data-acceptance="transcript-workspace"]');
  await selectOriginalSource();
  await verifyWorkspace(title, recoveredSegmentText);
  await verifyPersistedCorrections();
  await deleteOneSourceThroughUi(state.meetingId);
  await deleteMeetingThroughUi(state.meetingId, title);
  await expectMeetingMissing(state.meetingId);
}

async function openHomeThroughNavigation(): Promise<void> {
  click(await enabledElement('a[href="#/"]'));
  await element(".mode-grid");
}

async function openHistoryThroughNavigation(): Promise<void> {
  click(await enabledElement('a[href="#/history"]'));
  await element(".meeting-list");
}

function verifyThreeModeHome(): void {
  const grid = requireElement<HTMLElement>(".mode-grid");
  const cards = grid.querySelectorAll<HTMLElement>(".mode-card");
  const available = grid.querySelectorAll<HTMLElement>(".mode-card--available");
  const comingSoon = grid.querySelectorAll<HTMLElement>(
    ".mode-card--coming-soon",
  );
  requireCondition(cards.length === 3, "summary_ui_mode_count_mismatch");
  requireCondition(
    available.length === 1,
    "summary_ui_available_mode_mismatch",
  );
  requireCondition(comingSoon.length === 2, "summary_ui_locked_mode_mismatch");
  requireCondition(
    available[0]?.querySelector('a[href="#/summary"]') !== null,
    "summary_ui_summary_entry_missing",
  );
  for (const card of comingSoon) {
    requireCondition(
      card.querySelector("a") === null &&
        card.querySelector<HTMLButtonElement>("button")?.disabled === true,
      "summary_ui_future_mode_enabled",
    );
  }
}

async function verifyWorkspace(
  title: string,
  expectedFirstText: string,
): Promise<void> {
  const workspace = await element<HTMLElement>(
    '[data-acceptance="transcript-workspace"]',
  );
  requireCondition(
    workspace.querySelector("h1")?.textContent?.trim() === title,
    "summary_ui_workspace_title_mismatch",
  );
  await element(".transcript-source-row", workspace);
  await waitFor(() => {
    const total = workspace.querySelector(".transcript-total")?.textContent;
    return total?.includes(segmentCount.toString()) ? true : null;
  }, "summary_ui_segment_total_missing");
  const firstPage = await segmentPage(expectedFirstText, firstPageSize);
  requireCondition(
    firstPage[2]?.textContent?.includes(chineseSegmentText) === true,
    "summary_ui_chinese_segment_missing",
  );
}

async function verifyPagination(): Promise<void> {
  click(await enabledElement('[data-acceptance="transcript-segments-next"]'));
  await segmentPage("Synthetic UI segment 101", firstPageSize);

  click(await enabledElement('[data-acceptance="transcript-segments-next"]'));
  await segmentPage("Synthetic UI segment 201", segmentCount - 200);

  click(
    await enabledElement('[data-acceptance="transcript-segments-previous"]'),
  );
  await segmentPage("Synthetic UI segment 101", firstPageSize);
  click(
    await enabledElement('[data-acceptance="transcript-segments-previous"]'),
  );
  await segmentPage("Synthetic UI segment 001", firstPageSize);
}

async function editFirstSegment(): Promise<void> {
  const first = firstSegment();
  click(
    await enabledElement('[data-acceptance="edit-transcript-segment"]', first),
  );
  setControlValue(
    await input('input[id^="speaker-name-"]', first),
    individualSpeakerName,
  );
  setControlValue(
    await textarea('textarea[id^="segment-text-"]', first),
    correctedSegmentText,
  );
  await nextFrame();
  click(
    await enabledElement('[data-acceptance="save-transcript-segment"]', first),
  );
  await waitFor(() => {
    const current = firstSegment();
    const text = current.querySelector("p")?.textContent?.trim();
    const speaker = current.querySelector("strong")?.textContent?.trim();
    return text === correctedSegmentText && speaker === individualSpeakerName
      ? true
      : null;
  }, "summary_ui_segment_correction_missing");
}

async function renameSpeakerGroup(): Promise<void> {
  const row = await waitFor(
    () =>
      [...document.querySelectorAll<HTMLElement>(".speaker-group-row")].find(
        (candidate) =>
          candidate.querySelector("strong")?.textContent?.trim() ===
          "Speaker Beta",
      ) ?? null,
    "summary_ui_speaker_group_missing",
  );
  setControlValue(await input("input", row), batchSpeakerName);
  await nextFrame();
  click(
    await enabledElement('[data-acceptance="rename-transcript-speaker"]', row),
  );
  const confirmation = await element<HTMLElement>(".speaker-confirm");
  click(
    await enabledElement(
      '[data-acceptance="confirm-transcript-speaker-rename"]',
      confirmation,
    ),
  );
  await waitFor(() => {
    const second = document.querySelectorAll<HTMLElement>(
      ".transcript-segment-list .transcript-segment",
    )[1];
    return second?.querySelector("strong")?.textContent?.trim() ===
      batchSpeakerName
      ? true
      : null;
  }, "summary_ui_batch_speaker_name_missing");
}

async function verifyDuplicatePaste(meetingId: string): Promise<void> {
  const initialSources = await transcriptApi.listSources(meetingId);
  requireCondition(
    initialSources.sources.length === 1,
    "summary_ui_duplicate_precondition_failed",
  );

  const paste = await textarea("#transcript-paste-existing");
  setControlValue(paste, syntheticTranscript());
  await nextFrame();
  click(await enabledElement('[data-acceptance="submit-paste-existing"]'));

  const notice = await waitFor(
    () =>
      [...document.querySelectorAll<HTMLElement>(".notice--info")].find(
        (candidate) =>
          candidate.textContent?.includes(i18n.t("summaryMode.duplicate")) &&
          candidate.textContent?.includes(i18n.t("summaryMode.duplicateBody")),
      ) ?? null,
    "summary_ui_duplicate_notice_missing",
  );
  requireCondition(
    notice.getAttribute("role") === "status",
    "summary_ui_duplicate_notice_not_status",
  );
  await waitFor(
    () =>
      document.querySelectorAll(".transcript-source-row").length === 1
        ? true
        : null,
    "summary_ui_duplicate_dom_source_count_changed",
  );
  const persistedSources = await transcriptApi.listSources(meetingId);
  requireCondition(
    persistedSources.sources.length === initialSources.sources.length &&
      persistedSources.sources[0]?.id === initialSources.sources[0]?.id,
    "summary_ui_duplicate_source_count_changed",
  );
}

async function verifyRevisionConflictRecovery(
  meetingId: string,
): Promise<void> {
  const source = await originalTranscriptSource(meetingId);
  const before = await firstPersistedSegment(meetingId, source.id);
  const first = firstSegment();
  click(
    await enabledElement('[data-acceptance="edit-transcript-segment"]', first),
  );

  const concurrent = await transcriptApi.updateSegment({
    meetingId,
    segmentId: before.id,
    expectedRevision: before.revision,
    speakerName: concurrentSpeakerName,
    originalText: concurrentSegmentText,
  });
  requireCondition(
    concurrent.revision === before.revision + 1,
    "summary_ui_concurrent_revision_not_incremented",
  );

  setControlValue(
    await input('input[id^="speaker-name-"]', first),
    recoveredSpeakerName,
  );
  setControlValue(
    await textarea('textarea[id^="segment-text-"]', first),
    recoveredSegmentText,
  );
  await nextFrame();
  click(
    await enabledElement('[data-acceptance="save-transcript-segment"]', first),
  );

  const reload = await enabledElement(
    '[data-acceptance="reload-transcript-segments"]',
  );
  const conflict = reload
    .closest(".transcript-segment-error")
    ?.querySelector<HTMLElement>('[role="alert"]');
  requireCondition(
    conflict?.textContent?.includes("transcript_revision_conflict") === true,
    "summary_ui_revision_conflict_not_reported",
  );
  click(reload);

  await waitFor(() => {
    const current = firstSegment();
    return current.querySelector("p")?.textContent?.trim() ===
      concurrentSegmentText &&
      current.querySelector("strong")?.textContent?.trim() ===
        concurrentSpeakerName
      ? true
      : null;
  }, "summary_ui_conflict_reload_failed");

  const reloadedFirst = firstSegment();
  click(
    await enabledElement(
      '[data-acceptance="edit-transcript-segment"]',
      reloadedFirst,
    ),
  );
  setControlValue(
    await input('input[id^="speaker-name-"]', reloadedFirst),
    recoveredSpeakerName,
  );
  setControlValue(
    await textarea('textarea[id^="segment-text-"]', reloadedFirst),
    recoveredSegmentText,
  );
  await nextFrame();
  click(
    await enabledElement(
      '[data-acceptance="save-transcript-segment"]',
      reloadedFirst,
    ),
  );

  await waitFor(() => {
    const current = firstSegment();
    return current.querySelector("p")?.textContent?.trim() ===
      recoveredSegmentText &&
      current.querySelector("strong")?.textContent?.trim() ===
        recoveredSpeakerName
      ? true
      : null;
  }, "summary_ui_conflict_recovery_save_failed");

  const persisted = await firstPersistedSegment(meetingId, source.id);
  requireCondition(
    persisted.revision === concurrent.revision + 1 &&
      persisted.originalText === recoveredSegmentText &&
      persisted.speakerName === recoveredSpeakerName,
    "summary_ui_conflict_recovery_not_persisted",
  );
}

async function verifyLifecyclePastes(meetingId: string): Promise<void> {
  const recording = await meetingApi.start(meetingId);
  requireCondition(
    recording.status === "recording",
    "summary_ui_recording_transition_failed",
  );
  await addUniquePasteThroughUi(recordingLifecycleTranscript, 2);
  requireCondition(
    (await transcriptApi.listSources(meetingId)).sources.length === 2,
    "summary_ui_recording_source_not_persisted",
  );

  const ready = await meetingApi.end(meetingId);
  requireCondition(
    ready.status === "ready",
    "summary_ui_ready_transition_failed",
  );
  await addUniquePasteThroughUi(readyLifecycleTranscript, 3);
  requireCondition(
    (await transcriptApi.listSources(meetingId)).sources.length === 3,
    "summary_ui_ready_source_not_persisted",
  );
}

async function addUniquePasteThroughUi(
  content: string,
  expectedSourceCount: number,
): Promise<void> {
  const paste = await textarea("#transcript-paste-existing");
  setControlValue(paste, content);
  await nextFrame();
  click(await enabledElement('[data-acceptance="submit-paste-existing"]'));
  await waitFor(
    () =>
      document.querySelectorAll(".transcript-source-row").length ===
      expectedSourceCount
        ? true
        : null,
    "summary_ui_lifecycle_source_count_mismatch",
  );
}

async function originalTranscriptSource(meetingId: string) {
  const sources = (await transcriptApi.listSources(meetingId)).sources;
  const source = sources.find(
    (candidate) => candidate.segmentCount === segmentCount,
  );
  requireCondition(source !== undefined, "summary_ui_original_source_missing");
  return source;
}

async function firstPersistedSegment(meetingId: string, sourceId: string) {
  const page = await transcriptApi.listSegments(meetingId, sourceId, {
    cursor: null,
    limit: 1,
  });
  const first = page.items[0];
  requireCondition(first !== undefined, "summary_ui_first_segment_missing");
  return first;
}

async function verifyPersistedCorrections(): Promise<void> {
  await waitFor(() => {
    const segments = document.querySelectorAll<HTMLElement>(
      ".transcript-segment-list .transcript-segment",
    );
    const first = segments[0];
    const second = segments[1];
    return first?.querySelector("p")?.textContent?.trim() ===
      recoveredSegmentText &&
      first.querySelector("strong")?.textContent?.trim() ===
        recoveredSpeakerName &&
      second?.querySelector("strong")?.textContent?.trim() === batchSpeakerName
      ? true
      : null;
  }, "summary_ui_restart_persistence_missing");
}

async function selectOriginalSource(): Promise<void> {
  const source = await waitFor(
    () =>
      [
        ...document.querySelectorAll<HTMLElement>(".transcript-source-row"),
      ].find((candidate) =>
        candidate
          .querySelector(".transcript-source-facts div:first-child dd")
          ?.textContent?.includes(segmentCount.toString()),
      ) ?? null,
    "summary_ui_original_source_row_missing",
  );
  click(
    await enabledElement('[data-acceptance="view-transcript-source"]', source),
  );
  await waitFor(
    () =>
      source.classList.contains("transcript-source-row--selected")
        ? true
        : null,
    "summary_ui_original_source_not_selected",
  );
}

async function deleteOneSourceThroughUi(meetingId: string): Promise<void> {
  const sourceRows = [
    ...document.querySelectorAll<HTMLElement>(".transcript-source-row"),
  ];
  requireCondition(sourceRows.length === 3, "summary_ui_source_count_mismatch");
  const source = sourceRows.find(
    (candidate) =>
      !candidate
        .querySelector(".transcript-source-facts div:first-child dd")
        ?.textContent?.includes(segmentCount.toString()),
  );
  requireCondition(source !== undefined, "summary_ui_deletable_source_missing");
  click(
    await enabledElement(
      '[data-acceptance="delete-transcript-source"]',
      source,
    ),
  );
  click(
    await enabledElement(
      '[data-acceptance="confirm-transcript-source-delete"]',
      source,
    ),
  );
  await waitFor(
    () =>
      document.querySelectorAll(".transcript-source-row").length ===
      sourceRows.length - 1
        ? true
        : null,
    "summary_ui_source_delete_failed",
  );
  requireCondition(
    (await transcriptApi.listSources(meetingId)).sources.length ===
      sourceRows.length - 1,
    "summary_ui_source_delete_not_persisted",
  );
}

async function deleteMeetingThroughUi(
  meetingId: string,
  expectedTitle: string,
): Promise<void> {
  click(await enabledElement('[data-acceptance="back-to-meeting-details"]'));
  const detail = await element<HTMLElement>(
    '[data-acceptance="meeting-detail"]',
  );
  requireCondition(
    detail.querySelector("h1")?.textContent?.trim() === expectedTitle,
    "summary_ui_delete_title_mismatch",
  );
  click(await enabledElement('[data-acceptance="delete-meeting"]'));
  click(await enabledElement('[data-acceptance="confirm-delete"]'));
  await waitFor(
    () => (window.location.hash === "#/history" ? true : null),
    "summary_ui_delete_navigation_missing",
  );
  await waitFor(
    () =>
      document.querySelector(`[data-testid="meeting-row-${meetingId}"]`) ===
      null
        ? true
        : null,
    "summary_ui_deleted_meeting_visible",
  );
}

function syntheticTranscript(): string {
  return Array.from({ length: segmentCount }, (_, index) => {
    const number = (index + 1).toString().padStart(3, "0");
    const speaker = index % 2 === 0 ? "Speaker Alpha" : "Speaker Beta";
    if (index === 2) {
      return `${speaker}: ${chineseSegmentText} ${number}`;
    }
    return `${speaker}: Synthetic UI segment ${number}`;
  }).join("\n\n");
}

function firstSegment(): HTMLElement {
  return requireElement<HTMLElement>(
    ".transcript-segment-list .transcript-segment:first-child",
  );
}

async function segmentPage(
  expectedFirstText: string,
  expectedLength: number,
): Promise<HTMLElement[]> {
  return waitFor(() => {
    const segments = [
      ...document.querySelectorAll<HTMLElement>(
        ".transcript-segment-list .transcript-segment",
      ),
    ];
    return segments.length === expectedLength &&
      segments[0]?.textContent?.includes(expectedFirstText)
      ? segments
      : null;
  }, "summary_ui_segment_page_mismatch");
}

function meetingIdFromHash(): string | null {
  const match = window.location.hash.match(
    /^#\/meetings\/([0-9a-f-]+)\/transcript$/,
  );
  const candidate = match?.[1];
  return candidate && uuidPattern.test(candidate) ? candidate : null;
}

async function cleanUpOwnedMeeting(state: SmokeState): Promise<void> {
  if (state.meetingId === null) {
    const candidates = (await meetingApi.list()).meetings.filter(
      (meeting) => meeting.title === meetingTitle(state.runId),
    );
    if (candidates.length === 0) return;
    requireCondition(
      candidates.length === 1,
      "summary_ui_cleanup_candidate_ambiguous",
    );
    const candidate = candidates[0];
    requireCondition(
      candidate !== undefined,
      "summary_ui_cleanup_candidate_missing",
    );
    state.meetingId = candidate.id;
    writeState(state);
  }
  try {
    let meeting = await meetingApi.get(state.meetingId);
    requireCondition(
      meeting.title === meetingTitle(state.runId),
      "summary_ui_cleanup_ownership_mismatch",
    );
    if (meeting.status === "recording") {
      meeting = await meetingApi.end(meeting.id);
    }
    requireCondition(
      meeting.status === "draft" ||
        meeting.status === "ready" ||
        meeting.status === "delete_failed",
      "summary_ui_cleanup_state_invalid",
    );
    await meetingApi.delete(meeting.id);
  } catch (error: unknown) {
    if (
      error instanceof MeetingApiError &&
      error.code === "meeting_not_found"
    ) {
      return;
    }
    throw error;
  }
}

async function expectMeetingMissing(meetingId: string): Promise<void> {
  try {
    await meetingApi.get(meetingId);
  } catch (error: unknown) {
    if (
      error instanceof MeetingApiError &&
      error.code === "meeting_not_found"
    ) {
      return;
    }
    throw error;
  }
  throw new Error("summary_ui_deleted_meeting_still_exists");
}

async function verifyReadOnlySettingsAndLocales(): Promise<void> {
  const startupLocale = document.documentElement.lang;
  requireCondition(
    startupLocale === "zh-CN" || startupLocale === "en-US",
    "summary_ui_startup_locale_invalid",
  );
  click(await enabledElement('a[href="#/settings"]'));

  const runtime = await element<HTMLElement>(
    '[data-acceptance="sidecar-security-status"]',
  );
  await waitFor(() => {
    const text = runtime.querySelector("dd")?.textContent?.trim();
    return text === i18n.t("settings.serviceStatus.authenticated")
      ? true
      : null;
  }, "summary_ui_sidecar_not_authenticated");

  const secretPanel = await element<HTMLElement>(
    '[data-acceptance="deepseek-secret-settings"]',
  );
  await waitFor(() => {
    const status =
      secretPanel.querySelector<HTMLElement>(".secret-status")?.dataset.status;
    return status === "configured" || status === "unconfigured" ? status : null;
  }, "summary_ui_secret_status_unavailable");
  const secretInput = requireElement<HTMLInputElement>(
    '[data-acceptance="deepseek-secret-input"]',
    secretPanel,
  );
  requireCondition(
    secretInput.type === "password" && secretInput.value.length === 0,
    "summary_ui_secret_value_exposed",
  );
  requireCondition(
    secretPanel.querySelector(
      '[data-acceptance="secret-delete-confirmation"]',
    ) === null,
    "summary_ui_secret_mutation_started",
  );

  const languageSelect = requireElement<HTMLSelectElement>(
    '[data-acceptance="ui-language"]',
  );
  const alternateLocale = startupLocale === "zh-CN" ? "en-US" : "zh-CN";
  try {
    setSelectValue(languageSelect, alternateLocale);
    await waitFor(
      () => (document.documentElement.lang === alternateLocale ? true : null),
      "summary_ui_locale_switch_failed",
    );
    requireCondition(
      requireElement<HTMLInputElement>(
        '[data-acceptance="deepseek-secret-input"]',
      ).value.length === 0,
      "summary_ui_secret_value_exposed_after_locale_switch",
    );
    click(await enabledElement('a[href="#/help"]'));
    const help = await element<HTMLElement>('[data-acceptance="help-page"]');
    requireCondition(
      help.textContent?.includes(i18n.t("help.availableBody")) === true,
      "summary_ui_help_current_capability_missing",
    );
    requireCondition(
      help.textContent?.includes(i18n.t("help.unavailableBody")) === true,
      "summary_ui_help_unavailable_statement_missing",
    );
    click(await enabledElement('a[href="#/settings"]'));
    await element('[data-acceptance="ui-language"]');
    requireCondition(
      requireElement<HTMLInputElement>(
        '[data-acceptance="deepseek-secret-input"]',
      ).value.length === 0,
      "summary_ui_secret_value_exposed_after_help",
    );
  } finally {
    if (document.querySelector('[data-acceptance="ui-language"]') === null) {
      click(await enabledElement('a[href="#/settings"]'));
      await element('[data-acceptance="ui-language"]');
    }
    const currentSelect = requireElement<HTMLSelectElement>(
      '[data-acceptance="ui-language"]',
    );
    if (document.documentElement.lang !== startupLocale) {
      setSelectValue(currentSelect, startupLocale);
      await waitFor(
        () => (document.documentElement.lang === startupLocale ? true : null),
        "summary_ui_locale_restore_failed",
      );
    }
  }
}

function installNetworkRecorder(): NetworkRecorder {
  const externalRequests: string[] = [];
  const originalFetch = window.fetch;
  const originalXhrOpen = XMLHttpRequest.prototype.open;
  const OriginalWebSocket = window.WebSocket;

  const record = (value: string): void => {
    if (isExternalNetworkUrl(value)) externalRequests.push("external");
  };

  const wrappedFetch: typeof window.fetch = (input, init) => {
    record(fetchRequestUrl(input));
    return originalFetch.call(window, input, init);
  };
  window.fetch = wrappedFetch;

  XMLHttpRequest.prototype.open = function (
    method: string,
    url: string | URL,
    async = true,
    username?: string | null,
    password?: string | null,
  ): void {
    record(url.toString());
    const args =
      username === undefined && password === undefined
        ? [method, url, async]
        : [method, url, async, username ?? null, password ?? null];
    Reflect.apply(originalXhrOpen, this, args);
  };
  const wrappedXhrOpen = XMLHttpRequest.prototype.open;

  class RecordedWebSocket extends OriginalWebSocket {
    constructor(url: string | URL, protocols?: string | string[]) {
      record(url.toString());
      if (protocols === undefined) {
        super(url);
      } else {
        super(url, protocols);
      }
    }
  }
  window.WebSocket = RecordedWebSocket;

  return {
    assertNoExternalRequests: () => {
      requireCondition(
        externalRequests.length === 0,
        "summary_ui_external_network_detected",
      );
    },
    restore: () => {
      if (window.fetch === wrappedFetch) window.fetch = originalFetch;
      if (XMLHttpRequest.prototype.open === wrappedXhrOpen) {
        XMLHttpRequest.prototype.open = originalXhrOpen;
      }
      if (window.WebSocket === RecordedWebSocket) {
        window.WebSocket = OriginalWebSocket;
      }
    },
  };
}

function fetchRequestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.toString();
  return input.url;
}

function isExternalNetworkUrl(value: string): boolean {
  try {
    const url = new URL(value, window.location.href);
    if (!["http:", "https:", "ws:", "wss:"].includes(url.protocol)) {
      return false;
    }
    if (url.origin === window.location.origin) return false;
    const hostname = url.hostname.toLowerCase();
    return ![
      "127.0.0.1",
      "localhost",
      "[::1]",
      "::1",
      "ipc.localhost",
      "asset.localhost",
      "tauri.localhost",
    ].includes(hostname);
  } catch {
    return true;
  }
}

async function assertNoSyntheticBrowserPersistence(): Promise<void> {
  assertStorageContainsNoSyntheticContent(window.localStorage);
  assertStorageContainsNoSyntheticContent(window.sessionStorage);
  const databases = await window.indexedDB.databases();
  for (const info of databases) {
    if (!info.name) continue;
    const database = await openExistingDatabase(info.name);
    try {
      for (const storeName of [...database.objectStoreNames]) {
        const transaction = database.transaction(storeName, "readonly");
        const completion = idbTransaction(transaction);
        const request = transaction.objectStore(storeName).getAll();
        const values = await idbRequest(request);
        await completion;
        for (const value of values) {
          requireCondition(
            !(await containsSyntheticContent(value)),
            "summary_ui_indexeddb_contains_synthetic_content",
          );
        }
      }
    } finally {
      database.close();
    }
  }
}

function assertStorageContainsNoSyntheticContent(storage: Storage): void {
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (key === null) continue;
    const value = storage.getItem(key);
    requireCondition(
      !containsSyntheticText(key) &&
        (value === null || !containsSyntheticText(value)),
      "summary_ui_web_storage_contains_synthetic_content",
    );
  }
}

function containsSyntheticText(value: string): boolean {
  return [
    "Synthetic UI segment",
    correctedSegmentText,
    individualSpeakerName,
    batchSpeakerName,
    chineseSegmentText,
    concurrentSpeakerName,
    concurrentSegmentText,
    recoveredSpeakerName,
    recoveredSegmentText,
    recordingLifecycleTranscript,
    readyLifecycleTranscript,
  ].some((marker) => value.includes(marker));
}

async function openExistingDatabase(name: string): Promise<IDBDatabase> {
  return new Promise<IDBDatabase>((resolve, reject) => {
    const request = window.indexedDB.open(name);
    request.onupgradeneeded = () => request.transaction?.abort();
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(new Error("summary_ui_indexeddb_open_failed"));
    request.onblocked = () =>
      reject(new Error("summary_ui_indexeddb_open_blocked"));
  });
}

async function idbRequest<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(new Error("summary_ui_indexeddb_read_failed"));
  });
}

async function idbTransaction(transaction: IDBTransaction): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () =>
      reject(new Error("summary_ui_indexeddb_transaction_aborted"));
    transaction.onerror = () =>
      reject(new Error("summary_ui_indexeddb_transaction_failed"));
  });
}

async function containsSyntheticContent(
  value: unknown,
  seen = new WeakSet<object>(),
): Promise<boolean> {
  if (typeof value === "string") return containsSyntheticText(value);
  if (
    value === null ||
    (typeof value !== "object" && typeof value !== "function")
  ) {
    return false;
  }
  if (value instanceof Blob) {
    return containsSyntheticText(await value.text());
  }
  if (value instanceof ArrayBuffer) {
    return containsSyntheticText(new TextDecoder().decode(value));
  }
  if (ArrayBuffer.isView(value)) {
    return containsSyntheticText(
      new TextDecoder().decode(
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      ),
    );
  }
  if (seen.has(value)) return false;
  seen.add(value);
  if (value instanceof Map) {
    for (const [key, entry] of value) {
      if (
        (await containsSyntheticContent(key, seen)) ||
        (await containsSyntheticContent(entry, seen))
      ) {
        return true;
      }
    }
    return false;
  }
  if (value instanceof Set) {
    for (const entry of value) {
      if (await containsSyntheticContent(entry, seen)) return true;
    }
    return false;
  }
  for (const [key, entry] of Object.entries(value)) {
    if (
      containsSyntheticText(key) ||
      (await containsSyntheticContent(entry, seen))
    ) {
      return true;
    }
  }
  return false;
}

function meetingTitle(runId: string): string {
  return `${meetingTitlePrefix} ${runId}`;
}

function setControlValue(
  control: HTMLInputElement | HTMLTextAreaElement,
  value: string,
): void {
  const prototype =
    control instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
  requireCondition(setter !== undefined, "summary_ui_value_setter_missing");
  setter.call(control, value);
  control.dispatchEvent(new Event("input", { bubbles: true }));
}

function setSelectValue(control: HTMLSelectElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLSelectElement.prototype,
    "value",
  )?.set;
  requireCondition(setter !== undefined, "summary_ui_select_setter_missing");
  setter.call(control, value);
  control.dispatchEvent(new Event("change", { bubbles: true }));
}

function click(elementToClick: HTMLElement): void {
  elementToClick.click();
}

async function input(
  selector: string,
  root: ParentNode = document,
): Promise<HTMLInputElement> {
  return element<HTMLInputElement>(selector, root);
}

async function textarea(
  selector: string,
  root: ParentNode = document,
): Promise<HTMLTextAreaElement> {
  return element<HTMLTextAreaElement>(selector, root);
}

async function enabledElement(
  selector: string,
  root: ParentNode = document,
): Promise<HTMLElement> {
  return waitFor(() => {
    const candidate = root.querySelector<HTMLElement>(selector);
    if (candidate === null) return null;
    if (
      candidate instanceof HTMLButtonElement ||
      candidate instanceof HTMLInputElement ||
      candidate instanceof HTMLTextAreaElement
    ) {
      return candidate.disabled ? null : candidate;
    }
    return candidate;
  }, "summary_ui_enabled_element_missing");
}

async function element<T extends Element>(
  selector: string,
  root: ParentNode = document,
): Promise<T> {
  return waitFor(
    () => root.querySelector<T>(selector),
    "summary_ui_element_missing",
  );
}

function requireElement<T extends Element>(
  selector: string,
  root: ParentNode = document,
): T {
  const candidate = root.querySelector<T>(selector);
  requireCondition(candidate !== null, "summary_ui_required_element_missing");
  return candidate;
}

async function waitFor<T>(
  probe: () => T | null | undefined | false,
  failureCode: string,
  timeoutMs = defaultTimeoutMs,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = probe();
    if (result !== null && result !== undefined && result !== false) {
      return result;
    }
    await delay(pollIntervalMs);
  }
  throw new Error(failureCode);
}

async function nextFrame(): Promise<void> {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

async function delay(milliseconds: number): Promise<void> {
  await new Promise<void>((resolve) =>
    window.setTimeout(resolve, milliseconds),
  );
}

function readState(): SmokeState | null {
  const raw = window.localStorage.getItem(smokeStateKey);
  if (raw === null) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (
      !isRecord(value) ||
      !hasOnlyKeys(value, ["runId", "phase", "meetingId"]) ||
      typeof value.runId !== "string" ||
      !uuidPattern.test(value.runId) ||
      (value.phase !== "running" && value.phase !== "restart") ||
      !isNullableUuid(value.meetingId)
    ) {
      clearState();
      return null;
    }
    return {
      runId: value.runId,
      phase: value.phase,
      meetingId: value.meetingId,
    };
  } catch {
    clearState();
    return null;
  }
}

function writeState(state: SmokeState): void {
  window.localStorage.setItem(smokeStateKey, JSON.stringify(state));
}

function clearState(): void {
  window.localStorage.removeItem(smokeStateKey);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const allowed = new Set(keys);
  return Object.keys(value).every((key) => allowed.has(key));
}

const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function isNullableUuid(value: unknown): value is string | null {
  return (
    value === null || (typeof value === "string" && uuidPattern.test(value))
  );
}

function mark(status: string): void {
  const marker =
    document.querySelector<HTMLOutputElement>("[data-summary-mode-ui-smoke]") ??
    document.body.appendChild(document.createElement("output"));
  marker.dataset.summaryModeUiSmoke = status;
  marker.textContent = `Summary mode UI smoke: ${status}`;
  document.title = `MeetingLens summary UI smoke: ${status}`;
}

function safeErrorCode(error: unknown): string {
  if (error instanceof MeetingApiError || error instanceof TranscriptApiError) {
    return error.code;
  }
  if (error instanceof Error && /^summary_ui_[a-z0-9_]+$/.test(error.message)) {
    return error.message;
  }
  return "unexpected";
}

function requireCondition(
  condition: boolean,
  message = "summary_ui_smoke_assertion_failed",
): asserts condition {
  if (!condition) throw new Error(message);
}
