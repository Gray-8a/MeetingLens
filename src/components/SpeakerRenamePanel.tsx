import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { TranscriptApi, TranscriptErrorCode } from "../api/transcript";
import { transcriptErrorMessage } from "../i18n/errors";
import { transcriptErrorCode } from "../lib/transcriptPresentation";
import { ErrorNotice, LoadingState } from "./UiState";

type SpeakerGroup = Awaited<
  ReturnType<TranscriptApi["listSpeakerGroups"]>
>["items"][number];

export function SpeakerRenamePanel({
  canGoBack,
  groups,
  hasMore,
  loading,
  mutationBusy,
  onNext,
  onPrevious,
  onRename,
  page,
}: {
  canGoBack: boolean;
  groups: SpeakerGroup[];
  hasMore: boolean;
  loading: boolean;
  mutationBusy: boolean;
  onNext: () => void;
  onPrevious: () => void;
  onRename: (group: SpeakerGroup, speakerName: string | null) => Promise<void>;
  page: number;
}) {
  const { t } = useTranslation();
  const [pending, setPending] = useState<{
    group: SpeakerGroup;
    speakerName: string | null;
  } | null>(null);
  const [saving, setSaving] = useState(false);
  const [errorCode, setErrorCode] = useState<TranscriptErrorCode | null>(null);

  function requestRename(
    event: FormEvent<HTMLFormElement>,
    group: SpeakerGroup,
  ) {
    event.preventDefault();
    const formData = new FormData(event.currentTarget);
    const normalized = String(formData.get("speakerName") ?? "").trim();
    if (normalized.length === 0) {
      setErrorCode("transcript_invalid_input");
      return;
    }
    setErrorCode(null);
    setPending({ group, speakerName: normalized });
  }

  async function confirmRename() {
    if (!pending || saving) return;
    setSaving(true);
    setErrorCode(null);
    try {
      await onRename(pending.group, pending.speakerName);
      setPending(null);
    } catch (reason: unknown) {
      setErrorCode(transcriptErrorCode(reason));
    } finally {
      setSaving(false);
    }
  }

  if (!loading && groups.length === 0) return null;

  return (
    <section className="content-panel speaker-panel">
      <div className="section-heading section-heading--stacked">
        <p className="eyebrow">{t("transcriptWorkspace.speakersTitle")}</p>
        <h2>{t("transcriptWorkspace.speakersTitle")}</h2>
        <p>{t("transcriptWorkspace.speakersDescription")}</p>
      </div>

      {errorCode ? (
        <ErrorNotice
          code={errorCode}
          message={transcriptErrorMessage(errorCode)}
        />
      ) : null}
      {loading ? (
        <LoadingState label={t("transcriptWorkspace.loading")} />
      ) : null}

      {!loading ? (
        <ul className="speaker-group-list">
          {groups.map((group, index) => (
            <li className="speaker-group-row" key={group.speakerLabel}>
              <div>
                <strong>{group.speakerLabel}</strong>
                <span>
                  {t("transcriptWorkspace.affectedSegments", {
                    count: group.segmentCount,
                  })}
                </span>
                {group.hasMixedNames ? (
                  <span>{t("transcriptWorkspace.mixedSpeakerNames")}</span>
                ) : null}
              </div>
              <form onSubmit={(event) => requestRename(event, group)}>
                <label htmlFor={`speaker-group-${index}`}>
                  {t("transcriptWorkspace.speakerName")}
                </label>
                <input
                  autoComplete="off"
                  defaultValue={group.speakerName ?? ""}
                  disabled={saving || mutationBusy}
                  id={`speaker-group-${index}`}
                  key={`${group.speakerLabel}:${group.speakerName ?? ""}:${group.hasMixedNames}`}
                  maxLength={200}
                  name="speakerName"
                />
                <div className="form-actions">
                  <button
                    className="button button--secondary"
                    data-acceptance="rename-transcript-speaker"
                    disabled={saving || mutationBusy}
                    type="submit"
                  >
                    {t("transcriptWorkspace.renameSpeaker")}
                  </button>
                  <button
                    className="button button--ghost"
                    disabled={
                      saving ||
                      mutationBusy ||
                      (group.speakerName === null && !group.hasMixedNames)
                    }
                    onClick={() => {
                      setErrorCode(null);
                      setPending({ group, speakerName: null });
                    }}
                    type="button"
                  >
                    {t("transcriptWorkspace.clearSpeakerName")}
                  </button>
                </div>
              </form>
            </li>
          ))}
        </ul>
      ) : null}

      {pending ? (
        <div className="confirm-panel speaker-confirm" role="group">
          <strong>{t("transcriptWorkspace.renameConfirmTitle")}</strong>
          <p>
            {t("transcriptWorkspace.renameConfirmBody", {
              count: pending.group.segmentCount,
              speaker: pending.group.speakerLabel,
            })}
          </p>
          <div className="form-actions">
            <button
              className="button button--primary"
              data-acceptance="confirm-transcript-speaker-rename"
              disabled={saving || mutationBusy}
              onClick={() => void confirmRename()}
              type="button"
            >
              {saving ? t("common.saving") : t("common.confirm")}
            </button>
            <button
              className="button button--ghost"
              disabled={saving || mutationBusy}
              onClick={() => setPending(null)}
              type="button"
            >
              {t("common.cancel")}
            </button>
          </div>
        </div>
      ) : null}

      <div className="transcript-pagination">
        <button
          className="button button--secondary"
          data-acceptance="transcript-speakers-previous"
          disabled={!canGoBack || loading || mutationBusy}
          onClick={onPrevious}
          type="button"
        >
          {t("transcriptWorkspace.previousPage")}
        </button>
        <span>{t("transcriptWorkspace.pageSummary", { page })}</span>
        <button
          className="button button--secondary"
          data-acceptance="transcript-speakers-next"
          disabled={!hasMore || loading || mutationBusy}
          onClick={onNext}
          type="button"
        >
          {t("transcriptWorkspace.nextPage")}
        </button>
      </div>
    </section>
  );
}
