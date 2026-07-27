import "./App.css";

type ModuleStatus = "Ready" | "Planned";

interface ProductModule {
  title: string;
  description: string;
  status: ModuleStatus;
  icon: string;
}

const modules: ProductModule[] = [
  {
    title: "Audio capture",
    description: "Visible, consent-driven recording with local storage.",
    status: "Planned",
    icon: "AC",
  },
  {
    title: "Document context",
    description: "Controlled uploads with type, size, and path validation.",
    status: "Planned",
    icon: "DC",
  },
  {
    title: "DeepSeek summary",
    description: "User-approved, minimal text sent for verifiable summaries.",
    status: "Planned",
    icon: "DS",
  },
  {
    title: "Live translation",
    description: "Source and translated segments kept side by side.",
    status: "Planned",
    icon: "LT",
  },
  {
    title: "Speaker diarization",
    description: "Anonymous speaker labels that remain user-correctable.",
    status: "Planned",
    icon: "SD",
  },
  {
    title: "Screen context",
    description: "Opt-in text capture with preview and redaction.",
    status: "Planned",
    icon: "SC",
  },
];

function App() {
  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="#overview" aria-label="MeetingLens home">
          <span className="brand-mark" aria-hidden="true">
            ML
          </span>
          <span>MeetingLens</span>
        </a>
        <span className="local-badge">
          <span className="status-dot" aria-hidden="true" />
          Local-first workspace
        </span>
      </header>

      <main id="overview">
        <section className="hero" aria-labelledby="hero-title">
          <div className="hero-copy">
            <p className="eyebrow">Project foundation · v0.1</p>
            <h1 id="hero-title">Turn meetings into evidence-backed action.</h1>
            <p className="hero-text">
              Capture only with consent, process locally by default, and keep
              every decision connected to its source.
            </p>
            <div className="hero-actions">
              <a className="primary-action" href="#modules">
                Review modules
              </a>
              <a className="secondary-action" href="#runtime">
                Runtime contract
              </a>
            </div>
          </div>

          <aside className="privacy-card" aria-label="Privacy boundary">
            <p className="card-label">Privacy boundary</p>
            <h2>Raw meeting data stays local.</h2>
            <ul>
              <li>Recording requires an explicit user action.</li>
              <li>Screen context is off by default.</li>
              <li>External AI receives only confirmed text.</li>
              <li>Credentials never enter the frontend or database.</li>
            </ul>
          </aside>
        </section>

        <section className="section" id="modules" aria-labelledby="modules-title">
          <div className="section-heading">
            <div>
              <p className="eyebrow">Capability map</p>
              <h2 id="modules-title">Independent modules, one meeting record.</h2>
            </div>
            <p>Each processor can fail, retry, or evolve without blocking the others.</p>
          </div>

          <div className="module-grid">
            {modules.map((module) => (
              <article className="module-card" key={module.title}>
                <div className="module-card-header">
                  <span className="module-icon" aria-hidden="true">
                    {module.icon}
                  </span>
                  <span className="module-status">{module.status}</span>
                </div>
                <h3>{module.title}</h3>
                <p>{module.description}</p>
              </article>
            ))}
          </div>
        </section>

        <section className="runtime-section" id="runtime" aria-labelledby="runtime-title">
          <div>
            <p className="eyebrow">Runtime foundation</p>
            <h2 id="runtime-title">A narrow contract between desktop and AI.</h2>
            <p>
              Tauri owns permissions and process lifecycle. The Python sidecar
              owns processing. SQLite keeps versioned, traceable state.
            </p>
          </div>
          <dl className="runtime-list">
            <div>
              <dt>Desktop shell</dt>
              <dd>Tauri 2 + React + TypeScript</dd>
            </div>
            <div>
              <dt>Health endpoint</dt>
              <dd>
                <code>GET /health</code>
              </dd>
            </div>
            <div>
              <dt>Default bind</dt>
              <dd>
                <code>127.0.0.1:8765</code>
              </dd>
            </div>
            <div>
              <dt>External AI</dt>
              <dd>Disabled until the user confirms a text scope</dd>
            </div>
          </dl>
        </section>
      </main>

      <footer>
        <p>MeetingLens does not record, upload, or summarize without a clear user action.</p>
      </footer>
    </div>
  );
}

export default App;
