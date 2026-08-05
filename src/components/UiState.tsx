import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

export function LoadingState({ label }: { label?: string }) {
  const { t } = useTranslation();
  return (
    <div className="ui-state" role="status">
      <span className="spinner" aria-hidden="true" />
      <p>{label ?? t("common.loading")}</p>
    </div>
  );
}

export function EmptyState({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="ui-state ui-state--empty">
      <span className="empty-mark" aria-hidden="true">
        ML
      </span>
      <h2>{title}</h2>
      <div>{children}</div>
    </div>
  );
}

export function ErrorNotice({
  code,
  message,
}: {
  code: string;
  message: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="notice notice--error" role="alert">
      <strong>{message}</strong>
      <span>{t("common.errorCode", { code })}</span>
    </div>
  );
}

export function InfoNotice({ children }: { children: ReactNode }) {
  return (
    <div className="notice notice--info" role="status">
      {children}
    </div>
  );
}
