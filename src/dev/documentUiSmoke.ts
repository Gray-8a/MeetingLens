import { documentApi } from "../api/document";
import { meetingApi } from "../api/meeting";

const SMOKE_MEETING_TITLE = "Synthetic native document UI smoke";

interface SmokeState {
  meetingId: string;
  phase: "imported" | "ready";
}

export async function runDocumentUiSmoke(): Promise<void> {
  try {
    const state = await resolveState();
    if (state === null) {
      await importIntoDraft();
    } else if (state.phase === "imported") {
      await verifyRestartAndImportWhileRecording(state);
    } else {
      await verifyReadyRestartAndDelete(state);
    }
  } catch {
    mark("failed");
  }
}

async function importIntoDraft(): Promise<void> {
  const previousSmokeMeetings = (await meetingApi.list()).meetings.filter(
    (meeting) =>
      meeting.title === SMOKE_MEETING_TITLE &&
      (meeting.status === "draft" || meeting.status === "ready"),
  );
  for (const meeting of previousSmokeMeetings) {
    await meetingApi.delete(meeting.id);
  }
  const meeting = await meetingApi.create({
    title: SMOKE_MEETING_TITLE,
    primaryLanguage: "en-AU",
  });
  navigate(`/meetings/${meeting.id}`);
  await element("document-panel");

  mark("picker-cancel");
  click(await element<HTMLButtonElement>("document-add-files"));
  await waitForPickerCompletion("document-add-files");
  requireCondition((await documentApi.list(meeting.id)).documents.length === 0);

  mark("picker-draft");
  click(await element<HTMLButtonElement>("document-add-files"));
  await waitForPickerCompletion("document-add-files", 30_000);
  const documents = await documentApi.list(meeting.id);
  requireCondition(documents.documents.length === 3);
  requireCondition(
    documents.documents.filter((document) => document.status === "staged")
      .length === 2,
  );
  requireCondition(
    documents.documents.filter(
      (document) => document.status === "import_failed",
    ).length === 1,
  );
  await waitFor(() =>
    document.querySelectorAll("[data-acceptance='document-list'] > li")
      .length === 3
      ? true
      : null,
  );

  mark("restart-imported");
}

async function verifyRestartAndImportWhileRecording(
  state: SmokeState,
): Promise<void> {
  const persisted = await documentApi.list(state.meetingId);
  requireCondition(persisted.documents.length === 3);
  const recording = await meetingApi.start(state.meetingId);
  requireCondition(recording.status === "recording");
  navigate(`/meetings/${state.meetingId}/live`);
  await element("live-meeting");
  await waitFor(() =>
    document.querySelectorAll("[data-testid='transcript-stream'] li").length > 0
      ? true
      : null,
  );

  mark("picker-recording");
  click(await element<HTMLButtonElement>("document-add-files-live"));
  await waitForPickerCompletion("document-add-files-live", 30_000);
  requireCondition(
    (await documentApi.list(state.meetingId)).documents.length === 4,
  );
  requireCondition(
    document.querySelectorAll("[data-testid='transcript-stream'] li").length >
      0,
  );

  click(await element<HTMLButtonElement>("end-meeting"));
  await element("meeting-detail");
  requireCondition((await meetingApi.get(state.meetingId)).status === "ready");
  mark("restart-ready");
}

async function verifyReadyRestartAndDelete(state: SmokeState): Promise<void> {
  const meeting = await meetingApi.get(state.meetingId);
  requireCondition(meeting.status === "ready");
  navigate(`/meetings/${state.meetingId}`);
  await element("document-panel");
  await waitFor(() =>
    document.querySelectorAll("[data-acceptance='document-list'] > li")
      .length === 4
      ? true
      : null,
  );

  const deleteDocument = await waitFor(() =>
    document.querySelector<HTMLButtonElement>(
      "[data-acceptance='document-list'] button[aria-label^='Delete ']",
    ),
  );
  click(deleteDocument);
  const panel = await element("document-panel");
  click(
    await waitFor(
      () =>
        [...panel.querySelectorAll<HTMLButtonElement>("button")].find(
          (button) => button.textContent === "Confirm",
        ) ?? null,
    ),
  );
  await waitFor(() =>
    document.querySelectorAll("[data-acceptance='document-list'] > li")
      .length === 3
      ? true
      : null,
  );

  click(await element<HTMLButtonElement>("delete-meeting"));
  click(await element<HTMLButtonElement>("confirm-delete"));
  await waitFor(() => (window.location.hash === "#/history" ? true : null));
  await waitFor(async () => {
    const meetings = await meetingApi.list();
    return meetings.meetings.some(
      (candidate) => candidate.id === state.meetingId,
    )
      ? null
      : true;
  });
  mark("passed");
}

async function waitForPickerCompletion(
  acceptance: string,
  timeoutMs = 15_000,
): Promise<void> {
  await waitFor(() => {
    const button = document.querySelector<HTMLButtonElement>(
      `[data-acceptance="${acceptance}"]`,
    );
    return button && (button.disabled || button.textContent !== "Add files")
      ? true
      : null;
  }, timeoutMs);
  await waitFor(() => {
    const button = document.querySelector<HTMLButtonElement>(
      `[data-acceptance="${acceptance}"]`,
    );
    return button && !button.disabled && button.textContent === "Add files"
      ? true
      : null;
  }, timeoutMs);
}

function navigate(path: string): void {
  window.location.hash = path;
}

function click(target: HTMLElement): void {
  target.click();
}

async function element<T extends HTMLElement = HTMLElement>(
  name: string,
): Promise<T> {
  return waitFor(() =>
    document.querySelector<T>(`[data-acceptance="${name}"]`),
  );
}

async function resolveState(): Promise<SmokeState | null> {
  const candidates = (await meetingApi.list()).meetings
    .filter(
      (meeting) =>
        meeting.title === SMOKE_MEETING_TITLE &&
        (meeting.status === "draft" || meeting.status === "ready"),
    )
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
  if (candidates.length !== 1) return null;

  const meeting = candidates[0];
  const documents = (await documentApi.list(meeting.id)).documents;
  const staged = documents.filter(
    (document) => document.status === "staged",
  ).length;
  const failed = documents.filter(
    (document) => document.status === "import_failed",
  ).length;

  if (
    meeting.status === "draft" &&
    documents.length === 3 &&
    staged === 2 &&
    failed === 1
  ) {
    return { meetingId: meeting.id, phase: "imported" };
  }
  if (
    meeting.status === "ready" &&
    documents.length === 4 &&
    staged === 3 &&
    failed === 1
  ) {
    return { meetingId: meeting.id, phase: "ready" };
  }
  return null;
}

function mark(status: string): void {
  document.documentElement.dataset.meetinglensAcceptance = status;
  document.title = `MeetingLens document acceptance: ${status}`;
}

function requireCondition(condition: boolean): asserts condition {
  if (!condition) throw new Error("document smoke assertion failed");
}

async function waitFor<T>(
  read: () => T | null | Promise<T | null>,
  timeoutMs = 10_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (value !== null) return value;
    await new Promise((resolve) => window.setTimeout(resolve, 50));
  }
  throw new Error("document smoke timed out");
}
