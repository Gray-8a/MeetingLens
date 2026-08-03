import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  DocumentApiError,
  type DocumentErrorCode,
  type DocumentImportBatchResult,
  type DocumentImportPolicyResponse,
  type DocumentImportProgress,
  type DocumentResponse,
} from "../api/document";
import type { MeetingStatus } from "../api/meeting";
import { AppLink, meetingPath } from "../app/navigation";
import { useServices } from "../app/services";
import i18n from "../i18n";
import { documentErrorMessage } from "../i18n/errors";
import {
  formatCompletedDate,
  formatFileSize,
  formatNumber,
  formatPercent,
} from "../i18n/formatters";
import { statusLabel } from "../lib/presentation";
import { ErrorNotice, LoadingState } from "./UiState";

interface DocumentPanelProps {
  meetingId: string;
  meetingStatus: MeetingStatus;
  variant?: "full" | "compact";
}

interface SafeDocumentError {
  code: DocumentErrorCode;
}

export function DocumentPanel({
  meetingId,
  meetingStatus,
  variant = "full",
}: DocumentPanelProps) {
  const { t } = useTranslation();
  const { documentApi } = useServices();
  const activeRef = useRef(true);
  const [documents, setDocuments] = useState<DocumentResponse[]>([]);
  const [policy, setPolicy] = useState<DocumentImportPolicyResponse | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState<Map<number, DocumentImportProgress>>(
    new Map(),
  );
  const [batchResult, setBatchResult] =
    useState<DocumentImportBatchResult | null>(null);
  const [error, setError] = useState<SafeDocumentError | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const loadDocuments = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [list, importPolicy] = await Promise.all([
        documentApi.list(meetingId),
        documentApi.getImportPolicy(),
      ]);
      if (!activeRef.current) return;
      setDocuments(list.documents);
      setPolicy(importPolicy);
    } catch (reason: unknown) {
      if (activeRef.current) setError(toSafeDocumentError(reason));
    } finally {
      if (activeRef.current) setLoading(false);
    }
  }, [documentApi, meetingId]);

  useEffect(() => {
    activeRef.current = true;
    const timer = window.setTimeout(() => void loadDocuments(), 0);
    return () => {
      activeRef.current = false;
      window.clearTimeout(timer);
    };
  }, [loadDocuments]);

  async function selectAndImport() {
    if (importing || !canImportDocuments(meetingStatus)) return;
    setImporting(true);
    setError(null);
    setBatchResult(null);
    setProgress(new Map());
    try {
      const result = await documentApi.selectAndImport(meetingId, (event) => {
        if (!activeRef.current || event.phase === "complete") return;
        setProgress((current) => {
          const next = new Map(current);
          next.set(event.fileIndex, event);
          return next;
        });
      });
      if (!activeRef.current) return;
      if (!result.cancelled) {
        setBatchResult(result);
        const list = await documentApi.list(meetingId);
        if (activeRef.current) setDocuments(list.documents);
      }
    } catch (reason: unknown) {
      if (activeRef.current) setError(toSafeDocumentError(reason));
    } finally {
      if (activeRef.current) setImporting(false);
    }
  }

  async function deleteDocument(document: DocumentResponse, retry: boolean) {
    if (deletingId !== null) return;
    setDeletingId(document.id);
    setError(null);
    try {
      const result = retry
        ? await documentApi.retryDelete(document.id)
        : await documentApi.requestDelete(document.id);
      if (!activeRef.current) return;
      if (result.deleted) {
        setDocuments((current) =>
          current.filter((candidate) => candidate.id !== document.id),
        );
        setConfirmingDelete(null);
      } else if (result.pending) {
        setDocuments((current) =>
          current.map((candidate) =>
            candidate.id === document.id
              ? { ...candidate, status: "deleting" }
              : candidate,
          ),
        );
        setError({ code: "document_delete_pending" });
      }
    } catch (reason: unknown) {
      if (activeRef.current) setError(toSafeDocumentError(reason));
    } finally {
      if (activeRef.current) setDeletingId(null);
    }
  }

  const importAllowed = canImportDocuments(meetingStatus);
  const usageBytes = documents.reduce(
    (total, document) => total + (document.sizeBytes ?? 0),
    0,
  );
  const recentProgress = [...progress.values()].sort(
    (left, right) => left.fileIndex - right.fileIndex,
  );
  const latestProgress = recentProgress[recentProgress.length - 1];

  if (variant === "compact") {
    return (
      <section className="document-panel document-panel--compact content-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t("document.meetingFiles")}</p>
            <h2>
              {t("document.fileCount", {
                count: documents.length,
                formattedCount: formatNumber(documents.length),
              })}
            </h2>
          </div>
          <button
            className="button button--secondary"
            data-acceptance="document-add-files-live"
            disabled={!importAllowed || importing}
            onClick={() => void selectAndImport()}
            type="button"
          >
            {importing ? t("common.importing") : t("common.addFiles")}
          </button>
        </div>
        {error ? (
          <ErrorNotice
            code={error.code}
            message={documentErrorMessage(error.code)}
          />
        ) : null}
        {latestProgress ? (
          <DocumentProgressLine progress={latestProgress} />
        ) : (
          <p className="field-help">{t("document.filesStayLocal")}</p>
        )}
        <AppLink className="text-link" to={meetingPath(meetingId)}>
          {t("document.openFullList")}
        </AppLink>
      </section>
    );
  }

  return (
    <section
      className="document-panel content-panel"
      data-acceptance="document-panel"
      aria-labelledby="meeting-documents-title"
    >
      <div className="section-heading">
        <div>
          <p className="eyebrow">{t("document.localContext")}</p>
          <h2 id="meeting-documents-title">{t("document.meetingFiles")}</h2>
          <p className="supporting-copy">{t("document.description")}</p>
        </div>
        <button
          className="button button--secondary"
          data-acceptance="document-add-files"
          disabled={!importAllowed || importing}
          onClick={() => void selectAndImport()}
          type="button"
        >
          {importing
            ? t("document.selectingAndImporting")
            : t("common.addFiles")}
        </button>
      </div>

      {policy ? (
        <div
          className="document-policy"
          aria-label={t("document.importLimits")}
        >
          <span>
            {t("document.policyCount", {
              current: formatNumber(documents.length),
              maximum: formatNumber(policy.maxMeetingFileCount),
            })}
          </span>
          <span>
            {formatFileSize(usageBytes)} /{" "}
            {formatFileSize(policy.maxMeetingTotalBytes)}
          </span>
          <span>
            {policy.allowedExtensions
              .map((extension) => `.${extension}`)
              .join(", ")}
          </span>
        </div>
      ) : null}

      {!importAllowed ? (
        <p
          className="restriction"
          data-acceptance="document-import-restriction"
        >
          {t("document.changeRestricted", {
            status: statusLabel(meetingStatus),
          })}
        </p>
      ) : null}
      {error ? (
        <ErrorNotice
          code={error.code}
          message={documentErrorMessage(error.code)}
        />
      ) : null}

      {recentProgress.length > 0 ? (
        <div
          className="document-progress"
          data-acceptance="document-import-progress"
          aria-live="polite"
        >
          <h3>{t("document.importProgress")}</h3>
          <ul>
            {recentProgress.map((event) => (
              <li key={`${event.batchId}-${event.fileIndex}`}>
                <DocumentProgressLine progress={event} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {batchResult &&
      (batchResult.duplicates.length > 0 || batchResult.failed.length > 0) ? (
        <div className="document-batch-summary" aria-live="polite">
          {batchResult.duplicates.map((item) => (
            <p key={`duplicate-${item.documentId}`}>
              {t("document.alreadyInMeeting", {
                filename: displayFilename(item.originalFilename),
              })}
            </p>
          ))}
          {batchResult.failed.map((item) => (
            <p key={`failed-${item.documentId}`}>
              {displayFilename(item.originalFilename)} ·{" "}
              {formatDocumentError(item.errorCode ?? "internal_error")}
            </p>
          ))}
        </div>
      ) : null}

      {loading ? <LoadingState label={t("document.loading")} /> : null}
      {!loading && documents.length === 0 ? (
        <div className="document-empty" data-acceptance="document-empty-state">
          <h3>{t("document.emptyTitle")}</h3>
          <p>{t("document.emptyBody")}</p>
        </div>
      ) : null}
      {!loading && documents.length > 0 ? (
        <ul className="document-list" data-acceptance="document-list">
          {documents.map((document) => {
            const deleting = deletingId === document.id;
            const canDelete = canDeleteDocument(document.status);
            return (
              <li className="document-row" key={document.id}>
                <div className="document-row__main">
                  <strong>{displayFilename(document.originalFilename)}</strong>
                  <span>
                    {document.safeExtension.toUpperCase()} ·{" "}
                    {formatFileSize(document.sizeBytes ?? 0)}
                  </span>
                  <span>{formatCompletedDate(document.importedAt)}</span>
                </div>
                <span
                  className={`document-status document-status--${document.status}`}
                >
                  {documentStatusLabel(document.status)}
                </span>
                <div className="document-row__actions">
                  {document.status === "delete_failed" ? (
                    <button
                      className="button button--secondary"
                      disabled={deleting}
                      onClick={() => void deleteDocument(document, true)}
                      type="button"
                    >
                      {deleting
                        ? t("common.retrying")
                        : t("common.retryDeletion")}
                    </button>
                  ) : confirmingDelete === document.id ? (
                    <div className="document-delete-confirm" role="group">
                      <span>{t("document.deleteCopyQuestion")}</span>
                      <button
                        className="button button--danger"
                        disabled={deleting}
                        onClick={() => void deleteDocument(document, false)}
                        type="button"
                      >
                        {deleting ? t("common.deleting") : t("common.confirm")}
                      </button>
                      <button
                        className="button button--ghost"
                        disabled={deleting}
                        onClick={() => setConfirmingDelete(null)}
                        type="button"
                      >
                        {t("common.cancel")}
                      </button>
                    </div>
                  ) : (
                    <button
                      aria-label={t("document.deleteFileAria", {
                        filename: displayFilename(document.originalFilename),
                      })}
                      className="button button--danger-outline"
                      disabled={!canDelete || deleting}
                      onClick={() => setConfirmingDelete(document.id)}
                      type="button"
                    >
                      {t("common.delete")}
                    </button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}

function DocumentProgressLine({
  progress,
}: {
  progress: DocumentImportProgress;
}) {
  const { t } = useTranslation();
  const total = progress.totalBytes;
  const ratio =
    total && total > 0
      ? Math.min(1, Math.max(0, progress.bytesProcessed / total))
      : null;
  return (
    <div className="document-progress-line">
      <span>{displayFilename(progress.originalFilename)}</span>
      <span>{t(`document.progress.${progress.phase}`)}</span>
      {ratio === null ? null : <span>{formatPercent(ratio)}</span>}
      {progress.errorCode ? (
        <span>{formatDocumentError(progress.errorCode)}</span>
      ) : null}
    </div>
  );
}

function canImportDocuments(status: MeetingStatus): boolean {
  return status === "draft" || status === "recording" || status === "ready";
}

function canDeleteDocument(status: DocumentResponse["status"]): boolean {
  return (
    status === "staged" ||
    status === "import_failed" ||
    status === "parsed" ||
    status === "parse_failed"
  );
}

function documentStatusLabel(status: DocumentResponse["status"]): string {
  return i18n.t(`document.status.${status}`);
}

function displayFilename(value: string): string {
  const safe = Array.from(value, (character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)
      ? "�"
      : character;
  }).join("");
  return safe.length === 0 ? i18n.t("common.selectedFile") : safe;
}

function toSafeDocumentError(reason: unknown): SafeDocumentError {
  return {
    code: reason instanceof DocumentApiError ? reason.code : "internal_error",
  };
}

function formatDocumentError(code: DocumentErrorCode): string {
  return `${documentErrorMessage(code)} (${code})`;
}
