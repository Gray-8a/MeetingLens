import {
  MeetingApiError,
  meetingApi,
  type MeetingResponse,
} from "../api/meeting";
import {
  summaryApi,
  SummaryApiError,
  type SummaryContent,
} from "../api/summary";
import {
  TranscriptApiError,
  transcriptApi,
  type CompletedTranscriptImportResult,
} from "../api/transcript";

const smokeStateKey = "meetinglens.summary-run-core-smoke.state.v1";
const titlePrefix = "Synthetic summary command smoke";
const interruptionTitlePrefix = "Synthetic summary run interruption smoke";
const primaryLanguage = "en-AU";
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
let smokeCheckpoint = "startup";

interface SmokeState {
  runId: string;
  phase: "running" | "restart";
  meetingId: string | null;
  completedSummaryId: string | null;
  revisionSummaryId: string | null;
  cancelledSummaryId: string | null;
  interruptedSummaryId: string | null;
  queuedSummaryId: string | null;
}

export async function runSummaryRunCoreSmoke(): Promise<void> {
  const networkMonitor = installExternalNetworkMonitor();
  let state: SmokeState | null = null;
  try {
    requireCondition(
      import.meta.env.VITE_SUMMARY_RUN_CORE_SMOKE_ISOLATED === "1",
      "summary_smoke_isolation_required",
    );
    state = readState();
    if (state?.phase === "restart") {
      smokeCheckpoint = "restart-verification";
      await verifyRestartAndCleanUp(state, networkMonitor);
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
      completedSummaryId: null,
      revisionSummaryId: null,
      cancelledSummaryId: null,
      interruptedSummaryId: null,
      queuedSummaryId: null,
    };
    writeState(state);
    await runFirstPass(state, networkMonitor);
  } catch (error: unknown) {
    if (state !== null) {
      try {
        await cleanUpOwnedMeeting(state);
        clearState();
      } catch {
        // Keep the ID-only state so the next diagnostic run can retry cleanup safely.
      }
    }
    mark(`failed:${safeErrorCode(error)}`);
  } finally {
    networkMonitor.restore();
  }
}

