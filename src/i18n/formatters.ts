import i18n, { currentUiLocale } from ".";

export function formatLocalDate(value: string | null): string {
  if (value === null) return i18n.t("common.notSet");
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return i18n.t("common.unavailable");
  return new Intl.DateTimeFormat(currentUiLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds]
    .map((part) => part.toString().padStart(2, "0"))
    .join(":");
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return `0 ${i18n.t("document.unit.byte")}`;
  }

  const units = [
    i18n.t("document.unit.byte"),
    i18n.t("document.unit.kibibyte"),
    i18n.t("document.unit.mebibyte"),
    i18n.t("document.unit.gibibyte"),
  ];
  const unitIndex = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** unitIndex;
  return `${new Intl.NumberFormat(currentUiLocale(), {
    maximumFractionDigits: unitIndex === 0 ? 0 : 1,
  }).format(value)} ${units[unitIndex]}`;
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat(currentUiLocale()).format(value);
}

export function formatPercent(ratio: number): string {
  return new Intl.NumberFormat(currentUiLocale(), {
    style: "percent",
    maximumFractionDigits: 0,
  }).format(ratio);
}

export function formatCompletedDate(value: string | null): string {
  if (value === null) return i18n.t("common.notCompleted");
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return i18n.t("common.unknownTime");
  return new Intl.DateTimeFormat(currentUiLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
