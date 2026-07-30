const STORAGE_KEY = "meetinglens-ui-acceptance-v1";
const INITIAL_TITLE = "MeetingLens synthetic UI acceptance";
const EDITED_TITLE = "MeetingLens synthetic UI acceptance edited";

interface SmokeState {
  id: string;
  phase: "draft" | "ready";
}

export async function runMeetingUiSmoke(): Promise<void> {
  try {
    const state = readState();
    if (state === null) {
      await createAndEditDraft();
      return;
    }
    if (state.phase === "draft") {
      await completeMeeting(state);
      return;
    }
    await deleteReadyMeeting(state);
  } catch {
    document.documentElement.dataset.meetinglensAcceptance = "failed";
  }
}

async function createAndEditDraft(): Promise<void> {
  navigate("/new");
  const titleInput = await element<HTMLInputElement>("new-title");
  setInputValue(titleInput, INITIAL_TITLE);
  click(await element<HTMLButtonElement>("create-meeting"));
  await element("meeting-detail");

  const id = meetingIdFromHash();
  click(await element<HTMLButtonElement>("edit-title"));
  const editInput = await element<HTMLInputElement>("edit-title-input");
  setInputValue(editInput, EDITED_TITLE);
  click(await element<HTMLButtonElement>("save-title"));
  await waitFor(() =>
    document.querySelector("[data-acceptance='meeting-detail'] h1")
      ?.textContent === EDITED_TITLE
      ? true
      : null,
  );

  writeState({ id, phase: "draft" });
  mark("restart-draft");
}

async function completeMeeting(state: SmokeState): Promise<void> {
  navigate("/");
  await meetingRow(state.id);
  navigate(`/meetings/${encodeURIComponent(state.id)}`);
  await element("meeting-detail");
  assertText("meeting-detail", EDITED_TITLE);
  click(await element<HTMLButtonElement>("start-meeting"));
  await element("live-meeting");

  await waitFor(() => (transcriptCount() >= 3 ? true : null), 12_000);
  click(await element<HTMLButtonElement>("pause-transcript"));
  const pausedCount = transcriptCount();
  await delay(2_500);
  if (transcriptCount() !== pausedCount) {
    throw new Error("Synthetic transcript continued while paused.");
  }
  assertText("recording-delete-restriction", "meeting_invalid_state");

  // Keep the native recording workspace visible long enough for local QA evidence.
  mark("capture-live");
  await delay(10_000);
  click(await element<HTMLButtonElement>("resume-transcript"));
  await waitFor(() => (transcriptCount() > pausedCount ? true : null));
  click(await element<HTMLButtonElement>("end-meeting"));
  await element("meeting-detail");
  assertText("meeting-detail", "Ready");

  navigate("/history");
  await meetingRow(state.id);
  writeState({ id: state.id, phase: "ready" });
  mark("restart-ready");
}

async function deleteReadyMeeting(state: SmokeState): Promise<void> {
  navigate("/");
  await meetingRow(state.id);
  navigate(`/meetings/${encodeURIComponent(state.id)}`);
  await element("meeting-detail");
  assertText("meeting-detail", "Ready");
  click(await element<HTMLButtonElement>("delete-meeting"));
  click(await element<HTMLButtonElement>("confirm-delete"));
  await waitFor(() => (window.location.hash === "#/history" ? true : null));
  await waitFor(() =>
    document.querySelector(`[data-meeting-id="${state.id}"]`) === null
      ? true
      : null,
  );
  localStorage.removeItem(STORAGE_KEY);
  mark("passed");
}

function navigate(path: string): void {
  window.location.hash = path;
}

function meetingIdFromHash(): string {
  const match = window.location.hash.match(/^#\/meetings\/([^/]+)$/);
  if (!match) throw new Error("Meeting detail route was not reached.");
  return decodeURIComponent(match[1]);
}

async function meetingRow(id: string): Promise<HTMLElement> {
  return waitFor(() =>
    document.querySelector<HTMLElement>(`[data-meeting-id="${id}"]`),
  );
}

async function element<T extends HTMLElement = HTMLElement>(
  name: string,
): Promise<T> {
  return waitFor(() =>
    document.querySelector<T>(`[data-acceptance="${name}"]`),
  );
}

function click(target: HTMLElement): void {
  target.click();
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function transcriptCount(): number {
  return document.querySelectorAll("[data-testid='transcript-stream'] li")
    .length;
}

function assertText(name: string, expected: string): void {
  const text = document.querySelector(
    `[data-acceptance="${name}"]`,
  )?.textContent;
  if (!text?.includes(expected)) {
    throw new Error("Acceptance text was not present.");
  }
}

function readState(): SmokeState | null {
  const value = localStorage.getItem(STORAGE_KEY);
  if (value === null) return null;
  const parsed = JSON.parse(value) as Partial<SmokeState>;
  if (
    typeof parsed.id !== "string" ||
    (parsed.phase !== "draft" && parsed.phase !== "ready")
  ) {
    throw new Error("Acceptance state is invalid.");
  }
  return parsed as SmokeState;
}

function writeState(state: SmokeState): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function mark(status: string): void {
  document.documentElement.dataset.meetinglensAcceptance = status;
  document.title = `MeetingLens UI acceptance: ${status}`;
}

async function waitFor<T>(read: () => T | null, timeoutMs = 8_000): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== null) return value;
    await delay(50);
  }
  throw new Error("Timed out during UI acceptance.");
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}
