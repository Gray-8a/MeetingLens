import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import enUS from "./locales/en-US.json";
import zhCN from "./locales/zh-CN.json";

export const SUPPORTED_UI_LOCALES = ["zh-CN", "en-US"] as const;
export type UiLocale = (typeof SUPPORTED_UI_LOCALES)[number];
export const UI_LOCALE_STORAGE_KEY = "meetinglens.uiLocale";
export const FALLBACK_UI_LOCALE: UiLocale = "en-US";

export function isUiLocale(value: unknown): value is UiLocale {
  return (
    typeof value === "string" &&
    SUPPORTED_UI_LOCALES.includes(value as UiLocale)
  );
}

export function localeFromSystem(systemLocale: string | undefined): UiLocale {
  return systemLocale?.toLowerCase().startsWith("zh")
    ? "zh-CN"
    : FALLBACK_UI_LOCALE;
}

export function resolveInitialUiLocale(
  storage: Pick<Storage, "getItem"> | undefined = safeStorage(),
  systemLocale: string | undefined = safeSystemLocale(),
): UiLocale {
  try {
    const saved = storage?.getItem(UI_LOCALE_STORAGE_KEY);
    if (isUiLocale(saved)) return saved;
  } catch {
    // A denied or damaged WebView storage must not prevent startup.
  }
  return localeFromSystem(systemLocale);
}

function safeStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

function safeSystemLocale(): string | undefined {
  try {
    return globalThis.navigator?.language;
  } catch {
    return undefined;
  }
}

function updateDocumentLanguage(locale: UiLocale): void {
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale;
  }
}

const initialLocale = resolveInitialUiLocale();

void i18n.use(initReactI18next).init({
  resources: {
    "en-US": { translation: enUS },
    "zh-CN": { translation: zhCN },
  },
  lng: initialLocale,
  fallbackLng: FALLBACK_UI_LOCALE,
  supportedLngs: [...SUPPORTED_UI_LOCALES],
  load: "currentOnly",
  initAsync: false,
  interpolation: { escapeValue: false },
  returnNull: false,
});

updateDocumentLanguage(initialLocale);

i18n.on("languageChanged", (language) => {
  updateDocumentLanguage(isUiLocale(language) ? language : FALLBACK_UI_LOCALE);
});

export function currentUiLocale(): UiLocale {
  return isUiLocale(i18n.resolvedLanguage)
    ? i18n.resolvedLanguage
    : FALLBACK_UI_LOCALE;
}

export async function changeUiLocale(
  locale: UiLocale,
  storage: Pick<Storage, "setItem"> | undefined = safeStorage(),
): Promise<void> {
  if (!isUiLocale(locale)) return;

  try {
    storage?.setItem(UI_LOCALE_STORAGE_KEY, locale);
  } catch {
    // Language switching still works for this session when persistence fails.
  }

  await i18n.changeLanguage(locale);
}

export default i18n;
