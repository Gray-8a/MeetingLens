const FIRST_TITLE = "MeetingLens provider acceptance one";
const SECOND_TITLE = "MeetingLens provider acceptance two";

export async function runProviderUiSmoke(): Promise<void> {
  try {
    await verifySettings();
    const firstId = await createAndStart(FIRST_TITLE);
    await verifyTranscriptControls();
    await endAndDelete(firstId);

    const secondId = await createAndStart(SECOND_TITLE);
    await verifySecondSession();
    await delay(8_000);
    await endAndDelete(secondId);
    mark("passed");
  } catch {
    mark("failed");
  }
}

async function verifySettings(): Promise<void> {
  navigate("/settings");
  const runtime = await element("provider-statuses");
  const rows = [...runtime.querySelectorAll("li")];
  if (
    rows.length !== 5 ||
    rows.some(
      (row) =>
        !row.textContent?.includes("Mock") ||
        !row.textContent.includes("Not enabled"),
    )
  ) {
    throw new Error("Provider settings are incomplete.");
  }
  mark("capture-settings");
  await delay(5_000);
}

async function createAndStart(title: string): Promise<string> {
  navigate("/new");
  const input = await element<HTMLInputElement>("new-title");
  setInputValue(input, title);
  click(await element<HTMLButtonElement>("create-meeting"));
  await element("meeting-detail");
  const id = meetingIdFromHash();
  click(await element<HTMLButtonElement>("start-meeting"));
  await element("live-meeting");
  return id;
}

async function verifyTranscriptControls(): Promise<void> {
  await waitFor(() => (transcriptCount() >= 3 ? true : null), 12_000);
  click(await element<HTMLButtonElement>("pause-transcript"));
  const pausedCount = transcriptCount();
  await delay(2_500);
  if (transcriptCount() !== pausedCount) {
    throw new Error("Transcription Provider continued while paused.");
  }
  click(await element<HTMLButtonElement>("resume-transcript"));
  await waitFor(() => (transcriptCount() > pausedCount ? true : null), 8_000);
}

async function verifySecondSession(): Promise<void> {
  await waitFor(() => (transcriptCount() >= 2 ? true : null), 10_000);
  const before = transcriptCount();
  await delay(2_000);
  const growth = transcriptCount() - before;
  if (growth < 0 || growth > 2) {
    throw new Error("Transcription subscription multiplied unexpectedly.");
  }
  mark("capture-second-session");
}

async function endAndDelete(id: string): Promise<void> {
  click(await element<HTMLButtonElement>("end-meeting"));
  await element("meeting-detail");
  assertText("meeting-detail", "Ready");
  click(await element<HTMLButtonElement>("delete-meeting"));
  click(await element<HTMLButtonElement>("confirm-delete"));
  await waitFor(() => (window.location.hash === "#/history" ? true : null));
  await waitFor(() =>
    document.querySelector(`[data-meeting-id="${id}"]`) === null ? true : null,
  );
}

function navigate(path: string): void {
  window.location.hash = path;
}

function meetingIdFromHash(): string {
  const match = window.location.hash.match(/^#\/meetings\/([^/]+)$/);
  if (!match) throw new Error("Meeting detail route was not reached.");
  return decodeURIComponent(match[1]);
}

async function element<T extends HTMLElement = HTMLElement>(
  name: string,
): Promise<T> {
  return waitFor(() =>
    document.querySelector<T>(`[data-acceptance="${name}"]`),
  );
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function click(target: HTMLElement): void {
  target.click();
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

function mark(status: string): void {
  document.documentElement.dataset.providerAcceptance = status;
  document.title = `MeetingLens Provider acceptance: ${status}`;
}

async function waitFor<T>(read: () => T | null, timeoutMs = 8_000): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== null) return value;
    await delay(50);
  }
  throw new Error("Timed out during Provider UI acceptance.");
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}
