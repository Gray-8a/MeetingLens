import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
} from "react";
import { useTranslation } from "react-i18next";
import type {
  SummaryErrorCode,
  SummaryExternalDataCategory,
  SummaryExternalDisclosureResponse,
} from "../api/summary";
import { formatFileSize, formatNumber } from "../i18n/formatters";
import { summaryErrorMessage } from "../i18n/errors";

export interface DeepSeekConsentDialogProps {
  disclosure: SummaryExternalDisclosureResponse;
  errorCode?: SummaryErrorCode | null;
  onCancel: () => void;
  onConfirm: (remember: boolean) => Promise<void>;
}

export function DeepSeekConsentDialog({
  disclosure,
  errorCode = null,
  onCancel,
  onConfirm,
}: DeepSeekConsentDialogProps) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const submittingRef = useRef(false);
  const onCancelRef = useRef(onCancel);
  const [remember, setRemember] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    const previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    headingRef.current?.focus();

    const onDocumentKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && !submittingRef.current) {
        event.preventDefault();
        onCancelRef.current();
      } else if (
        event.key === "Tab" &&
        dialogRef.current !== null &&
        !dialogRef.current.contains(document.activeElement)
      ) {
        event.preventDefault();
        headingRef.current?.focus();
      }
    };
    document.addEventListener("keydown", onDocumentKeyDown);
    return () => {
      document.removeEventListener("keydown", onDocumentKeyDown);
      previousFocus?.focus();
    };
  }, []);

  async function confirm() {
    if (submittingRef.current) return;
    submittingRef.current = true;
    headingRef.current?.focus();
    setSubmitting(true);
    try {
      await onConfirm(remember);
    } catch {
      submittingRef.current = false;
      setSubmitting(false);
    }
  }

  function cancel() {
    if (!submittingRef.current) onCancel();
  }

  function closeFromBackdrop(event: MouseEvent<HTMLDivElement>) {
    if (event.target === event.currentTarget) cancel();
  }

  function trapFocus(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Tab" || dialogRef.current === null) return;
    const focusableSelector =
      'button:not(:disabled), input:not(:disabled), [tabindex]:not([tabindex="-1"])';
    const focusable = [
      ...dialogRef.current.querySelectorAll<HTMLElement>("*"),
    ].filter((element) => element.matches(focusableSelector));
    if (focusable.length === 0) {
      event.preventDefault();
      headingRef.current?.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    if (
      event.shiftKey &&
      (active === first || active === null || !focusable.includes(active))
    ) {
      event.preventDefault();
      last?.focus();
    } else if (
      !event.shiftKey &&
      (active === last || active === null || !focusable.includes(active))
    ) {
      event.preventDefault();
      first?.focus();
    }
  }

  return (
    <div
      className="consent-dialog-backdrop"
      data-acceptance="deepseek-consent-dialog"
      onClick={closeFromBackdrop}
    >
      <div
        ref={dialogRef}
        aria-busy={submitting}
        aria-describedby="deepseek-consent-description deepseek-consent-warning"
        aria-labelledby="deepseek-consent-title"
        aria-modal="true"
        className="consent-dialog"
        onKeyDown={trapFocus}
        role="dialog"
      >
        <button
          aria-label={t("summaryConsent.close")}
          className="consent-dialog__close"
          disabled={submitting}
          onClick={cancel}
          type="button"
        >
          ×
        </button>
        <h2 ref={headingRef} id="deepseek-consent-title" tabIndex={-1}>
          {t("summaryConsent.title")}
        </h2>
        <p id="deepseek-consent-description">
          {t("summaryConsent.description")}
        </p>

        <dl className="detail-list consent-dialog__details">
          <div>
            <dt>{t("summaryConsent.provider")}</dt>
            <dd>{disclosure.provider}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.host")}</dt>
            <dd>{disclosure.endpointHost}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.model")}</dt>
            <dd>{disclosure.model}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.sources")}</dt>
            <dd>{formatNumber(disclosure.sourceCount)}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.segments")}</dt>
            <dd>{formatNumber(disclosure.segmentCount)}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.inputSize")}</dt>
            <dd>{formatFileSize(disclosure.inputBytes)}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.template")}</dt>
            <dd>{t(`summaryConsent.templateValue.${disclosure.template}`)}</dd>
          </div>
          <div>
            <dt>{t("summaryConsent.outputLanguage")}</dt>
            <dd>
              {t(
                `summaryConsent.outputLanguageValue.${disclosure.outputLanguage}`,
              )}
            </dd>
          </div>
        </dl>

        <h3>{t("summaryConsent.dataTitle")}</h3>
        <ul className="consent-dialog__data-list">
          {disclosure.dataCategories.map((category) => (
            <li key={category}>{t(dataCategoryKey(category))}</li>
          ))}
        </ul>
        <p className="consent-dialog__not-sent">
          {t("summaryConsent.notSent")}
        </p>
        <p className="consent-dialog__warning" id="deepseek-consent-warning">
          {t("summaryConsent.warning")}
        </p>

        <label className="consent-dialog__remember">
          <input
            checked={remember}
            disabled={submitting}
            onChange={(event) => setRemember(event.currentTarget.checked)}
            type="checkbox"
          />
          <span>{t("summaryConsent.remember")}</span>
        </label>

        {errorCode === null ? null : (
          <p className="field-error" role="alert">
            {summaryErrorMessage(errorCode)}
          </p>
        )}

        <div className="form-actions consent-dialog__actions">
          <button
            className="button button--primary"
            disabled={submitting}
            onClick={() => void confirm()}
            type="button"
          >
            {submitting
              ? t("summaryConsent.confirming")
              : remember
                ? t("summaryConsent.confirmRemembered")
                : t("summaryConsent.confirmOnce")}
          </button>
          <button
            className="button button--secondary"
            disabled={submitting}
            onClick={cancel}
            type="button"
          >
            {t("common.cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}

function dataCategoryKey(category: SummaryExternalDataCategory) {
  switch (category) {
    case "transcript_text":
      return "summaryConsent.dataCategory.transcript_text" as const;
    case "speaker_labels_and_names":
      return "summaryConsent.dataCategory.speaker_labels_and_names" as const;
    case "time_offsets":
      return "summaryConsent.dataCategory.time_offsets" as const;
    case "evidence_ids":
      return "summaryConsent.dataCategory.evidence_ids" as const;
    case "summary_template":
      return "summaryConsent.dataCategory.summary_template" as const;
    case "output_language":
      return "summaryConsent.dataCategory.output_language" as const;
  }
}
