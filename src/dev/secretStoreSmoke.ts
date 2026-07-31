const PHASE_KEY = "meetinglens-secret-store-smoke-phase";
const MEETING_TITLE = "MeetingLens synthetic SecretStore acceptance";

type SmokePhase = "saved" | "overwritten" | "deleted";

export async function runSecretStoreSmoke(): Promise<void> {
  let checkpoint = "startup";
  try {
    navigate("/settings");
    const phase = readPhase();
    if (phase === null) {
      checkpoint = "initial-status";
      await expectSecretStatus("Not configured");
      checkpoint = "initial-save";
      await saveGeneratedSecret();
      writePhase("saved");
      mark("restart-saved");
      return;
    }
    if (phase === "saved") {
      checkpoint = "restart-persistence";
      await expectConfiguredWithoutPrefill();
      checkpoint = "overwrite";
      await saveGeneratedSecret();
      writePhase("overwritten");
      mark("restart-overwritten");
      return;
    }
    if (phase === "overwritten") {
      checkpoint = "overwrite-persistence";
      await expectConfiguredWithoutPrefill();
      checkpoint = "delete";
      click(await element("delete-deepseek-secret"));
      click(await element("confirm-secret-delete"));
      await expectSecretStatus("Not configured");
      writePhase("deleted");
      mark("restart-deleted");
      return;
    }

    checkpoint = "delete-persistence";
    await expectSecretStatus("Not configured");
    checkpoint = "sidecar-regression";
    const sidecar = await element("sidecar-security-status");
    requireText(sidecar, "Local Sidecar authenticated");
    checkpoint = "provider-regression";
    const providers = await element("provider-statuses");
    requireText(providers, "Mock");
    checkpoint = "meeting-regression";
    await completeSyntheticMeeting();
    localStorage.removeItem(PHASE_KEY);
    mark("passed");
  } catch {
    mark(`failed-${checkpoint}`);
  }
}

async function saveGeneratedSecret(): Promise<void> {
  const input = await element<HTMLInputElement>("deepseek-secret-input");
  setInputValue(input, generatedSecret());
  const save = await element<HTMLButtonElement>("save-deepseek-secret");
  await waitFor(() => (!save.disabled ? true : null));
  click(save);
  await expectSecretStatus("Configured");
  await waitFor(() => (input.value === "" ? true : null));
}

async function expectConfiguredWithoutPrefill(): Promise<void> {
  await expectSecretStatus("Configured");
  const input = await element<HTMLInputElement>("deepseek-secret-input");
  if (input.value !== "") throw new Error("Stored secret was refilled.");
}

async function expectSecretStatus(expected: string): Promise<void> {
  const settings = await element("deepseek-secret-settings");
  await waitFor(() =>
    settings.textContent?.includes(`Configuration: ${expected}`) ? true : null,
  );
}

async function completeSyntheticMeeting(): Promise<void> {
  navigate("/new");
  const input = await element<HTMLInputElement>("new-title");
  setInputValue(input, MEETING_TITLE);
  click(await element("create-meeting"));
  await element("meeting-detail");
  click(await element("start-meeting"));
  await element("live-meeting");
  await waitFor(() =>
    document.querySelectorAll("[data-testid='transcript-stream'] li").length >=
    2
      ? true
      : null,
  );
  click(await element("end-meeting"));
  await element("meeting-detail");
  click(await element("delete-meeting"));
  click(await element("confirm-delete"));
  await waitFor(() => (window.location.hash === "#/history" ? true : null));
}

function generatedSecret(): string {
  return `${crypto.randomUUID()}${crypto.randomUUID()}`;
}

function readPhase(): SmokePhase | null {
  const value = localStorage.getItem(PHASE_KEY);
  return value === "saved" || value === "overwritten" || value === "deleted"
    ? value
    : null;
}

function writePhase(phase: SmokePhase): void {
  localStorage.setItem(PHASE_KEY, phase);
}

function navigate(path: string): void {
  window.location.hash = path;
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

function requireText(target: HTMLElement, expected: string): void {
  if (!target.textContent?.includes(expected)) {
    throw new Error("Acceptance status was incomplete.");
  }
}

function mark(status: string): void {
  document.documentElement.dataset.secretStoreAcceptance = status;
  document.title = `MeetingLens SecretStore acceptance: ${status}`;
  const marker =
    document.querySelector<HTMLOutputElement>("[data-secret-store-smoke]") ??
    document.createElement("output");
  marker.dataset.secretStoreSmoke = status;
  marker.textContent = `SecretStore smoke: ${status}`;
  marker.style.cssText = [
    "position:fixed",
    "right:16px",
    "bottom:16px",
    "z-index:2147483647",
    "padding:10px 14px",
    "border-radius:8px",
    "background:#10243b",
    "color:#ffffff",
    "font:600 14px system-ui",
  ].join(";");
  if (!marker.isConnected) document.body.append(marker);
}

async function waitFor<T>(
  read: () => T | null,
  timeoutMs = 10_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== null) return value;
    await new Promise((resolve) => window.setTimeout(resolve, 50));
  }
  throw new Error("Timed out during SecretStore acceptance.");
}
