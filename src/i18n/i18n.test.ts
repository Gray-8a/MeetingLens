import appSource from "../App.tsx?raw";
import documentSource from "../components/DocumentPanel.tsx?raw";
import deepSeekConsentSource from "../components/DeepSeekConsentDialog.tsx?raw";
import summaryResultSource from "../components/SummaryResultView.tsx?raw";
import speakerRenameSource from "../components/SpeakerRenamePanel.tsx?raw";
import transcriptInputSource from "../components/TranscriptInputPanel.tsx?raw";
import transcriptSegmentSource from "../components/TranscriptSegmentPanel.tsx?raw";
import transcriptSourceListSource from "../components/TranscriptSourceList.tsx?raw";
import helpSource from "../pages/HelpPage.tsx?raw";
import historySource from "../pages/HistoryPage.tsx?raw";
import homeSource from "../pages/HomePage.tsx?raw";
import liveSource from "../pages/LiveMeetingPage.tsx?raw";
import detailSource from "../pages/MeetingDetailPage.tsx?raw";
import newSource from "../pages/NewMeetingPage.tsx?raw";
import settingsSource from "../pages/SettingsPage.tsx?raw";
import summaryInputSource from "../pages/SummaryModeInputPage.tsx?raw";
import summaryMinutesSource from "../pages/SummaryMinutesPage.tsx?raw";
import transcriptWorkspaceSource from "../pages/TranscriptWorkspacePage.tsx?raw";
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
  deepSeekConsentSource,
  summaryResultSource,
  speakerRenameSource,
  transcriptInputSource,
  transcriptSegmentSource,
  transcriptSourceListSource,
  helpSource,
  historySource,
  homeSource,
  liveSource,
  detailSource,
  newSource,
  settingsSource,
  summaryInputSource,
  summaryMinutesSource,
  transcriptWorkspaceSource,
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

  it("localizes the three modes without claiming unfinished modes are available", async () => {
    await i18n.changeLanguage("en-US");
    expect(i18n.t("modes.summaryAvailable")).toBe("Available now");
    expect(i18n.t("modes.translationTitle")).toBe("Translation mode");
    expect(i18n.t("modes.meetingTitle")).toBe("Meeting mode");
    expect(i18n.t("modes.comingSoon")).toBe("Coming soon");

    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("modes.summaryAvailable")).toBe("现在可用");
    expect(i18n.t("modes.comingSoon")).toBe("即将推出");
  });

  it("localizes every supported and reserved transcript source type", async () => {
    const sourceTypeKeys = [
      "transcriptWorkspace.sourceType.pasted_text",
      "transcriptWorkspace.sourceType.imported_txt",
      "transcriptWorkspace.sourceType.imported_srt",
      "transcriptWorkspace.sourceType.imported_vtt",
      "transcriptWorkspace.sourceType.live_asr",
      "transcriptWorkspace.sourceType.platform_export",
    ] as const;

    for (const locale of ["en-US", "zh-CN"] as const) {
      await i18n.changeLanguage(locale);
      for (const key of sourceTypeKeys) {
        expect(i18n.exists(key)).toBe(true);
        expect(i18n.t(key)).not.toBe(key);
      }
    }
  });

  it("keeps transcript readiness and correction messages localized", async () => {
    for (const locale of ["en-US", "zh-CN"] as const) {
      await i18n.changeLanguage(locale);
      for (const state of ["empty", "usable", "partial", "ready"]) {
        expect(i18n.exists(`transcriptWorkspace.readiness.${state}`)).toBe(
          true,
        );
      }
      expect(i18n.t("transcriptWorkspace.revisionConflict")).not.toContain(
        "transcript_revision_conflict",
      );
      expect(i18n.t("transcriptWorkspace.summaryComingSoon")).not.toContain(
        "DeepSeek",
      );
    }
  });

  it("describes the available minutes loop and keeps deferred capabilities scoped", async () => {
    await i18n.changeLanguage("en-US");
    expect(i18n.t("help.availableBody")).toContain("TXT, SRT and VTT");
    expect(i18n.t("help.availableBody")).toContain("canonical Markdown");
    expect(i18n.t("help.unavailableBody")).toContain("realtime transcription");
    expect(i18n.t("help.deepseekBody")).toContain(
      "Rust-only DeepSeek provider",
    );
    expect(i18n.t("help.deepseekBody")).toContain("never receives the key");

    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("help.availableBody")).toContain("TXT、SRT、VTT");
    expect(i18n.t("help.availableBody")).toContain("规范 Markdown");
    expect(i18n.t("help.unavailableBody")).toContain("实时转录");
    expect(i18n.t("help.deepseekBody")).toContain(
      "Rust-only DeepSeek Provider",
    );
    expect(i18n.t("help.deepseekBody")).toContain("永远不会收到 Key");
  });

  it("keeps the DeepSeek disclosure meaning aligned in both locales", async () => {
    await i18n.changeLanguage("en-US");
    expect(i18n.t("summaryConsent.title")).toContain("DeepSeek");
    expect(i18n.t("summaryConsent.warning")).toContain("cannot recall");
    expect(i18n.t("settings.deepseek.revokeHelp")).toContain("future sends");

    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("summaryConsent.title")).toContain("DeepSeek");
    expect(i18n.t("summaryConsent.warning")).toContain("无法召回");
    expect(i18n.t("settings.deepseek.revokeHelp")).toContain("未来发送");
  });

  it("detects every literal translation key used by the main UI", () => {
    const keys = [...uiSources.matchAll(/\bt\("([^"]+)"/g)].map(
      (match) => match[1],
    );
    const missing = keys.filter((key) => !i18n.exists(key, { count: 2 }));
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

  it("keeps transcript UI out of browser persistence, console and direct Tauri invoke", () => {
    const transcriptUiSources = [
      speakerRenameSource,
      transcriptInputSource,
      transcriptSegmentSource,
      transcriptSourceListSource,
      summaryInputSource,
      transcriptWorkspaceSource,
    ].join("\n");

    expect(transcriptUiSources).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\./,
    );
    expect(transcriptUiSources).not.toContain("invoke(");
    expect(transcriptUiSources).not.toContain("dangerouslySetInnerHTML");
  });

  it("keeps the consent dialog out of storage, logs and direct transports", () => {
    expect(deepSeekConsentSource).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\.|\binvoke\s*\(|\bfetch\s*\(|XMLHttpRequest|WebSocket/,
    );
    expect(deepSeekConsentSource).not.toContain("dangerouslySetInnerHTML");
  });

  it("keeps the Minutes UI out of storage, logs, HTML injection and direct transports", () => {
    const minutesSources = `${summaryMinutesSource}\n${summaryResultSource}`;
    expect(minutesSources).not.toMatch(
      /localStorage|sessionStorage|indexedDB|console\.|dangerouslySetInnerHTML|\binvoke\s*\(|\bfetch\s*\(|XMLHttpRequest|WebSocket/,
    );
    expect(minutesSources).not.toContain("clipboard.read");
    expect(minutesSources).not.toContain("execCommand");
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
