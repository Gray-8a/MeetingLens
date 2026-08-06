import appSource from "../App.tsx?raw";
import documentSource from "../components/DocumentPanel.tsx?raw";
import historySource from "../pages/HistoryPage.tsx?raw";
import homeSource from "../pages/HomePage.tsx?raw";
import liveSource from "../pages/LiveMeetingPage.tsx?raw";
import detailSource from "../pages/MeetingDetailPage.tsx?raw";
import newSource from "../pages/NewMeetingPage.tsx?raw";
import settingsSource from "../pages/SettingsPage.tsx?raw";
import { MeetingApiError } from "../api/meeting";
import { transcriptErrorMessage } from "./errors";
import { meetingErrorMessage, statusLabel, uiError } from "../lib/presentation";
import i18n, {
  changeUiLocale,
  localeFromSystem,
  resolveInitialUiLocale,
  UI_LOCALE_STORAGE_KEY,
} from ".";
import { formatFileSize, formatLocalDate } from "./formatters";
import enUS from "./locales/en-US.json";
import zhCN from "./locales/zh-CN.json";

const uiSources = [
  appSource,
  documentSource,
  historySource,
  homeSource,
  liveSource,
  detailSource,
  newSource,
  settingsSource,
].join("\n");

afterEach(async () => {
  await i18n.changeLanguage("en-US");
});

describe("MeetingLens i18n foundation", () => {
  it("initializes zh-CN resources", async () => {
    await i18n.changeLanguage("zh-CN");
    expect(i18n.resolvedLanguage).toBe("zh-CN");
    expect(i18n.t("app.nav.home")).toBe("首页");
  });

  it("initializes en-US resources", async () => {
    await i18n.changeLanguage("en-US");
    expect(i18n.resolvedLanguage).toBe("en-US");
    expect(i18n.t("app.nav.home")).toBe("Home");
  });

  it("selects zh-CN for a Chinese system locale", () => {
    expect(localeFromSystem("zh-Hans-CN")).toBe("zh-CN");
    expect(resolveInitialUiLocale(undefined, "zh-TW")).toBe("zh-CN");
  });

  it("selects en-US for a non-Chinese system locale", () => {
    expect(localeFromSystem("en-AU")).toBe("en-US");
    expect(localeFromSystem("fr-FR")).toBe("en-US");
  });

  it("lets an allowed user preference override the system locale", () => {
    const storage = { getItem: () => "en-US" };
    expect(resolveInitialUiLocale(storage, "zh-CN")).toBe("en-US");
  });

  it("persists only the allowed locale for the next startup", async () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    };
    await changeUiLocale("zh-CN", storage);
    expect([...values.entries()]).toEqual([[UI_LOCALE_STORAGE_KEY, "zh-CN"]]);
    expect(resolveInitialUiLocale(storage, "en-US")).toBe("zh-CN");
  });

  it("falls back safely from an unknown stored locale", () => {
    const storage = { getItem: () => "../../remote-locale" };
    expect(resolveInitialUiLocale(storage, "en-AU")).toBe("en-US");
  });

  it("falls back safely when local preference storage is damaged", () => {
    const storage = {
      getItem: () => {
        throw new Error("synthetic denied storage");
      },
    };
    expect(resolveInitialUiLocale(storage, "zh-CN")).toBe("zh-CN");
  });

  it("localizes stable meeting statuses without changing their values", async () => {
    await i18n.changeLanguage("zh-CN");
    expect(statusLabel("draft")).toBe("待准备");
    expect(statusLabel("recording")).toBe("会议进行中");
    expect(statusLabel("ready")).toBe("可继续处理");
  });

  it("localizes stable backend error codes", async () => {
    await i18n.changeLanguage("zh-CN");
    const error = uiError(new MeetingApiError("meeting_not_found", false));
    expect(error.code).toBe("meeting_not_found");
    expect(meetingErrorMessage(error.code)).toBe("此会议已不存在。");
  });

  it("uses a localized safe fallback for unknown errors", async () => {
    await i18n.changeLanguage("zh-CN");
    const error = uiError({ privateDetail: "do not expose" });
    expect(error.code).toBe("internal_error");
    expect(meetingErrorMessage(error.code)).toBe(
      "MeetingLens 无法完成此操作。",
    );
  });

  it("localizes stable transcript revision and picker errors", async () => {
    await i18n.changeLanguage("zh-CN");
    expect(transcriptErrorMessage("transcript_revision_conflict")).toBe(
      "此转录片段已发生变化，请刷新后重试。",
    );
    expect(transcriptErrorMessage("transcript_picker_unavailable")).toBe(
      "无法打开转录文件选择器。",
    );
  });

  it("formats dates centrally for the active locale", async () => {
    const value = "2026-07-30T01:02:00.000Z";
    await i18n.changeLanguage("en-US");
    const english = formatLocalDate(value);
    await i18n.changeLanguage("zh-CN");
    const chinese = formatLocalDate(value);
    expect(english).not.toBe(chinese);
    expect(formatLocalDate("invalid")).toBe("不可用");
  });

  it("formats file sizes centrally without locale-specific branches", async () => {
    await i18n.changeLanguage("en-US");
    expect(formatFileSize(1_536)).toBe("1.5 KiB");
    await i18n.changeLanguage("zh-CN");
    expect(formatFileSize(0)).toBe("0 B");
  });

  it("provides localized placeholders and accessibility labels", () => {
    expect(enUS.newMeeting.placeholder).toContain("Synthetic");
    expect(zhCN.newMeeting.placeholder).toContain("模拟");
    expect(enUS.settings.languageAria).toBe("Select interface language");
    expect(zhCN.settings.languageAria).toBe("选择界面语言");
  });

  it("detects every literal translation key used by the main UI", () => {
    const keys = [...uiSources.matchAll(/\bt\("([^"]+)"/g)].map(
      (match) => match[1],
    );
    const missing = keys.filter(
      (key) =>
        !i18n.exists(key, key === "document.fileCount" ? { count: 2 } : {}),
    );
    expect(missing).toEqual([]);
  });

  it("keeps the English and Chinese resource key sets identical", () => {
    expect(flattenKeys(zhCN)).toEqual(flattenKeys(enUS));
  });

  it("keeps major user copy out of production JSX", () => {
    for (const phrase of [
      ">New meeting<",
      ">Settings<",
      ">Add files<",
      ">Delete meeting<",
      'placeholder="Not saved',
      'aria-label="Primary navigation"',
    ]) {
      expect(uiSources).not.toContain(phrase);
    }
    expect(uiSources).not.toContain("dangerouslySetInnerHTML");
    expect(uiSources).not.toMatch(/locale\s*===\s*["']zh-CN["']/);
  });
});

function flattenKeys(value: object, prefix = ""): string[] {
  return Object.entries(value)
    .flatMap(([key, child]) => {
      const path = prefix.length === 0 ? key : `${prefix}.${key}`;
      return child !== null && typeof child === "object"
        ? flattenKeys(child as object, path)
        : [path];
    })
    .sort();
}
