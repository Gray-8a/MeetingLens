import { statusLabel } from "../lib/presentation";

export function StatusBadge({ status }: { status: string }) {
  const knownStatus = [
    "draft",
    "recording",
    "processing",
    "ready",
    "failed",
    "archived",
  ].includes(status);

  return (
    <span
      className={`status-badge status-badge--${knownStatus ? status : "unknown"}`}
      data-testid="status-badge"
    >
      <span className="status-badge__dot" aria-hidden="true" />
      {statusLabel(status)}
    </span>
  );
}