async function runFirstPass(
  state: SmokeState,
  networkMonitor: ExternalNetworkMonitor,
): Promise<void> {
  smokeCheckpoint = "policy";
  const policy = await summaryApi.getPolicy();
  requireCondition(policy.provider === "deterministic_mock");
  requireCondition(policy.maxSegments === 20_000);
  requireCondition(policy.maxInputBytes === 10 * 1024 * 1024);
  requireCondition(policy.maxEvidenceExcerptBytes === 1_024);

  smokeCheckpoint = "create-transcript";
  const imported = requireCompleted(
    await transcriptApi.createMeetingFromPaste({
      title: normalTitle(state.runId),
      primaryLanguage,
      displayName: "Synthetic summary smoke source",
      pastedText:
        "Speaker 1: Synthetic opening point\n\nSpeaker 2: Synthetic follow-up point",
      language: primaryLanguage,
      baseStartedAt: "2026-08-07T00:00:00.000Z",
    }),
  );
  state.meetingId = imported.meeting.id;
  writeState(state);

  smokeCheckpoint = "complete-run";
  const first = await summaryApi.createRun({
    meetingId: imported.meeting.id,
    sourceIds: [imported.source.id],
    template: "detailed",
    outputLanguage: "en",
  });
  requireCondition(first.status === "queued");
  const completed = await summaryApi.executeRun({
    meetingId: imported.meeting.id,
    summaryId: first.id,
  });
  requireCondition(completed.status === "completed");
  requireCondition(completed.content !== null);
  state.completedSummaryId = first.id;
  const frozenContent = JSON.stringify(completed.content);
  const evidence = await summaryApi.listEvidence({
    meetingId: imported.meeting.id,
    summaryId: first.id,
    limit: 1,
  });
  requireCondition(evidence.items.length === 1);
  requireCondition(evidence.totalCount >= 1);
  const frozenExcerpt = evidence.items[0]?.excerpt;
  requireCondition(frozenExcerpt !== undefined);

  smokeCheckpoint = "stale-revision";
  const segments = await transcriptApi.listSegments(
    imported.meeting.id,
    imported.source.id,
    { limit: 100 },
  );
  const segment = segments.items[0];
  requireCondition(segment !== undefined);
  requireCondition(frozenExcerpt === segment.originalText);
  await transcriptApi.updateSegment({
    meetingId: imported.meeting.id,
    segmentId: segment.id,
    originalText: "Synthetic corrected summary smoke point",
    expectedRevision: segment.revision,
  });
  const stale = await summaryApi.getRun({
    meetingId: imported.meeting.id,
    summaryId: first.id,
  });
  requireCondition(stale.isStale);
  requireCondition(stale.staleReason === "segment_revised");
  requireCondition(JSON.stringify(stale.content) === frozenContent);
  requireCondition(
    (
      await summaryApi.listEvidence({
        meetingId: imported.meeting.id,
        summaryId: first.id,
        limit: 1,
      })
    ).items[0]?.excerpt === frozenExcerpt,
  );

  const second = await summaryApi.createRevision({
    meetingId: imported.meeting.id,
    summaryId: first.id,
  });
  requireCondition(second.revision === 2);
  const completedRevision = await summaryApi.executeRun({
    meetingId: imported.meeting.id,
    summaryId: second.id,
  });
  requireCondition(completedRevision.status === "completed");
  requireCondition(completedRevision.content !== null);
  state.revisionSummaryId = second.id;
  requireCondition(
    (
      await summaryApi.listEvidence({
        meetingId: imported.meeting.id,
        summaryId: second.id,
        limit: 1,
      })
    ).items[0]?.excerpt === "Synthetic corrected summary smoke point",
  );
  const cancelledRevision = await summaryApi.createRevision({
    meetingId: imported.meeting.id,
    summaryId: second.id,
  });
  requireCondition(
    (
      await summaryApi.cancelRun({
        meetingId: imported.meeting.id,
        summaryId: cancelledRevision.id,
      })
    ).status === "cancelled",
  );
  state.cancelledSummaryId = cancelledRevision.id;

  smokeCheckpoint = "source-redaction";
  const activeSourceConsumer = await summaryApi.createRun({
    meetingId: imported.meeting.id,
    sourceIds: [imported.source.id],
    template: "concise",
    outputLanguage: "bilingual",
  });
  await expectSourceInUse(imported.meeting.id, imported.source.id);
  requireCondition(
    (
      await summaryApi.cancelRun({
        meetingId: imported.meeting.id,
        summaryId: activeSourceConsumer.id,
      })
    ).status === "cancelled",
  );
  const deleted = await transcriptApi.deleteSource(
    imported.meeting.id,
    imported.source.id,
  );
  requireCondition(deleted.deleted);
  const redactedEvidence = await summaryApi.listEvidence({
    meetingId: imported.meeting.id,
    summaryId: first.id,
    limit: 100,
  });
  requireCondition(
    redactedEvidence.items.length > 0 &&
      redactedEvidence.items.every((item) => item.sourceDeleted) &&
      redactedEvidence.items[0]?.excerpt === frozenExcerpt,
  );
  const sourceDeletedRun = await summaryApi.getRun({
    meetingId: imported.meeting.id,
    summaryId: first.id,
  });
  requireCondition(sourceDeletedRun.staleReason === "source_deleted");
  requireCondition(JSON.stringify(sourceDeletedRun.content) === frozenContent);

  smokeCheckpoint = "prepare-recovery-source";
  const recoverySource = requireCompleted(
    await transcriptApi.addPasteToMeeting({
      meetingId: imported.meeting.id,
      displayName: "Synthetic recovery source",
      pastedText: "Synthetic recovery point",
      language: primaryLanguage,
      baseStartedAt: null,
    }),
  ).source;
  smokeCheckpoint = "prepare-interruption-title";
  await meetingApi.update({
    id: imported.meeting.id,
    title: interruptionTitle(state.runId),
  });
  smokeCheckpoint = "prepare-interrupted-run";
  const interrupted = await summaryApi.createRun({
    meetingId: imported.meeting.id,
    sourceIds: [recoverySource.id],
    template: "concise",
    outputLanguage: "en",
  });
  smokeCheckpoint = "prepare-queued-run";
  const queued = await summaryApi.createRun({
    meetingId: imported.meeting.id,
    sourceIds: [recoverySource.id],
    template: "actions_decisions",
    outputLanguage: "zh",
  });
  state.interruptedSummaryId = interrupted.id;
  state.queuedSummaryId = queued.id;

  smokeCheckpoint = "prepare-interrupted-execution";
  const execution = summaryApi.executeRun({
    meetingId: imported.meeting.id,
    summaryId: interrupted.id,
  });
  void execution.catch(() => undefined);
  smokeCheckpoint = "prepare-running-wait";
  await waitForRunning(imported.meeting.id, interrupted.id);
  smokeCheckpoint = "prepare-restart-state";
  state.phase = "restart";
  writeState(state);
  smokeCheckpoint = "prepare-browser-storage";
  await assertBrowserStorageContainsNoMeetingData([
    "Synthetic opening point",
    "Synthetic follow-up point",
    frozenExcerpt,
    "Synthetic corrected summary smoke point",
    "Synthetic recovery point",
    ...summaryContentStorageProbes(completed.content),
    ...summaryContentStorageProbes(completedRevision.content),
  ]);
  smokeCheckpoint = "prepare-network-count";
  requireCondition(networkMonitor.externalCount() === 0);
  mark("restart-running");
}

