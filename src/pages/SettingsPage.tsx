import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { SecretApiError, toSecretError } from "../api/secret";
import { useServices } from "../app/services";
import { providerStatusLabel, type ProviderKind } from "../providers/types";
import type { DatabaseStatus } from "./HomePage";

export type LocalServiceStatus =
  | "checking"
  | "authenticated"
  | "unavailable"
  | "error"
  | "desktop-only";

type SecretUiStatus =
  | "checking"
  | "unconfigured"
  | "configured"
  | "unavailable"
  | "error";

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

const secretLabels: Record<SecretUiStatus, string> = {
  checking: "Checking secure storage…",
  unconfigured: "Not configured",
  configured: "Configured",
  unavailable: "SecretStore unavailable",
  error: "Secure storage error",
};

export function SettingsPage({
  databaseStatus,
  localServiceStatus,
}: {
  databaseStatus: DatabaseStatus;
  localServiceStatus: LocalServiceStatus;
}) {
  const { providers, secretApi } = useServices();
  const [providerSnapshots, setProviderSnapshots] = useState(() =>
    providers.snapshots(),
  );
  const secretInput = useRef<HTMLInputElement>(null);
  const [secretStatus, setSecretStatus] = useState<SecretUiStatus>("checking");
  const [hasSecretInput, setHasSecretInput] = useState(false);
  const [secretAction, setSecretAction] = useState<"save" | "delete" | null>(
    null,
  );
  const [secretFeedback, setSecretFeedback] = useState<string | null>(null);
  const [confirmSecretDelete, setConfirmSecretDelete] = useState(false);

  useEffect(() => {
    const refresh = () => setProviderSnapshots(providers.snapshots());
    const unsubscribers = providerKinds.map((kind) =>
      providers.get(kind).subscribeStatus(refresh),
    );
    return () => unsubscribers.forEach((unsubscribe) => unsubscribe());
  }, [providers]);

  useEffect(() => {
    let active = true;
    void secretApi
      .getStatus("deepseek_api_key")
      .then((status) => {
        if (!active) return;
        setSecretStatus(
          !status.available
            ? "unavailable"
            : status.configured
              ? "configured"
              : "unconfigured",
        );
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setSecretStatus(secretErrorStatus(toSecretError(reason)));
      });
    return () => {
      active = false;
    };
  }, [secretApi]);

  useLayoutEffect(
    () => () => {
      if (secretInput.current !== null) secretInput.current.value = "";
    },
    [],
  );

  async function saveSecret(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (secretAction !== null) return;
    const value = secretInput.current?.value ?? "";
    if (trimAscii(value).length === 0) {
      setSecretFeedback("Enter a non-empty API key.");
      return;
    }

    setSecretAction("save");
    setSecretFeedback(null);
    try {
      const status = await secretApi.set("deepseek_api_key", value);
      if (secretInput.current !== null) secretInput.current.value = "";
      setHasSecretInput(false);
      setSecretStatus(status.configured ? "configured" : "unconfigured");
      setSecretFeedback("API key saved to secure local storage.");
    } catch (reason: unknown) {
      const error = toSecretError(reason);
      setSecretStatus(secretErrorStatus(error));
      setSecretFeedback(error.message);
    } finally {
      setSecretAction(null);
    }
  }

  async function deleteSecret() {
    if (secretAction !== null) return;
    setSecretAction("delete");
    setSecretFeedback(null);
    try {
      await secretApi.delete("deepseek_api_key");
      if (secretInput.current !== null) secretInput.current.value = "";
      setHasSecretInput(false);
      setSecretStatus("unconfigured");
      setSecretFeedback("Stored API key deleted.");
    } catch (reason: unknown) {
      const error = toSecretError(reason);
      setSecretStatus(secretErrorStatus(error));
      setSecretFeedback(error.message);
    } finally {
      setConfirmSecretDelete(false);
      setSecretAction(null);
    }
  }

  return (
    <section className="content-panel" aria-labelledby="settings-title">
      <div className="section-heading section-heading--stacked">
        <p className="eyebrow">Local configuration</p>
        <h1 id="settings-title">Settings</h1>
        <p>Manage local runtime status and approved secure settings.</p>
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
              <dt>Sidecar credential access</dt>
              <dd>Disabled</dd>
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
        <article data-acceptance="deepseek-secret-settings">
          <span className="settings-icon" aria-hidden="true">
            DS
          </span>
          <h2>DeepSeek API</h2>
          <p className="secret-status" data-status={secretStatus}>
            <strong>Configuration:</strong> {secretLabels[secretStatus]}
          </p>
          <form className="secret-form" onSubmit={saveSecret}>
            <label htmlFor="deepseek-api-key">API key</label>
            <input
              ref={secretInput}
              autoCapitalize="none"
              autoComplete="off"
              data-acceptance="deepseek-secret-input"
              disabled={secretAction !== null || secretStatus === "unavailable"}
              id="deepseek-api-key"
              name="deepseek-api-key"
              onInput={(event) =>
                setHasSecretInput(event.currentTarget.value.length > 0)
              }
              spellCheck={false}
              type="password"
            />
            <div className="form-actions">
              <button
                className="button button--primary"
                data-acceptance="save-deepseek-secret"
                disabled={
                  !hasSecretInput ||
                  secretAction !== null ||
                  secretStatus === "unavailable"
                }
                type="submit"
              >
                {secretAction === "save" ? "Saving…" : "Save API key"}
              </button>
              <button
                className="button button--danger-outline"
                data-acceptance="delete-deepseek-secret"
                disabled={
                  secretStatus !== "configured" || secretAction !== null
                }
                onClick={() => setConfirmSecretDelete(true)}
                type="button"
              >
                Delete stored key
              </button>
            </div>
          </form>
          {confirmSecretDelete ? (
            <div
              className="confirm-panel"
              data-acceptance="secret-delete-confirmation"
            >
              <p>Delete the stored API key from this Windows account?</p>
              <div className="form-actions">
                <button
                  className="button button--danger"
                  data-acceptance="confirm-secret-delete"
                  disabled={secretAction !== null}
                  onClick={() => void deleteSecret()}
                  type="button"
                >
                  {secretAction === "delete" ? "Deleting…" : "Confirm delete"}
                </button>
                <button
                  className="button button--secondary"
                  disabled={secretAction !== null}
                  onClick={() => setConfirmSecretDelete(false)}
                  type="button"
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : null}
          <p aria-live="polite" className="field-help" role="status">
            {secretFeedback ??
              "Saved locally in Windows Credential Manager. AI features are not enabled yet."}
          </p>
        </article>
      </div>
    </section>
  );
}

function secretErrorStatus(error: SecretApiError): SecretUiStatus {
  return error.code === "secret_store_unavailable" ||
    error.code === "secret_unsupported_platform"
    ? "unavailable"
    : "error";
}

function trimAscii(value: string): string {
  return value.replace(/^[\t\n\f\r ]+|[\t\n\f\r ]+$/g, "");
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
