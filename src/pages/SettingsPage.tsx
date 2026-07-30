import type { DatabaseStatus } from "./HomePage";

export type LocalServiceStatus =
  | "checking"
  | "healthy"
  | "unavailable"
  | "desktop-only";

const serviceLabels: Record<LocalServiceStatus, string> = {
  checking: "Checking local service…",
  healthy: "Local service healthy",
  unavailable: "Local service unavailable",
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
        <article>
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
          </dl>
        </article>
        <article>
          <span className="settings-icon" aria-hidden="true">
            AI
          </span>
          <h2>Planned capabilities</h2>
          <ul className="settings-list">
            <li>
              <strong>DeepSeek</strong>
              <span>Not configured</span>
            </li>
            <li>
              <strong>Audio</strong>
              <span>Not enabled</span>
            </li>
            <li>
              <strong>Document parsing</strong>
              <span>Not enabled</span>
            </li>
          </ul>
        </article>
      </div>
    </section>
  );
}
