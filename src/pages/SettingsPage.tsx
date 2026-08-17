import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { useTranslation } from "react-i18next";
import {
  SecretApiError,
  toSecretError,
  type SecretErrorCode,
} from "../api/secret";
import {
  toSummaryError,
  type DeepSeekProviderStatusResponse,
  type SummaryErrorCode,
} from "../api/summary";
import { AppLink } from "../app/navigation";
import { useServices } from "../app/services";
import { APP_VERSION } from "../app/version";
import { changeUiLocale, currentUiLocale, type UiLocale } from "../i18n";
import { summaryErrorMessage, secretErrorMessage } from "../i18n/errors";
import { formatLocalDate } from "../i18n/formatters";
import { safeProviderStatus, type ProviderKind } from "../providers/types";
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

type SecretFeedback = "saved" | "deleted" | { error: SecretErrorCode } | null;
type ConsentFeedback = "revoked" | { error: SummaryErrorCode } | null;

export function SettingsPage({
  databaseStatus,
  localServiceStatus,
}: {
  databaseStatus: DatabaseStatus;
  localServiceStatus: LocalServiceStatus;
}) {
  const { t } = useTranslation();
  const { providers, secretApi, summaryApi } = useServices();
  const [providerSnapshots, setProviderSnapshots] = useState(() =>
    providers.snapshots(),
  );
  const secretInput = useRef<HTMLInputElement>(null);
  const [secretStatus, setSecretStatus] = useState<SecretUiStatus>("checking");
  const [hasSecretInput, setHasSecretInput] = useState(false);
  const [secretAction, setSecretAction] = useState<"save" | "delete" | null>(
    null,
  );
  const [secretFeedback, setSecretFeedback] = useState<SecretFeedback>(null);
  const [confirmSecretDelete, setConfirmSecretDelete] = useState(false);
  const consentActionRef = useRef(false);
  const [deepSeekStatus, setDeepSeekStatus] =
    useState<DeepSeekProviderStatusResponse | null>(null);
  const [deepSeekStatusError, setDeepSeekStatusError] =
    useState<SummaryErrorCode | null>(null);
  const [consentAction, setConsentAction] = useState(false);
  const [consentFeedback, setConsentFeedback] = useState<ConsentFeedback>(null);

  async function refreshDeepSeekStatus() {
    try {
      const status = await summaryApi.getDeepSeekProviderStatus();
      setDeepSeekStatus(status);
      setDeepSeekStatusError(null);
    } catch (reason: unknown) {
      setDeepSeekStatus(null);
      setDeepSeekStatusError(toSummaryError(reason).code);
    }
  }

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

  useEffect(() => {
    let active = true;
    void summaryApi
      .getDeepSeekProviderStatus()
      .then((status) => {
        if (!active) return;
        setDeepSeekStatus(status);
        setDeepSeekStatusError(null);
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setDeepSeekStatus(null);
        setDeepSeekStatusError(toSummaryError(reason).code);
      });
    return () => {
      active = false;
    };
  }, [summaryApi]);

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
      setSecretFeedback({ error: "secret_invalid_input" });
      return;
    }

    setSecretAction("save");
    setSecretFeedback(null);
    try {
      const status = await secretApi.set("deepseek_api_key", value);
      if (secretInput.current !== null) secretInput.current.value = "";
      setHasSecretInput(false);
      setSecretStatus(status.configured ? "configured" : "unconfigured");
      setSecretFeedback("saved");
      void refreshDeepSeekStatus();
    } catch (reason: unknown) {
      const error = toSecretError(reason);
      setSecretStatus(secretErrorStatus(error));
      setSecretFeedback({ error: error.code });
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
      setSecretFeedback("deleted");
      void refreshDeepSeekStatus();
    } catch (reason: unknown) {
      const error = toSecretError(reason);
      setSecretStatus(secretErrorStatus(error));
      setSecretFeedback({ error: error.code });
    } finally {
      setConfirmSecretDelete(false);
      setSecretAction(null);
    }
  }

  async function revokeDeepSeekConsent() {
    if (consentActionRef.current) return;
    consentActionRef.current = true;
    setConsentAction(true);
    setConsentFeedback(null);
    try {
      const status = await summaryApi.revokeDeepSeekConsent();
      setDeepSeekStatus(status);
      setDeepSeekStatusError(null);
      setConsentFeedback("revoked");
    } catch (reason: unknown) {
      const error = toSummaryError(reason);
      setConsentFeedback({ error: error.code });
    } finally {
      consentActionRef.current = false;
      setConsentAction(false);
    }
  }

  function feedbackMessage(): string {
    if (secretFeedback === "saved") return t("settings.deepseek.saved");
    if (secretFeedback === "deleted") return t("settings.deepseek.deleted");
    if (secretFeedback !== null) {
      return secretErrorMessage(secretFeedback.error);
    }
    return t("settings.deepseek.defaultHelp");
  }

  return (
    <section className="content-panel" aria-labelledby="settings-title">
      <div className="section-heading section-heading--stacked">
        <p className="eyebrow">{t("settings.eyebrow")}</p>
        <h1 id="settings-title">{t("settings.title")}</h1>
        <p>{t("settings.description")}</p>
      </div>

      <div className="settings-grid">
        <article>
          <span className="settings-icon" aria-hidden="true">
            ML
          </span>
          <h2>{t("settings.application")}</h2>
          <dl className="detail-list">
            <div>
              <dt>{t("settings.version")}</dt>
              <dd>{APP_VERSION}</dd>
            </div>
            <div>
              <dt>{t("settings.dataLocation")}</dt>
              <dd>{t("settings.dataLocationValue")}</dd>
            </div>
          </dl>
        </article>

        <article data-acceptance="language-settings">
          <span className="settings-icon" aria-hidden="true">
            LG
          </span>
          <h2>{t("settings.interfaceLanguage")}</h2>
          <p className="field-help">{t("settings.languageDescription")}</p>
          <select
            aria-label={t("settings.languageAria")}
            data-acceptance="ui-language"
            onChange={(event) =>
              void changeUiLocale(event.currentTarget.value as UiLocale)
            }
            value={currentUiLocale()}
          >
            <option value="zh-CN">{t("settings.languageZh")}</option>
            <option value="en-US">{t("settings.languageEn")}</option>
          </select>
          <p>
            <AppLink className="text-link" to="/help">
              {t("settings.openHelp")}
            </AppLink>
          </p>
        </article>

        <article data-acceptance="sidecar-security-status">
          <span className="settings-icon" aria-hidden="true">
            RT
          </span>
          <h2>{t("settings.localRuntime")}</h2>
          <dl className="detail-list">
            <div>
              <dt>{t("settings.service")}</dt>
              <dd>{t(`settings.serviceStatus.${localServiceStatus}`)}</dd>
            </div>
            <div>
              <dt>{t("settings.database")}</dt>
              <dd>{t(`settings.databaseStatus.${databaseStatus}`)}</dd>
            </div>
            <div>
              <dt>{t("settings.binding")}</dt>
              <dd>{t("settings.bindingValue")}</dd>
            </div>
            <div>
              <dt>{t("settings.session")}</dt>
              <dd>{t("settings.sessionValue")}</dd>
            </div>
            <div>
              <dt>{t("settings.sidecarCredentialAccess")}</dt>
              <dd>{t("settings.disabled")}</dd>
            </div>
          </dl>
        </article>

        <article data-acceptance="provider-statuses">
          <span className="settings-icon" aria-hidden="true">
            PR
          </span>
          <h2>{t("settings.providerRuntime")}</h2>
          <ul className="settings-list">
            {providerSnapshots.map((provider) => (
              <li key={provider.kind}>
                <strong>{t(`settings.providerKind.${provider.kind}`)}</strong>
                <span>
                  {t("settings.providerModeMock")} ·{" "}
                  {t(
                    `settings.providerStatus.${safeProviderStatus(provider.status)}`,
                  )}{" "}
                  · {t("settings.providerNotEnabled")}
                </span>
              </li>
            ))}
          </ul>
          <p className="field-help">{t("settings.providerHelp")}</p>
        </article>

        <article data-acceptance="deepseek-secret-settings">
          <span className="settings-icon" aria-hidden="true">
            DS
          </span>
          <h2>{t("settings.deepseek.title")}</h2>
          <p className="secret-status" data-status={secretStatus}>
            <strong>{t("settings.deepseek.configuration")}</strong>{" "}
            {t(`settings.deepseek.status.${secretStatus}`)}
          </p>
          <form className="secret-form" onSubmit={saveSecret}>
            <label htmlFor="deepseek-api-key">
              {t("settings.deepseek.apiKey")}
            </label>
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
                {secretAction === "save"
                  ? t("common.saving")
                  : t("settings.deepseek.save")}
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
                {t("settings.deepseek.delete")}
              </button>
            </div>
          </form>
          {confirmSecretDelete ? (
            <div
              className="confirm-panel"
              data-acceptance="secret-delete-confirmation"
            >
              <p>{t("settings.deepseek.deleteQuestion")}</p>
              <div className="form-actions">
                <button
                  className="button button--danger"
                  data-acceptance="confirm-secret-delete"
                  disabled={secretAction !== null}
                  onClick={() => void deleteSecret()}
                  type="button"
                >
                  {secretAction === "delete"
                    ? t("common.deleting")
                    : t("common.confirmDelete")}
                </button>
                <button
                  className="button button--secondary"
                  disabled={secretAction !== null}
                  onClick={() => setConfirmSecretDelete(false)}
                  type="button"
                >
                  {t("common.cancel")}
                </button>
              </div>
            </div>
          ) : null}
          <p aria-live="polite" className="field-help" role="status">
            {feedbackMessage()}
          </p>
          <hr className="settings-divider" />
          <h3>{t("settings.deepseek.externalSharing")}</h3>
          {deepSeekStatus === null ? (
            <p
              aria-live="polite"
              className={
                deepSeekStatusError === null ? "field-help" : "field-error"
              }
              role={deepSeekStatusError === null ? "status" : "alert"}
            >
              {deepSeekStatusError === null
                ? t("settings.deepseek.consentChecking")
                : summaryErrorMessage(deepSeekStatusError)}
            </p>
          ) : (
            <>
              <dl className="detail-list deepseek-status-list">
                <div>
                  <dt>{t("settings.deepseek.provider")}</dt>
                  <dd>{deepSeekStatus.provider}</dd>
                </div>
                <div>
                  <dt>{t("settings.deepseek.host")}</dt>
                  <dd>{deepSeekStatus.endpointHost}</dd>
                </div>
                <div>
                  <dt>{t("settings.deepseek.model")}</dt>
                  <dd>{deepSeekStatus.model}</dd>
                </div>
                <div>
                  <dt>{t("settings.deepseek.rememberedConsent")}</dt>
                  <dd>
                    {deepSeekStatus.consentRemembered
                      ? t("settings.deepseek.consentRemembered")
                      : t("settings.deepseek.consentNotRemembered")}
                  </dd>
                </div>
                <div>
                  <dt>{t("settings.deepseek.consentedAt")}</dt>
                  <dd>{formatLocalDate(deepSeekStatus.consentedAt)}</dd>
                </div>
              </dl>
              <button
                className="button button--danger-outline"
                data-acceptance="revoke-deepseek-consent"
                disabled={consentAction}
                onClick={() => void revokeDeepSeekConsent()}
                type="button"
              >
                {consentAction
                  ? t("settings.deepseek.revokingConsent")
                  : t("settings.deepseek.revokeConsent")}
              </button>
              <p className="field-help">{t("settings.deepseek.revokeHelp")}</p>
            </>
          )}
          {consentFeedback === null ? null : (
            <p
              aria-live="polite"
              className={
                consentFeedback === "revoked" ? "field-help" : "field-error"
              }
              role={consentFeedback === "revoked" ? "status" : "alert"}
            >
              {consentFeedback === "revoked"
                ? t("settings.deepseek.consentRevoked")
                : summaryErrorMessage(consentFeedback.error)}
            </p>
          )}
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
