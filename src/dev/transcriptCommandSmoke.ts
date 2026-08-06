import {
  MeetingApiError,
  meetingApi,
  type MeetingResponse,
} from "../api/meeting";
import {
  TranscriptApiError,
  transcriptApi,
  type CompletedTranscriptImportResult,
  type TranscriptSegmentResponse,
  type TranscriptSourceResponse,
} from "../api/transcript";

const smokeStateKey = "meetinglens.transcript-command-smoke.state.v1";
const primaryTitlePrefix = "Synthetic transcript command smoke";
const restartTitlePrefix = "Synthetic transcript command smoke restart";
const selectedFileTitlePrefix = "Synthetic transcript file smoke";
const cancelledTitlePrefix = "Synthetic transcript cancelled smoke";
const primaryLanguage = "en-AU";
let smokeCheckpoint = "startup";

interface SmokeState {
  runId: string;
  phase: "running" | "restart";
  primaryMeetingId: string | null;
  selectedFileMeetingId: string | null;
}

export async function runTranscriptCommandSmoke(): Promise<void> {
  let state = readState();
  try {
    if (state?.phase === "restart") {
      smokeCheckpoint = "restart-verification";
      await verifyRestartAndCleanUp(state);
      clearState();
      mark("passed");
      return;
    }

    if (state !== null) {
      await cleanUpOwnedMeetings(state);
      clearState();
    }

    state = {
      runId: crypto.randomUUID(),
      phase: "running",
      primaryMeetingId: null,
      selectedFileMeetingId: null,
    };
    writeState(state);
    smokeCheckpoint = "first-pass";
    await runFirstPass(state);
  } catch (error: unknown) {
    if (state !== null) {
      try {
        await cleanUpOwnedMeetings(state);
        clearState();
      } catch {
        // Keep the ID-only state so the next diagnostic run can retry cleanup safely.
      }
    }
    mark(`failed:${safeErrorCode(error)}`);
  }
}

