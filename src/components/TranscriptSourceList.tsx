import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { TranscriptSourceResponse } from "../api/transcript";
import { formatLocalDate } from "../i18n/formatters";
import { formatTranscriptOffset } from "../lib/transcriptPresentation";

export function TranscriptSourceList({
  deletingSourceId,
  interactionDisabled,
  onDelete,
  onSelect,
  selectedSourceId,
  sources,
}: {
  deletingSourceId: string | null;
  interactionDisabled: boolean;
  onDelete: (source: TranscriptSourceResponse) => Promise<void>;
  onSelect: (source: TranscriptSourceResponse) => void;
  selectedSourceId: string | null;
  sources: TranscriptSourceResponse[];
}) {
  const { t } = useTranslation();
  const [confirmingId, setConfirmingId] = useState<string | null>(null);

  return (
    <ul className="transcript-source-list">
      {sources.map((source) => {
        const selected = source.id === selectedSourceId;
        const deleting = source.id === deletingSourceId;
        const timeRange = sourceTimeRange(source);
        return (
          <li
            className={
              selected
                ? "transcript-source-row transcript-source-row--selected"
                : "transcript-source-row"
            }
            key={source.id}
          >
            <div className="transcript-source-row__main">
              <div>
                <strong>{source.displayName}</strong>
                <span className="transcript-source-type">
                  {t(`transcriptWorkspace.sourceType.${source.sourceType}`)}
                </span>
              </div>
              {source.originalFilename ? (
                <span className="transcript-filename">
                  {source.originalFilename}
                </span>
              ) : null}
              <dl className="transcript-source-facts">
                <div>
                  <dt>{t("transcriptWorkspace.segmentsTitle")}</dt>
                  <dd>
                    {t("transcriptWorkspace.segmentCount", {
                      count: source.segmentCount,
                    })}
                  </dd>
                </div>
                {timeRange ? (
                  <div>
                    <dt>{t("transcriptWorkspace.timeRange")}</dt>
                    <dd>{timeRange}</dd>
                  </div>
                ) : null}
                <div>
                  <dt>{t("transcriptWorkspace.speakersTitle")}</dt>
                  <dd>
                    {t("transcriptWorkspace.speakerCount", {
                      count: source.speakerCount,
                    })}
                  </dd>
                </div>
                <div>
                  <dt>{t("transcriptWorkspace.importedAt")}</dt>
                  <dd>{formatLocalDate(source.importedAt)}</dd>
                </div>
              </dl>
            </div>
            <div className="transcript-source-row__actions">
              <button
                aria-pressed={selected}
                className="button button--secondary"
                data-acceptance="view-transcript-source"
                disabled={interactionDisabled || deletingSourceId !== null}
                onClick={() => onSelect(source)}
                type="button"
              >
                {t("transcriptWorkspace.view")}
              </button>
              {confirmingId === source.id ? (
                <div className="transcript-delete-confirm" role="group">
                  <strong>{t("transcriptWorkspace.deleteConfirmTitle")}</strong>
                  <span>{t("transcriptWorkspace.deleteConfirmBody")}</span>
                  <div>
                    <button
                      className="button button--danger"
                      data-acceptance="confirm-transcript-source-delete"
                      disabled={interactionDisabled || deleting}
                      onClick={() =>
                        void onDelete(source)
                          .then(() => setConfirmingId(null))
                          .catch(() => undefined)
                      }
                      type="button"
                    >
                      {deleting
                        ? t("common.deleting")
                        : t("common.confirmDelete")}
                    </button>
                    <button
                      className="button button--ghost"
                      disabled={interactionDisabled || deleting}
                      onClick={() => setConfirmingId(null)}
                      type="button"
                    >
                      {t("common.cancel")}
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  className="button button--danger-outline"
                  data-acceptance="delete-transcript-source"
                  disabled={interactionDisabled || deletingSourceId !== null}
                  onClick={() => setConfirmingId(source.id)}
                  type="button"
                >
                  {t("transcriptWorkspace.deleteSource")}
                </button>
              )}
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function sourceTimeRange(source: TranscriptSourceResponse): string | null {
  if (source.startOffsetMs === null && source.endOffsetMs === null) return null;
  if (source.startOffsetMs === null) {
    return formatTranscriptOffset(source.endOffsetMs as number);
  }
  if (source.endOffsetMs === null) {
    return formatTranscriptOffset(source.startOffsetMs);
  }
  const start = formatTranscriptOffset(source.startOffsetMs);
  const end = formatTranscriptOffset(source.endOffsetMs);
  return start === end ? start : `${start} – ${end}`;
}