async function verifyRestartAndCleanUp(
  state: SmokeState,
  networkMonitor: ExternalNetworkMonitor,
): Promise<void> {
  requireCondition(state.meetingId !== null);
  requireCondition(state.completedSummaryId !== null);
  requireCondition(state.revisionSummaryId !== null);
  requireCondition(state.cancelledSummaryId !== null);
  requireCondition(state.interruptedSummaryId !== null);
  requireCondition(state.queuedSummaryId !== null);
  const meeting = await meetingApi.get(state.meetingId);
  requireCondition(meeting.title === interruptionTitle(state.runId));

  const completed = await summaryApi.getRun({
    meetingId: state.meetingId,
    summaryId: state.completedSummaryId,
  });
  requireCondition(completed.status === "completed");
  requireCondition(completed.content?.schemaVersion === 1);
  requireCondition(completed.isStale);
  requireCondition(completed.staleReason === "source_deleted");
  const completedEvidence = await summaryApi.listEvidence({
    meetingId: state.meetingId,
    summaryId: state.completedSummaryId,
    limit: 100,
  });
  requireCondition(completedEvidence.items.length > 0);
  requireCondition(completedEvidence.items.every((item) => item.sourceDeleted));
  requireCondition(
    completed.content.overview.evidenceIds.every((id) =>
      completedEvidence.items.some((item) => item.id === id),
    ),
  );

  const revision = await summaryApi.getRun({
    meetingId: state.meetingId,
    summaryId: state.revisionSummaryId,
  });
  requireCondition(revision.status === "completed");
  requireCondition(revision.revision === 2);
  requireCondition(revision.previousSummaryId === state.completedSummaryId);
  requireCondition(revision.content !== null);
  requireCondition(revision.content.schemaVersion === 1);
  const revisionEvidence = await summaryApi.listEvidence({
    meetingId: state.meetingId,
    summaryId: state.revisionSummaryId,
    limit: 100,
  });
  requireCondition(revisionEvidence.items.length > 0);
  requireCondition(revisionEvidence.items.every((item) => item.sourceDeleted));

  const cancelledRevision = await summaryApi.getRun({
    meetingId: state.meetingId,
    summaryId: state.cancelledSummaryId,
  });
  requireCondition(cancelledRevision.status === "cancelled");
  requireCondition(cancelledRevision.revision === 3);
  requireCondition(
    cancelledRevision.previousSummaryId === state.revisionSummaryId,
  );

  const interrupted = await summaryApi.getRun({
    meetingId: state.meetingId,
    summaryId: state.interruptedSummaryId,
  });
  requireCondition(interrupted.status === "failed");
  requireCondition(interrupted.errorCode === "summary_interrupted");
  requireCondition(interrupted.startedAt !== null);
  requireCondition(interrupted.finishedAt !== null);

  const queued = await summaryApi.getRun({
    meetingId: state.meetingId,
    summaryId: state.queuedSummaryId,
  });
  requireCondition(queued.status === "queued");
  requireCondition(
    (
      await summaryApi.cancelRun({
        meetingId: state.meetingId,
        summaryId: state.queuedSummaryId,
      })
    ).status === "cancelled",
  );
  const page = await summaryApi.listRuns({ meetingId: state.meetingId });
  requireCondition(page.totalCount >= 5);
  await assertBrowserStorageContainsNoMeetingData([
    "Synthetic opening point",
    "Synthetic follow-up point",
    "Synthetic corrected summary smoke point",
    "Synthetic recovery point",
    ...summaryContentStorageProbes(completed.content),
    ...summaryContentStorageProbes(revision.content),
    ...completedEvidence.items.map((item) => item.excerpt),
    ...revisionEvidence.items.map((item) => item.excerpt),
  ]);

  await deleteOwnedMeetingResponse(meeting, interruptionTitle(state.runId));
  await expectMeetingMissing(state.meetingId);
  requireCondition(
    !(await meetingApi.list()).meetings.some((candidate) =>
      candidate.title.includes(state.runId),
    ),
  );
  requireCondition(networkMonitor.externalCount() === 0);
}