async function runFirstPass(state: SmokeState): Promise<void> {
  const titles = titlesFor(state.runId);
  smokeCheckpoint = "import-policy";
  const policy = await transcriptApi.getImportPolicy();
  requireCondition(policy.allowedExtensions.join(",") === "txt,srt,vtt");
  requireCondition(policy.maxPasteBytes === 2 * 1024 * 1024);
  requireCondition(policy.maxFileBytes === 10 * 1024 * 1024);
  requireCondition(policy.maxSegmentsPerSource === 20_000);
  requireCondition(policy.maxSegmentBytes === 64 * 1024);

  smokeCheckpoint = "create-pasted-transcript";
  const pasted = requireCompleted(
    await transcriptApi.createMeetingFromPaste({
      title: titles.primary,
      primaryLanguage,
      displayName: "Synthetic pasted transcript",
      pastedText:
        "Speaker 1: Synthetic opening point\n\nSpeaker 2: Synthetic reply",
      language: primaryLanguage,
      baseStartedAt: "2026-08-06T00:00:00.000Z",
    }),
  );
  state.primaryMeetingId = pasted.meeting.id;
  writeState(state);
  requireCondition(!pasted.duplicate);
  requireCondition(pasted.meeting.status === "draft");
  requireCondition(pasted.source.sourceType === "pasted_text");
  requireCondition(pasted.segmentCount === 2);

  smokeCheckpoint = "cancel-picker";
  mark("cancel-picker");
  const cancelled = await transcriptApi.selectTranscriptAndCreateMeeting({
    title: titles.cancelled,
    primaryLanguage,
    language: primaryLanguage,
  });
  smokeCheckpoint = "cancel-result";
  requireCondition(cancelled.cancelled);
  smokeCheckpoint = "cancel-empty-meeting";
  const cancelledMeetings = (await meetingApi.list()).meetings.filter(
    (meeting) => meeting.title === titles.cancelled,
  );
  for (const meeting of cancelledMeetings) {
    await deleteOwnedMeeting(meeting, titles.cancelled);
  }
  requireCondition(cancelledMeetings.length === 0);

  smokeCheckpoint = "select-txt";
  mark("select-txt");
  const selectedTxt = requireCompleted(
    await transcriptApi.selectTranscriptAndCreateMeeting({
      title: titles.selectedFile,
      primaryLanguage,
      language: primaryLanguage,
    }),
  );
  state.selectedFileMeetingId = selectedTxt.meeting.id;
  writeState(state);
  requireCondition(selectedTxt.meeting.status === "draft");
  requireCondition(selectedTxt.source.sourceType === "imported_txt");

  smokeCheckpoint = "select-srt";
  mark("select-srt");
  const selectedSrt = requireCompleted(
    await transcriptApi.selectAndAddToMeeting({
      meetingId: pasted.meeting.id,
      language: primaryLanguage,
    }),
  );
  requireCondition(selectedSrt.source.sourceType === "imported_srt");
  const srtSegment = requireTimedSegment(
    await loadAllSegments(pasted.meeting.id, selectedSrt.source.id),
  );
  requireCondition(srtSegment.speakerLabel === "Speaker SRT");

  smokeCheckpoint = "select-vtt";
  mark("select-vtt");
  const selectedVtt = requireCompleted(
    await transcriptApi.selectAndAddToMeeting({
      meetingId: pasted.meeting.id,
      language: primaryLanguage,
    }),
  );
  requireCondition(selectedVtt.source.sourceType === "imported_vtt");
  const vttSegment = requireTimedSegment(
    await loadAllSegments(pasted.meeting.id, selectedVtt.source.id),
  );
  requireCondition(vttSegment.speakerLabel === "Speaker VTT");

  smokeCheckpoint = "verify-imports";
  const sources = await transcriptApi.listSources(pasted.meeting.id);
  requireCondition(sources.sources.length === 3);
  for (const source of sources.sources) {
    const fetched = await transcriptApi.getSource(pasted.meeting.id, source.id);
    requireCondition(fetched.id === source.id);
    requireCondition(fetched.meetingId === pasted.meeting.id);
  }

  smokeCheckpoint = "segment-correction";
  const pastedSegments = await loadAllSegments(
    pasted.meeting.id,
    pasted.source.id,
  );
  requireCondition(pastedSegments.length === 2);
  const original = requireFirst(pastedSegments);
  const corrected = await transcriptApi.updateSegment({
    meetingId: pasted.meeting.id,
    segmentId: original.id,
    speakerName: "Synthetic Speaker",
    originalText: "Corrected synthetic point",
    expectedRevision: original.revision,
  });
  requireCondition(corrected.revision === original.revision + 1);
  requireCondition(corrected.speakerName === "Synthetic Speaker");
  requireCondition(corrected.originalText === "Corrected synthetic point");
  await expectRevisionConflict(pasted.meeting.id, original);

  smokeCheckpoint = "speaker-batch-rename";
  const speakerGroups = await transcriptApi.listSpeakerGroups(
    pasted.meeting.id,
    pasted.source.id,
    { limit: 100 },
  );
  const batchGroup = speakerGroups.items.find(
    (group) => group.speakerLabel === "Speaker 2",
  );
  requireCondition(batchGroup?.segmentCount === 1);
  const renamed = await transcriptApi.renameSpeakerInSource({
    meetingId: pasted.meeting.id,
    sourceId: pasted.source.id,
    speakerLabel: "Speaker 2",
    speakerName: "Synthetic Batch Speaker",
  });
  requireCondition(renamed.updatedCount === 1);

  smokeCheckpoint = "source-delete";
  const deleted = await transcriptApi.deleteSource(
    pasted.meeting.id,
    selectedSrt.source.id,
  );
  requireCondition(deleted.deleted);
  requireCondition(
    !(await transcriptApi.listSources(pasted.meeting.id)).sources.some(
      (source) => source.id === selectedSrt.source.id,
    ),
  );

  smokeCheckpoint = "prepare-restart";
  const updated = await meetingApi.update({
    id: pasted.meeting.id,
    title: titles.restart,
  });
  requireCondition(updated.status === "draft");
  state.phase = "restart";
  writeState(state);
  mark("restart");
}

async function verifyRestartAndCleanUp(state: SmokeState): Promise<void> {
  requireCondition(state.primaryMeetingId !== null);
  requireCondition(state.selectedFileMeetingId !== null);
  const titles = titlesFor(state.runId);
  const persisted = await meetingApi.get(state.primaryMeetingId);
  const selectedFileMeeting = await meetingApi.get(state.selectedFileMeetingId);
  requireCondition(persisted.title === titles.restart);
  requireCondition(selectedFileMeeting.title === titles.selectedFile);
  requireCondition(persisted.status === "draft");

  const sources = (await transcriptApi.listSources(persisted.id)).sources;
  requireCondition(sources.length === 2);
  const pasted = requireSource(sources, "pasted_text");
  requireSource(sources, "imported_vtt");
  const segments = await loadAllSegments(persisted.id, pasted.id);
  requireCondition(
    segments.some(
      (segment) =>
        segment.speakerName === "Synthetic Speaker" &&
        segment.originalText === "Corrected synthetic point" &&
        segment.revision === 2,
    ),
  );
  requireCondition(
    segments.some(
      (segment) =>
        segment.speakerLabel === "Speaker 2" &&
        segment.speakerName === "Synthetic Batch Speaker" &&
        segment.revision === 2,
    ),
  );

  const fileSources = (await transcriptApi.listSources(selectedFileMeeting.id))
    .sources;
  const txt = requireSource(fileSources, "imported_txt");
  requireCondition(
    (await loadAllSegments(selectedFileMeeting.id, txt.id)).length > 0,
  );

  await deleteOwnedMeeting(persisted, titles.restart);
  await deleteOwnedMeeting(selectedFileMeeting, titles.selectedFile);
}

