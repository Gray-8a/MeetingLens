import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type {
  TranscriptErrorCode,
  TranscriptSegmentResponse,
} from "../api/transcript";
import { transcriptErrorMessage } from "../i18n/errors";
import {
  formatTranscriptOffset,
  segmentSpeaker,
  transcriptErrorCode,
  utf8ByteLength,
} from "../lib/transcriptPresentation";
import { ErrorNotice, LoadingState } from "./UiState";

export function TranscriptSegmentPanel({
  canGoBack,
  hasMore,
  items,
  loading,
  maxSegmentBytes,
  mutationBusy,
  onNext,
  onPrevious,
  onReload,
  onUpdate,
  page,
  sourceName,
  totalCount,
}: {
  canGoBack: boolean;
  hasMore: boolean;
  items: TranscriptSegmentResponse[];
  loading: boolean;
  maxSegmentBytes: number;
  mutationBusy: boolean;
  onNext: () => void;
  onPrevious: () => void;
  onReload: () => Promise<void>;
  onUpdate: (
    segment: TranscriptSegmentResponse,
    changes: { speakerName: string | null; originalText: string },
  ) => Promise<void>;
  page: number;
  sourceName: string;
  totalCount: number;
}) {
  const { t } = useTranslation();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [speakerName, setSpeakerName] = useState("");
  const [originalText, setOriginalText] = useState("");
  const [saving, setSaving] = useState(false);
  const [errorCode, setErrorCode] = useState<TranscriptErrorCode | null>(null);

  function edit(segment: TranscriptSegmentResponse) {
    setEditingId(segment.id);
    setSpeakerName(segment.speakerName ?? "");
    setOriginalText(segment.originalText);
    setErrorCode(null);
  }

  async function save(
    event: FormEvent<HTMLFormElement>,
    segment: TranscriptSegmentResponse,
  ) {
    event.preventDefault();
    if (saving) return;
    const normalizedText = originalText.trim();
    if (normalizedText.length === 0) {
      setErrorCode("transcript_empty");
      return;
    }
    if (utf8ByteLength(normalizedText) > maxSegmentBytes) {
      setErrorCode("transcript_segment_too_large");
      return;
    }
    setSaving(true);
    setErrorCode(null);
    try {
      await onUpdate(segment, {
        speakerName: speakerName.trim() || null,
        originalText: normalizedText,
      });
      setEditingId(null);
    } catch (reason: unknown) {
      setErrorCode(transcriptErrorCode(reason));
    } finally {
      setSaving(false);
    }
  }

  async function reloadAfterConflict() {
    if (saving || mutationBusy) return;
    setSaving(true);
    try {
      await onReload();
      setEditingId(null);
      setSpeakerName("");
      setOriginalText("");
      setErrorCode(null);
    } catch (reason: unknown) {
      setErrorCode(transcriptErrorCode(reason));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="content-panel transcript-segment-panel">
      <div className="section-heading">
        <div>
          <p className="eyebrow">{sourceName}</p>
          <h2>{t("transcriptWorkspace.segmentsTitle")}</h2>
        </div>
        <span className="transcript-total">
          {t("transcriptWorkspace.segmentCount", { count: totalCount })}
        </span>
      </div>

      {errorCode ? (
        <div className="transcript-segment-error">
          <ErrorNotice
            code={errorCode}
            message={
              errorCode === "transcript_revision_conflict"
                ? t("transcriptWorkspace.revisionConflict")
                : transcriptErrorMessage(errorCode)
            }
          />
          {errorCode === "transcript_revision_conflict" ? (
            <button
              className="button button--secondary"
              data-acceptance="reload-transcript-segments"
              disabled={saving || mutationBusy}
              onClick={() => void reloadAfterConflict()}
              type="button"
            >
              {t("transcriptWorkspace.reload")}
            </button>
          ) : null}
        </div>
      ) : null}

      {loading ? (
        <LoadingState label={t("transcriptWorkspace.loading")} />
      ) : null}
      {!loading && items.length === 0 ? (
        <p className="transcript-empty-copy">
          {t("transcriptWorkspace.emptySegments")}
        </p>
      ) : null}

      {!loading && items.length > 0 ? (
        <ol className="transcript-segment-list">
          {items.map((segment) => {
            const textBytes =
              editingId === segment.id ? utf8ByteLength(originalText) : 0;
            return (
              <li className="transcript-segment" key={segment.id}>
                <div className="transcript-segment__meta">
                  <span>{sourceName}</span>
                  <span>
                    {segment.startOffsetMs === null
                      ? t("transcriptWorkspace.ordinal", {
                          count: segment.ordinal + 1,
                        })
                      : formatTranscriptOffset(segment.startOffsetMs)}
                  </span>
                  <strong>{segmentSpeaker(segment, t)}</strong>
                </div>
                {editingId === segment.id ? (
                  <form onSubmit={(event) => void save(event, segment)}>
                    <div className="field-group">
                      <label htmlFor={`speaker-name-${segment.id}`}>
                        {t("transcriptWorkspace.speakerName")}
                      </label>
                      <input
                        autoComplete="off"
                        disabled={saving || mutationBusy}
                        id={`speaker-name-${segment.id}`}
                        maxLength={200}
                        onChange={(event) => setSpeakerName(event.target.value)}
                        value={speakerName}
                      />
                    </div>
                    <div className="field-group">
                      <label htmlFor={`segment-text-${segment.id}`}>
                        {t("transcriptWorkspace.originalText")}
                      </label>
                      <textarea
                        aria-invalid={textBytes > maxSegmentBytes}
                        disabled={saving || mutationBusy}
                        id={`segment-text-${segment.id}`}
                        onChange={(event) =>
                          setOriginalText(event.target.value)
                        }
                        rows={4}
                        value={originalText}
                      />
                      <p
                        className={
                          textBytes > maxSegmentBytes
                            ? "field-error"
                            : "field-help"
                        }
                      >
                        {t("transcriptWorkspace.textByteCount", {
                          count: textBytes,
                          limit: maxSegmentBytes,
                        })}
                      </p>
                    </div>
                    <div className="form-actions">
                      <button
                        className="button button--primary"
                        data-acceptance="save-transcript-segment"
                        disabled={
                          saving || mutationBusy || textBytes > maxSegmentBytes
                        }
                        type="submit"
                      >
                        {saving
                          ? t("common.saving")
                          : t("transcriptWorkspace.saveChanges")}
                      </button>
                      <button
                        className="button button--ghost"
                        disabled={saving || mutationBusy}
                        onClick={() => setEditingId(null)}
                        type="button"
                      >
                        {t("common.cancel")}
                      </button>
                    </div>
                  </form>
                ) : (
                  <>
                    <p>{segment.originalText}</p>
                    <button
                      className="text-link"
                      data-acceptance="edit-transcript-segment"
                      disabled={mutationBusy}
                      onClick={() => edit(segment)}
                      type="button"
                    >
                      {t("transcriptWorkspace.editSegment")}
                    </button>
                  </>
                )}
              </li>
            );
          })}
        </ol>
      ) : null}

      <div
        className="transcript-pagination"
        aria-label={t("transcriptWorkspace.segmentsTitle")}
      >
        <button
          className="button button--secondary"
          data-acceptance="transcript-segments-previous"
          disabled={!canGoBack || loading || mutationBusy}
          onClick={onPrevious}
          type="button"
        >
          {t("transcriptWorkspace.previousPage")}
        </button>
        <span>{t("transcriptWorkspace.pageSummary", { page })}</span>
        <button
          className="button button--secondary"
          data-acceptance="transcript-segments-next"
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
