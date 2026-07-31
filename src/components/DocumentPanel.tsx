import { useCallback, useEffect, useRef, useState } from "react";
import { AppLink, meetingPath } from "../app/navigation";
import { useServices } from "../app/services";
import {
  DocumentApiError,
  documentErrorMessage,
  type DocumentErrorCode,
  type DocumentImportBatchResult,
  type DocumentImportPolicyResponse,
  type DocumentImportProgress,
  type DocumentResponse,
} from "../api/document";
import type { MeetingStatus } from "../api/meeting";
import { ErrorNotice, LoadingState } from "./UiState";

interface DocumentPanelProps {
  meetingId: string;
  meetingStatus: MeetingStatus;
  variant?: "full" | "compact";
}

interface SafeDocumentError {
  code: DocumentErrorCode;
  message: string;
  retryable: boolean;
}

export function DocumentPanel({
  meetingId,
  meetingStatus,
  variant = "full",
}: DocumentPanelProps) {
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
        setError({
          code: "document_delete_pending",
          message: "The document is still being deleted.",
          retryable: true,
        });
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
            <p className="eyebrow">Meeting files</p>
            <h2>
              {documents.length} {documents.length === 1 ? "file" : "files"}
            </h2>
          </div>
          <button
            className="button button--secondary"
            data-acceptance="document-add-files-live"
            disabled={!importAllowed || importing}
            onClick={() => void selectAndImport()}
            type="button"
          >
            {importing ? "Importing…" : "Add files"}
          </button>
        </div>
        {error ? (
          <ErrorNotice code={error.code} message={error.message} />
        ) : null}
        {latestProgress ? (
          <DocumentProgressLine progress={latestProgress} />
        ) : (
          <p className="field-help">Files stay local and are not parsed yet.</p>
        )}
        <AppLink className="text-link" to={meetingPath(meetingId)}>
          Open full file list
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
          <p className="eyebrow">Local context</p>
          <h2 id="meeting-documents-title">Meeting files</h2>
          <p className="supporting-copy">
            Stored locally only. Files are not opened, parsed, or sent to Python
            or external AI.
          </p>
        </div>
        <button
          className="button button--secondary"
          data-acceptance="document-add-files"
          disabled={!importAllowed || importing}
          onClick={() => void selectAndImport()}
          type="button"
        >
          {importing ? "Selecting and importing…" : "Add files"}
        </button>
      </div>

      {policy ? (
        <div className="document-policy" aria-label="Document import limits">
          <span>
            {documents.length} / {policy.maxMeetingFileCount} files
          </span>
          <span>
            {formatBytes(usageBytes)} /{" "}
            {formatBytes(policy.maxMeetingTotalBytes)}
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
          Files cannot be changed while this meeting is{" "}
          {meetingStatus.replace("_", " ")}.
        </p>
      ) : null}
      {error ? <ErrorNotice code={error.code} message={error.message} /> : null}

      {recentProgress.length > 0 ? (
        <div
          className="document-progress"
          data-acceptance="document-import-progress"
          aria-live="polite"
        >
          <h3>Import progress</h3>
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
              {displayFilename(item.originalFilename)} is already in this
              meeting.
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

      {loading ? <LoadingState label="Loading meeting files…" /> : null}
      {!loading && documents.length === 0 ? (
        <div className="document-empty" data-acceptance="document-empty-state">
          <h3>No files added</h3>
          <p>
            Use the native picker to copy approved files into private app
            storage.
          </p>
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
                    {formatBytes(document.sizeBytes ?? 0)}
                  </span>
                  <span>{formatDocumentDate(document.importedAt)}</span>
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
                      {deleting ? "Retrying…" : "Retry deletion"}
                    </button>
                  ) : confirmingDelete === document.id ? (
                    <div className="document-delete-confirm" role="group">
                      <span>
                        Delete local copy? The source file is unchanged.
                      </span>
                      <button
                        className="button button--danger"
                        disabled={deleting}
                        onClick={() => void deleteDocument(document, false)}
                        type="button"
                      >
                        {deleting ? "Deleting…" : "Confirm"}
                      </button>
                      <button
                        className="button button--ghost"
                        disabled={deleting}
                        onClick={() => setConfirmingDelete(null)}
                        type="button"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      aria-label={`Delete ${displayFilename(document.originalFilename)}`}
                      className="button button--danger-outline"
                      disabled={!canDelete || deleting}
                      onClick={() => setConfirmingDelete(document.id)}
                      type="button"
                    >
                      Delete
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
  const total = progress.totalBytes;
  const percentage =
    total && total > 0
      ? Math.min(100, Math.round((progress.bytesProcessed / total) * 100))
      : null;
  return (
    <div className="document-progress-line">
      <span>{displayFilename(progress.originalFilename)}</span>
      <span>{progressLabel(progress.phase)}</span>
      {percentage === null ? null : <span>{percentage}%</span>}
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
  const labels: Record<DocumentResponse["status"], string> = {
    importing: "Importing",
    staged: "Imported",
    import_failed: "Import failed",
    parsing: "Parsing",
    parsed: "Parsed",
    parse_failed: "Parse failed",
    deleting: "Deleting",
    delete_failed: "Delete failed",
    unknown: "Unavailable",
  };
  return labels[status];
}

function progressLabel(phase: DocumentImportProgress["phase"]): string {
  const labels: Record<DocumentImportProgress["phase"], string> = {
    selected: "Selected",
    validating: "Validating",
    copying: "Copying",
    committing: "Saving",
    imported: "Imported",
    duplicate: "Already imported",
    failed: "Failed",
    complete: "Complete",
  };
  return labels[phase];
}

function displayFilename(value: string): string {
  const safe = Array.from(value, (character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)
      ? "�"
      : character;
  }).join("");
  return safe.length === 0 ? "Selected file" : safe;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

function formatDocumentDate(value: string | null): string {
  if (value === null) return "Not completed";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown time" : date.toLocaleString();
}

function toSafeDocumentError(reason: unknown): SafeDocumentError {
  if (reason instanceof DocumentApiError) {
    return {
      code: reason.code,
      message: reason.message,
      retryable: reason.retryable,
    };
  }
  return {
    code: "internal_error",
    message: "An internal error occurred.",
    retryable: false,
  };
}

function formatDocumentError(code: DocumentErrorCode): string {
  return `${documentErrorMessage(code)} (${code})`;
}