async function expectSourceInUse(
  meetingId: string,
  sourceId: string,
): Promise<void> {
  try {
    await transcriptApi.deleteSource(meetingId, sourceId);
  } catch (error: unknown) {
    requireCondition(
      error instanceof TranscriptApiError &&
        error.code === "transcript_source_in_use",
    );
    return;
  }
  throw new Error("summary_smoke_source_delete_should_be_rejected");
}

async function waitForRunning(
  meetingId: string,
  summaryId: string,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const run = await summaryApi.getRun({ meetingId, summaryId });
    if (run.status === "running") return;
    await new Promise((resolve) => window.setTimeout(resolve, 50));
  }
  throw new Error("summary_running_timeout");
}

async function cleanUpOwnedMeeting(state: SmokeState): Promise<void> {
  if (state.meetingId === null) return;
  try {
    const meeting = await meetingApi.get(state.meetingId);
    requireCondition(
      meeting.title === normalTitle(state.runId) ||
        meeting.title === interruptionTitle(state.runId),
    );
    await deleteOwnedMeetingResponse(meeting, meeting.title);
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

async function deleteOwnedMeetingResponse(
  meeting: MeetingResponse,
  expectedTitle: string,
): Promise<void> {
  requireCondition(meeting.title === expectedTitle);
  let current = meeting;
  if (current.status === "recording") {
    current = await meetingApi.end(current.id);
  }
  requireCondition(current.status === "draft" || current.status === "ready");
  requireCondition((await meetingApi.delete(current.id)).deleted);
}

async function expectMeetingMissing(meetingId: string): Promise<void> {
  try {
    await meetingApi.get(meetingId);
  } catch (error: unknown) {
    requireCondition(
      error instanceof MeetingApiError && error.code === "meeting_not_found",
    );
    return;
  }
  throw new Error("summary_smoke_meeting_should_be_deleted");
}

function requireCompleted(
  result: Awaited<ReturnType<typeof transcriptApi.createMeetingFromPaste>>,
): CompletedTranscriptImportResult {
  requireCondition(!result.cancelled);
  return result;
}

function normalTitle(runId: string): string {
  return `${titlePrefix} ${runId}`;
}

function interruptionTitle(runId: string): string {
  return `${interruptionTitlePrefix} ${runId}`;
}

function readState(): SmokeState | null {
  const raw = window.localStorage.getItem(smokeStateKey);
  if (raw === null) return null;
  try {
    const value = JSON.parse(raw) as Partial<SmokeState>;
    if (
      typeof value.runId !== "string" ||
      !uuidPattern.test(value.runId) ||
      (value.phase !== "running" && value.phase !== "restart") ||
      !isNullableUuid(value.meetingId) ||
      !isNullableUuid(value.completedSummaryId) ||
      !isNullableUuid(value.revisionSummaryId) ||
      !isNullableUuid(value.cancelledSummaryId) ||
      !isNullableUuid(value.interruptedSummaryId) ||
      !isNullableUuid(value.queuedSummaryId)
    ) {
      clearState();
      return null;
    }
    return value as SmokeState;
  } catch {
    clearState();
    return null;
  }
}

function isNullableUuid(value: unknown): value is string | null {
  return (
    value === null || (typeof value === "string" && uuidPattern.test(value))
  );
}

function writeState(state: SmokeState): void {
  window.localStorage.setItem(smokeStateKey, JSON.stringify(state));
}

function clearState(): void {
  window.localStorage.removeItem(smokeStateKey);
}

interface ExternalNetworkMonitor {
  externalCount(): number;
  restore(): void;
}

function installExternalNetworkMonitor(): ExternalNetworkMonitor {
  let externalCount = 0;
  const originalFetch = window.fetch;
  const originalXhrOpen = window.XMLHttpRequest.prototype.open;
  const originalWebSocket = window.WebSocket;

  window.fetch = new Proxy(originalFetch, {
    apply(target, thisArgument, argumentsList) {
      if (isExternalNetworkTarget(argumentsList[0])) externalCount += 1;
      return Reflect.apply(target, thisArgument, argumentsList);
    },
  });
  window.XMLHttpRequest.prototype.open = new Proxy(originalXhrOpen, {
    apply(target, thisArgument, argumentsList) {
      if (isExternalNetworkTarget(argumentsList[1])) externalCount += 1;
      return Reflect.apply(target, thisArgument, argumentsList);
    },
  });
  window.WebSocket = new Proxy(originalWebSocket, {
    construct(target, argumentsList, newTarget) {
      if (isExternalNetworkTarget(argumentsList[0])) externalCount += 1;
      return Reflect.construct(target, argumentsList, newTarget);
    },
  });

  return {
    externalCount: () => externalCount,
    restore: () => {
      window.fetch = originalFetch;
      window.XMLHttpRequest.prototype.open = originalXhrOpen;
      window.WebSocket = originalWebSocket;
    },
  };
}

export function isExternalNetworkTarget(value: unknown): boolean {
  let raw: string;
  if (typeof value === "string") {
    raw = value;
  } else if (value instanceof URL) {
    raw = value.href;
  } else if (value instanceof Request) {
    raw = value.url;
  } else {
    return false;
  }
  try {
    const target = new URL(raw, window.location.href);
    if (!["http:", "https:", "ws:", "wss:"].includes(target.protocol)) {
      return false;
    }
    const hostname = target.hostname.toLowerCase();
    return !(
      hostname === "localhost" ||
      hostname.endsWith(".localhost") ||
      ["127.0.0.1", "::1", "[::1]"].includes(hostname)
    );
  } catch {
    return true;
  }
}

async function assertBrowserStorageContainsNoMeetingData(
  forbiddenValues: readonly string[],
): Promise<void> {
  requireCondition(
    !browserStorageContainsForbiddenValue(
      window.localStorage,
      forbiddenValues,
    ) &&
      !browserStorageContainsForbiddenValue(
        window.sessionStorage,
        forbiddenValues,
      ),
    "summary_smoke_browser_storage_contains_meeting_data",
  );
  requireCondition(
    (await window.indexedDB.databases()).length === 0,
    "summary_smoke_indexeddb_not_empty",
  );
}

export function browserStorageContainsForbiddenValue(
  storage: Pick<Storage, "length" | "key" | "getItem">,
  forbiddenValues: readonly string[],
): boolean {
  const probes = [...new Set(forbiddenValues)].filter(
    (value) => value.length > 0,
  );
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (key === null) continue;
    const stored = storage.getItem(key) ?? "";
    if (
      probes.some(
        (forbidden) => key.includes(forbidden) || stored.includes(forbidden),
      )
    ) {
      return true;
    }
  }
  return false;
}

export function summaryContentStorageProbes(content: SummaryContent): string[] {
  return [
    content.overview.text,
    ...content.decisions.map((item) => item.text),
    ...content.actionItems.flatMap((item) => [
      item.text,
      item.owner ?? "",
      item.dueDate ?? "",
    ]),
    ...content.risks.map((item) => item.text),
    ...content.openQuestions.map((item) => item.text),
  ].filter((value) => value.length > 0);
}

function mark(status: string): void {
  const marker =
    document.querySelector<HTMLOutputElement>(
      "[data-summary-run-core-smoke]",
    ) ?? document.body.appendChild(document.createElement("output"));
  marker.dataset.summaryRunCoreSmoke = status;
  marker.textContent = `Summary Run Core smoke: ${status}`;
  document.title = `MeetingLens summary run smoke: ${status}`;
}

function safeErrorCode(error: unknown): string {
  if (
    error instanceof SummaryApiError ||
    error instanceof TranscriptApiError ||
    error instanceof MeetingApiError
  ) {
    return error.code;
  }
  return smokeCheckpoint;
}

function requireCondition(
  condition: boolean,
  message = "summary_run_core_smoke_assertion_failed",
): asserts condition {
  if (!condition) throw new Error(message);
}
