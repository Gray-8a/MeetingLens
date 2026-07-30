import { useEffect, useState } from "react";
import { useServices } from "../app/services";
import { providerStatusLabel, type ProviderKind } from "../providers/types";
import type { DatabaseStatus } from "./HomePage";

export type LocalServiceStatus =
  | "checking"
  | "authenticated"
  | "unavailable"
  | "error"
  | "desktop-only";

const serviceLabels: Record<LocalServiceStatus, string> = {
  checking: "Checking local service…",
  authenticated: "Local Sidecar authenticated",
  unavailable: "Local service unavailable",
  error: "Local Sidecar error",
  "desktop-only": "Available in the desktop runtime",
};

const databaseLabels: Record<DatabaseStatus, string> = {
  checking: "Checking local database…",
  ready: "Local database ready",
  unavailable: "Local database unavailable",
};

export function SettingsPage({
  databaseStatus,
  localServiceStatus,
}: {
  databaseStatus: DatabaseStatus;
  localServiceStatus: LocalServiceStatus;
}) {
  const { providers } = useServices();
  const [providerSnapshots, setProviderSnapshots] = useState(() =>
    providers.snapshots(),
  );

  useEffect(() => {
    const refresh = () => setProviderSnapshots(providers.snapshots());
    const unsubscribers = providerKinds.map((kind) =>
      providers.get(kind).subscribeStatus(refresh),
    );
    return () => unsubscribers.forEach((unsubscribe) => unsubscribe());
  }, [providers]);

  return (
    <section className="content-panel" aria-labelledby="settings-title">
      <div className="section-heading section-heading--stacked">
        <p className="eyebrow">Read-only configuration</p>
        <h1 id="settings-title">Settings</h1>
        <p>Later milestones will add explicit provider and privacy controls.</p>
      </div>

      <div className="settings-grid">
        <article>
          <span className="settings-icon" aria-hidden="true">
            ML
          </span>
          <h2>Application</h2>
          <dl className="detail-list">
            <div>
              <dt>Version</dt>
              <dd>0.1.0</dd>
            </div>
            <div>
              <dt>Data location</dt>
              <dd>Private MeetingLens application storage on this device</dd>
            </div>
          </dl>
        </article>
        <article data-acceptance="sidecar-security-status">
          <span className="settings-icon" aria-hidden="true">
            RT
          </span>
          <h2>Local runtime</h2>
          <dl className="detail-list">
            <div>
              <dt>Service</dt>
              <dd>{serviceLabels[localServiceStatus]}</dd>
            </div>
            <div>
              <dt>Database</dt>
              <dd>{databaseLabels[databaseStatus]}</dd>
            </div>
            <div>
              <dt>Binding</dt>
              <dd>This device only</dd>
            </div>
            <div>
              <dt>Session</dt>
              <dd>Current application session</dd>
            </div>
            <div>
              <dt>Secret storage</dt>
              <dd>Not implemented</dd>
            </div>
          </dl>
        </article>
        <article data-acceptance="provider-statuses">
          <span className="settings-icon" aria-hidden="true">
            PR
          </span>
          <h2>Provider runtime</h2>
          <ul className="settings-list">
            {providerSnapshots.map((provider) => (
              <li key={provider.kind}>
                <strong>{providerLabel(provider.kind)}</strong>
                <span>
                  Mock · {providerStatusLabel(provider.status)} · Not enabled
                </span>
              </li>
            ))}
          </ul>
          <p className="field-help">
            Mock providers use synthetic memory-only data. No device, file, or
            external AI access is enabled.
          </p>
        </article>
      </div>
    </section>
  );
}

const providerKinds: readonly ProviderKind[] = [
  "capture",
  "transcription",
  "translation",
  "summary",
  "document-parser",
];

function providerLabel(kind: ProviderKind): string {
  const labels: Record<ProviderKind, string> = {
    capture: "Capture",
    transcription: "Transcription",
    translation: "Translation",
    summary: "Summary",
    "document-parser": "Document parser",
  };
  return labels[kind];
}