async function expectRevisionConflict(
  meetingId: string,
  original: TranscriptSegmentResponse,
): Promise<void> {
  try {
    await transcriptApi.updateSegment({
      meetingId,
      segmentId: original.id,
      originalText: "Stale synthetic correction",
      expectedRevision: original.revision,
    });
  } catch (error: unknown) {
    requireCondition(
      error instanceof TranscriptApiError &&
        error.code === "transcript_revision_conflict",
    );
    return;
  }
  throw new Error("revision_conflict_expected");
}

async function cleanUpOwnedMeetings(state: SmokeState): Promise<void> {
  const titles = titlesFor(state.runId);
  if (state.primaryMeetingId !== null) {
    await deleteOwnedMeetingIfPresent(state.primaryMeetingId, [
      titles.primary,
      titles.restart,
    ]);
  }
  if (state.selectedFileMeetingId !== null) {
    await deleteOwnedMeetingIfPresent(state.selectedFileMeetingId, [
      titles.selectedFile,
    ]);
  }
}

async function deleteOwnedMeetingIfPresent(
  meetingId: string,
  expectedTitles: string[],
): Promise<void> {
  try {
    const meeting = await meetingApi.get(meetingId);
    requireCondition(expectedTitles.includes(meeting.title));
    await deleteOwnedMeeting(meeting, meeting.title);
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

async function deleteOwnedMeeting(
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

function requireCompleted(
  result: Awaited<ReturnType<typeof transcriptApi.createMeetingFromPaste>>,
): CompletedTranscriptImportResult {
  requireCondition(!result.cancelled);
  return result;
}

function requireFirst(
  segments: TranscriptSegmentResponse[],
): TranscriptSegmentResponse {
  const first = segments[0];
  requireCondition(first !== undefined);
  return first;
}

function requireTimedSegment(
  segments: TranscriptSegmentResponse[],
): TranscriptSegmentResponse {
  const segment = requireFirst(segments);
  requireCondition(segment.startOffsetMs !== null);
  requireCondition(segment.endOffsetMs !== null);
  requireCondition(segment.endOffsetMs >= segment.startOffsetMs);
  return segment;
}

function requireSource(
  sources: TranscriptSourceResponse[],
  sourceType: TranscriptSourceResponse["sourceType"],
): TranscriptSourceResponse {
  const source = sources.find(
    (candidate) => candidate.sourceType === sourceType,
  );
  requireCondition(source !== undefined);
  return source;
}

async function loadAllSegments(
  meetingId: string,
  sourceId: string,
): Promise<TranscriptSegmentResponse[]> {
  const segments: TranscriptSegmentResponse[] = [];
  let cursor: number | null = null;
  let expectedTotal: number | null = null;
  do {
    const page = await transcriptApi.listSegments(meetingId, sourceId, {
      cursor,
      limit: 100,
    });
    expectedTotal ??= page.totalCount;
    requireCondition(page.totalCount === expectedTotal);
    segments.push(...page.items);
    cursor = page.nextCursor;
    if (!page.hasMore) break;
    requireCondition(cursor !== null);
  } while (segments.length <= 20_000);
  requireCondition(expectedTotal === segments.length);
  return segments;
}

function titlesFor(runId: string) {
  return {
    primary: `${primaryTitlePrefix} ${runId}`,
    restart: `${restartTitlePrefix} ${runId}`,
    selectedFile: `${selectedFileTitlePrefix} ${runId}`,
    cancelled: `${cancelledTitlePrefix} ${runId}`,
  };
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
      !isNullableUuid(value.primaryMeetingId) ||
      !isNullableUuid(value.selectedFileMeetingId)
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

function writeState(state: SmokeState): void {
  window.localStorage.setItem(smokeStateKey, JSON.stringify(state));
}

function clearState(): void {
  window.localStorage.removeItem(smokeStateKey);
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
    document.querySelector<HTMLOutputElement>(
      "[data-transcript-command-smoke]",
    ) ?? document.body.appendChild(document.createElement("output"));
  marker.dataset.transcriptCommandSmoke = status;
  marker.textContent = `Transcript command smoke: ${status}`;
  document.title = `MeetingLens transcript smoke: ${status}`;
}

function safeErrorCode(error: unknown): string {
  if (error instanceof TranscriptApiError || error instanceof MeetingApiError) {
    return error.code;
  }
  return smokeCheckpoint;
}

function requireCondition(
  condition: boolean,
  message = "transcript_command_smoke_assertion_failed",
): asserts condition {
  if (!condition) throw new Error(message);
}
