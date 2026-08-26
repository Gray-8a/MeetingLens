import { useEffect, useRef, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AudioTranscriptionResult } from "../api/audioTranscription";
import { useServices } from "../app/services";
import type {
  TranscriptErrorCode,
  TranscriptImportResult,
  TranscriptImportPolicyResponse,
} from "../api/transcript";
import { transcriptErrorMessage } from "../i18n/errors";
import {
  defaultPrimaryLanguage,
  defaultSummaryMeetingTitle,
  FALLBACK_PASTE_LIMIT_BYTES,
  transcriptErrorCode,
  utf8ByteLength,
} from "../lib/transcriptPresentation";
import { AudioTranscriptionPanel } from "./AudioTranscriptionPanel";
import { ErrorNotice, InfoNotice } from "./UiState";

interface CreateTarget {
  kind: "create";
}

interface ExistingTarget {
  kind: "existing";
  meetingId: string;
}

export function TranscriptInputPanel({
  target,
  interactionDisabled = false,
  onCancel,
  onCompleted,
}: {
  target: CreateTarget | ExistingTarget;
  interactionDisabled?: boolean;
  onCancel?: () => void;
  onCompleted: (
    result: TranscriptImportResult | AudioTranscriptionResult,
  ) => void;
}) {
  const { t } = useTranslation();
  const { transcriptApi } = useServices();
  const [title, setTitle] = useState("");
  const [pastedText, setPastedText] = useState("");
  const [policy, setPolicy] = useState<TranscriptImportPolicyResponse | null>(
    null,
  );
  const [busy, setBusy] = useState<"paste" | "import" | null>(null);
  const [audioBusy, setAudioBusy] = useState(false);
  const [errorCode, setErrorCode] = useState<TranscriptErrorCode | null>(null);
  const [duplicate, setDuplicate] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    void transcriptApi
      .getImportPolicy()
      .then((result) => {
        if (mounted.current) setPolicy(result);
      })
      .catch((reason: unknown) => {
        if (mounted.current) setErrorCode(transcriptErrorCode(reason));
      });
    return () => {
      mounted.current = false;
    };
  }, [transcriptApi]);

  const byteCount = utf8ByteLength(pastedText);
  const pasteLimit = policy?.maxPasteBytes ?? FALLBACK_PASTE_LIMIT_BYTES;
  const byteLimitExceeded = byteCount > pasteLimit;

  function meetingTitle(): string {
    const normalized = title.trim();
    return normalized.length > 0
      ? normalized
      : defaultSummaryMeetingTitle(t("modes.summaryTitle"));
  }

  async function submitPaste(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy !== null || audioBusy || interactionDisabled) return;
    if (pastedText.trim().length === 0) {
      setErrorCode("transcript_empty");
      return;
    }
    if (byteLimitExceeded) {
      setErrorCode("transcript_too_large");
      return;
    }

    setBusy("paste");
    setErrorCode(null);
    setDuplicate(false);
    try {
      const result =
        target.kind === "create"
          ? await transcriptApi.createMeetingFromPaste({
              title: meetingTitle(),
              primaryLanguage: defaultPrimaryLanguage(),
              pastedText,
              displayName: t("summaryMode.pasteTitle"),
            })
          : await transcriptApi.addPasteToMeeting({
              meetingId: target.meetingId,
              pastedText,
              displayName: t("summaryMode.pasteTitle"),
            });
      if (!mounted.current || result.cancelled) return;
      setPastedText("");
      setDuplicate(result.duplicate);
      onCompleted(result);
    } catch (reason: unknown) {
      if (mounted.current) setErrorCode(transcriptErrorCode(reason));
    } finally {
      if (mounted.current) setBusy(null);
    }
  }

  async function selectAndImport() {
    if (busy !== null || audioBusy || interactionDisabled) return;
    setBusy("import");
    setErrorCode(null);
    setDuplicate(false);
    try {
      const result =
        target.kind === "create"
          ? await transcriptApi.selectTranscriptAndCreateMeeting({
              title: meetingTitle(),
              primaryLanguage: defaultPrimaryLanguage(),
            })
          : await transcriptApi.selectAndAddToMeeting({
              meetingId: target.meetingId,
            });
      if (!mounted.current || result.cancelled) return;
      setDuplicate(result.duplicate);
      onCompleted(result);
    } catch (reason: unknown) {
      if (mounted.current) setErrorCode(transcriptErrorCode(reason));
    } finally {
      if (mounted.current) setBusy(null);
    }
  }

  function cancel() {
    if (busy !== null || audioBusy || interactionDisabled) return;
    setTitle("");
    setPastedText("");
    setDuplicate(false);
    setErrorCode(null);
    onCancel?.();
  }

  return (
    <div className="transcript-input-grid">
      {target.kind === "create" ? (
        <div className="field-group transcript-input-title">
          <label htmlFor="summary-meeting-title">
            {t("summaryMode.meetingTitle")}
          </label>
          <input
            autoComplete="off"
            disabled={busy !== null || audioBusy || interactionDisabled}
            id="summary-meeting-title"
            maxLength={200}
            onChange={(event) => setTitle(event.target.value)}
            placeholder={t("summaryMode.titlePlaceholder")}
            value={title}
          />
          <p className="field-help">{t("summaryMode.meetingTitleOptional")}</p>
        </div>
      ) : null}

      {errorCode ? (
        <ErrorNotice
          code={errorCode}
          message={transcriptErrorMessage(errorCode)}
        />
      ) : null}
      {duplicate ? (
        <InfoNotice>
          <strong>{t("summaryMode.duplicate")}</strong>
          <span>{t("summaryMode.duplicateBody")}</span>
        </InfoNotice>
      ) : null}

      <section className="transcript-input-card">
        <div>
          <h2>{t("summaryMode.pasteTitle")}</h2>
          <p>{t("summaryMode.pasteDescription")}</p>
        </div>
        <form onSubmit={submitPaste}>
          <div className="field-group">
            <label htmlFor={`transcript-paste-${target.kind}`}>
              {t("summaryMode.pasteLabel")}
            </label>
            <textarea
              aria-describedby={`transcript-byte-count-${target.kind}`}
              aria-invalid={byteLimitExceeded}
              disabled={busy !== null || audioBusy || interactionDisabled}
              id={`transcript-paste-${target.kind}`}
              onChange={(event) => {
                setPastedText(event.target.value);
                setErrorCode(null);
                setDuplicate(false);
              }}
              placeholder={t("summaryMode.pastePlaceholder")}
              rows={target.kind === "create" ? 12 : 7}
              value={pastedText}
            />
            <div
              className={byteLimitExceeded ? "field-error" : "field-help"}
              id={`transcript-byte-count-${target.kind}`}
            >
              <span>{t("summaryMode.byteCount", { count: byteCount })}</span>
              <span>{t("summaryMode.byteLimit", { count: pasteLimit })}</span>
            </div>
          </div>
          <div className="form-actions">
            <button
              className="button button--primary"
              data-acceptance={`submit-paste-${target.kind}`}
              disabled={
                busy !== null ||
                audioBusy ||
                interactionDisabled ||
                byteLimitExceeded
              }
              type="submit"
            >
              {busy === "paste"
                ? t("common.saving")
                : t("summaryMode.continue")}
            </button>
            {onCancel ? (
              <button
                className="button button--ghost"
                disabled={busy !== null || audioBusy || interactionDisabled}
                onClick={cancel}
                type="button"
              >
                {t("summaryMode.cancel")}
              </button>
            ) : null}
          </div>
        </form>
      </section>

      <section className="transcript-input-card transcript-input-card--import">
        <div>
          <h2>{t("summaryMode.importTitle")}</h2>
          <p>{t("summaryMode.importDescription")}</p>
          <p className="field-help">{t("summaryMode.supportedTypes")}</p>
        </div>
        <button
          className="button button--secondary"
          data-acceptance={`select-transcript-${target.kind}`}
          disabled={busy !== null || audioBusy || interactionDisabled}
          onClick={() => void selectAndImport()}
          type="button"
        >
          {busy === "import"
            ? t("common.importing")
            : t("summaryMode.importButton")}
        </button>
      </section>

      <AudioTranscriptionPanel
        interactionDisabled={busy !== null || interactionDisabled}
        onBusyChange={setAudioBusy}
        onCompleted={onCompleted}
        target={
          target.kind === "create"
            ? {
                kind: "create",
                meetingTitle,
                primaryLanguage: defaultPrimaryLanguage,
              }
            : target
        }
      />
    </div>
  );
}
