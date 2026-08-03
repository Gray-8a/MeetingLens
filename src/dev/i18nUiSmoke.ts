import { meetingApi } from "../api/meeting";
import { UI_LOCALE_STORAGE_KEY } from "../i18n";

const SMOKE_TITLE = "Synthetic i18n restart acceptance";
const PHASE = import.meta.env.VITE_I18N_UI_SMOKE_PHASE;

export async function runI18nUiSmoke(): Promise<void> {
  try {
    if (PHASE === "zh") {
      await verifyChineseFlow();
    } else if (PHASE === "en") {
      await verifyEnglishRestart();
    } else if (PHASE === "final") {
      await verifyFinalRestart();
    } else {
      throw new Error("unknown i18n smoke phase");
    }
  } catch {
    mark("failed");
  }
}

async function verifyChineseFlow(): Promise<void> {
  for (const meeting of (await meetingApi.list()).meetings.filter(
    (candidate) => candidate.title === SMOKE_TITLE,
  )) {
    if (meeting.status === "recording") await meetingApi.end(meeting.id);
    await meetingApi.delete(meeting.id);
  }

  navigate("/settings");
  await element("language-settings");
  selectLocale("zh-CN");
  await waitFor(() =>
    document.documentElement.lang === "zh-CN" &&
    localStorage.getItem(UI_LOCALE_STORAGE_KEY) === "zh-CN"
      ? true
      : null,
  );
  assertPageText("首页");
  assertPageText("界面语言与未来的会议纪要输出语言可以分别配置。");
  assertPageText("Provider 运行环境");
  await pageText("模拟");

  navigate("/new");
  const title = await element<HTMLInputElement>("new-title");
  setInputValue(title, SMOKE_TITLE);
  click(await element<HTMLButtonElement>("create-meeting"));
  await element("meeting-detail");
  assertPageText("待准备");
  click(await element<HTMLButtonElement>("start-meeting"));
  await element("live-meeting");
  assertPageText("会议进行中");
  assertPageText("模拟会议");
  await waitFor(() =>
    document.querySelectorAll("[data-testid='transcript-stream'] li").length > 0
      ? true
      : null,
  );
  click(await element<HTMLButtonElement>("end-meeting"));
  await element("meeting-detail");
  assertPageText("可继续处理");

  navigate("/help");
  await element("help-page");
  assertPageText("开发预览版");
  assertPageText("真实录音、实时转录");
  mark("restart-zh");
}

async function verifyEnglishRestart(): Promise<void> {
  requireCondition(document.documentElement.lang === "zh-CN");
  requireCondition(localStorage.getItem(UI_LOCALE_STORAGE_KEY) === "zh-CN");
  const meeting = await uniqueSmokeMeeting();
  requireCondition(meeting.status === "ready");

  navigate(`/meetings/${meeting.id}`);
  await element("meeting-detail");
  assertPageText(SMOKE_TITLE);
  assertPageText("可继续处理");

  navigate("/settings");
  await element("language-settings");
  selectLocale("en-US");
  await waitFor(() =>
    document.documentElement.lang === "en-US" &&
    localStorage.getItem(UI_LOCALE_STORAGE_KEY) === "en-US"
      ? true
      : null,
  );
  assertPageText("Home");
  assertPageText("Interface language");
  await pageText("Mock");

  navigate(`/meetings/${meeting.id}`);
  await element("meeting-detail");
  assertPageText("Ready");
  click(await element<HTMLButtonElement>("delete-meeting"));
  click(await element<HTMLButtonElement>("confirm-delete"));
  await waitFor(() => (window.location.hash === "#/history" ? true : null));

  navigate("/meetings/00000000-0000-4000-8000-000000000000");
  await waitFor(() => document.querySelector("[role='alert']"));
  assertPageText("This meeting no longer exists.");
  assertPageText("meeting_not_found");

  navigate("/help");
  await element("help-page");
  assertPageText("Development Preview");
  assertPageText("Real recording, realtime transcription");
  mark("restart-en");
}

async function verifyFinalRestart(): Promise<void> {
  requireCondition(document.documentElement.lang === "en-US");
  requireCondition(localStorage.getItem(UI_LOCALE_STORAGE_KEY) === "en-US");
  requireCondition(
    !(await meetingApi.list()).meetings.some(
      (candidate) => candidate.title === SMOKE_TITLE,
    ),
  );
  navigate("/settings");
  await element("sidecar-security-status");
  await pageText("Local Sidecar authenticated");
  await pageText("Provider runtime");
  const secretSettings = await element("deepseek-secret-settings");
  await waitFor(() => {
    const status =
      secretSettings.querySelector<HTMLElement>(".secret-status")?.dataset
        .status;
    return status === "configured" || status === "unconfigured" ? true : null;
  });
  mark("passed");
}

async function uniqueSmokeMeeting() {
  const matches = (await meetingApi.list()).meetings.filter(
    (candidate) => candidate.title === SMOKE_TITLE,
  );
  requireCondition(matches.length === 1);
  return matches[0];
}

function navigate(path: string): void {
  window.location.hash = path;
}

function selectLocale(locale: string): void {
  const select = document.querySelector<HTMLSelectElement>(
    "[data-acceptance='ui-language']",
  );
  requireCondition(select !== null);
  const setter = Object.getOwnPropertyDescriptor(
    HTMLSelectElement.prototype,
    "value",
  )?.set;
  setter?.call(select, locale);
  select.dispatchEvent(new Event("change", { bubbles: true }));
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

async function element<T extends HTMLElement = HTMLElement>(name: string) {
  return waitFor(() =>
    document.querySelector<T>(`[data-acceptance="${name}"]`),
  );
}

function assertPageText(expected: string): void {
  requireCondition(document.body.textContent?.includes(expected) === true);
}

async function pageText(expected: string): Promise<void> {
  await waitFor(() =>
    document.body.textContent?.includes(expected) === true ? true : null,
  );
}

function mark(status: string): void {
  document.documentElement.dataset.meetinglensAcceptance = status;
  document.title = `MeetingLens i18n acceptance: ${status}`;
}

function requireCondition(condition: boolean): asserts condition {
  if (!condition) throw new Error("i18n smoke assertion failed");
}

async function waitFor<T>(
  read: () => T | null | Promise<T | null>,
  timeoutMs = 15_000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await read();
    if (value !== null) return value;
    await new Promise((resolve) => window.setTimeout(resolve, 50));
  }
  throw new Error("i18n smoke timed out");
}
