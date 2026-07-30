const TITLE = "MeetingLens sidecar auth acceptance";

export async function runSidecarAuthSmoke(): Promise<void> {
  let checkpoint = "startup";
  try {
    checkpoint = "authentication-wait";
    navigate("/settings");
    const status = await waitFor(() => {
      const current = document.querySelector<HTMLElement>(
        '[data-acceptance="sidecar-security-status"]',
      );
      return current?.textContent?.includes("Local Sidecar authenticated")
        ? current
        : null;
    });
    checkpoint = "authentication-service";
    assertIncludes(status, "Local Sidecar authenticated");
    checkpoint = "authentication-binding";
    assertIncludes(status, "This device only");
    checkpoint = "authentication-session";
    assertIncludes(status, "Current application session");
    checkpoint = "authentication-secret-storage";
    assertIncludes(status, "Not implemented");
    mark("authenticated");
    await delay(5_000);

    checkpoint = "create";
    navigate("/new");
    const input = await element<HTMLInputElement>("new-title");
    setInputValue(input, TITLE);
    click(await element<HTMLButtonElement>("create-meeting"));
    await element("meeting-detail");
    checkpoint = "start";
    click(await element<HTMLButtonElement>("start-meeting"));
    await element("live-meeting");
    checkpoint = "transcript";
    await waitFor(() => (transcriptCount() >= 2 ? true : null), 10_000);
    checkpoint = "end";
    click(await element<HTMLButtonElement>("end-meeting"));
    await element("meeting-detail");
    checkpoint = "delete";
    click(await element<HTMLButtonElement>("delete-meeting"));
    click(await element<HTMLButtonElement>("confirm-delete"));
    await waitFor(() => (window.location.hash === "#/history" ? true : null));
    mark("passed");
  } catch {
    mark(`failed-${checkpoint}`);
  }
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

function assertIncludes(target: HTMLElement, expected: string): void {
  if (!target.textContent?.includes(expected)) {
    throw new Error("The safe Sidecar status is incomplete.");
  }
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

function mark(status: string): void {
  document.documentElement.dataset.sidecarAuthAcceptance = status;
  document.title = `MeetingLens Sidecar auth acceptance: ${status}`;

  const marker =
    document.querySelector<HTMLOutputElement>("[data-sidecar-auth-smoke]") ??
    document.createElement("output");
  marker.dataset.sidecarAuthSmoke = status;
  marker.textContent = `Sidecar authentication smoke: ${status}`;
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

async function waitFor<T>(read: () => T | null, timeoutMs = 8_000): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = read();
    if (value !== null) return value;
    await delay(50);
  }
  throw new Error("Timed out during Sidecar authentication acceptance.");
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}
