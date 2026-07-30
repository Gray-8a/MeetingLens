import type { ReactNode } from "react";

export function LoadingState({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="ui-state" role="status">
      <span className="spinner" aria-hidden="true" />
      <p>{label}</p>
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
  return (
    <div className="notice notice--error" role="alert">
      <strong>{message}</strong>
      <span>Error code: {code}</span>
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
