import { useCallback, useEffect, useState, type FormEvent } from "react";
import { AppLink, liveMeetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { DocumentPanel } from "../components/DocumentPanel";
import { StatusBadge } from "../components/StatusBadge";
import { ErrorNotice, LoadingState } from "../components/UiState";
import { formatLocalDate, uiError } from "../lib/presentation";
import type { DatabaseStatus } from "./HomePage";

interface MeetingDetailPageProps {
  meetingId: string;
  onDatabaseStatus: (status: DatabaseStatus) => void;
  onMeetingStatus: (status: string | null) => void;
}

export function MeetingDetailPage({
  meetingId,
  onDatabaseStatus,
  onMeetingStatus,
}: MeetingDetailPageProps) {
  const { meetingApi } = useServices();
  const { navigate } = useNavigation();
  const [meeting, setMeeting] = useState<MeetingResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");
  const [validationMessage, setValidationMessage] = useState<string | null>(
    null,
  );
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const loadMeeting = useCallback(async () => {
    setLoading(true);
    setError(null);
    onDatabaseStatus("checking");
    try {
      const result = await meetingApi.get(meetingId);
      setMeeting(result);
      setTitle(result.title);
      onMeetingStatus(result.status);
      onDatabaseStatus("ready");
    } catch (reason: unknown) {
      setMeeting(null);
      onMeetingStatus(null);
      const safeError = uiError(reason);
      setError(safeError);
      onDatabaseStatus(
        safeError.code === "meeting_not_found" ? "ready" : "unavailable",
      );
    } finally {
      setLoading(false);
    }
  }, [meetingApi, meetingId, onDatabaseStatus, onMeetingStatus]);

  useEffect(() => {
    const timer = window.setTimeout(() => void loadMeeting(), 0);
    return () => {
      window.clearTimeout(timer);
      onMeetingStatus(null);
    };
  }, [loadMeeting, onMeetingStatus]);

  async function saveTitle(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!meeting || busyAction) return;
    const normalized = title.trim();
    if (normalized.length === 0) {
      setValidationMessage("Enter a meeting title.");
      return;
    }

    setBusyAction("update");
    setError(null);
    try {
      const updated = await meetingApi.update({
        id: meeting.id,
        title: normalized,
      });
      setMeeting(updated);
      setTitle(updated.title);
      setEditing(false);
      setValidationMessage(null);
    } catch (reason: unknown) {
      setError(uiError(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function startMeeting() {
    if (!meeting || busyAction) return;
    setBusyAction("start");
    setError(null);
    try {
      const recording = await meetingApi.start(meeting.id);
      setMeeting(recording);
      onMeetingStatus(recording.status);
      navigate(liveMeetingPath(recording.id));
    } catch (reason: unknown) {
      setError(uiError(reason));
      setBusyAction(null);
    }
  }

  async function endMeeting() {
    if (!meeting || busyAction) return;
    setBusyAction("end");
    setError(null);
    try {
      const ready = await meetingApi.end(meeting.id);
      setMeeting(ready);
      onMeetingStatus(ready.status);
    } catch (reason: unknown) {
      setError(uiError(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function deleteMeeting() {
    if (!meeting || busyAction) return;
    setBusyAction("delete");
    setError(null);
    try {
      await meetingApi.delete(meeting.id);
      navigate("/history");
    } catch (reason: unknown) {
      const safeError = uiError(reason);
      setError(safeError);
      if (
        safeError.code === "meeting_delete_failed" ||
        safeError.code === "meeting_delete_pending"
      ) {
        try {
          const refreshed = await meetingApi.get(meeting.id);
          setMeeting(refreshed);
          onMeetingStatus(refreshed.status);
        } catch {
          // Keep the stable delete error if the refresh races final cleanup.
        }
      }
      setBusyAction(null);
      setConfirmingDelete(false);
    }
  }

  if (loading) return <LoadingState label="Loading meeting details…" />;

  if (meeting === null) {
    return (
      <section className="content-panel narrow-panel">
        {error ? (
          <ErrorNotice code={error.code} message={error.message} />
        ) : null}
        <AppLink className="button button--secondary" to="/history">
          Back to history
        </AppLink>
      </section>
    );
  }

  const canDelete = meeting.status === "draft" || meeting.status === "ready";
  const canRetryDelete = meeting.status === "delete_failed";

  return (
    <div className="page-stack">
      <section className="detail-hero" data-acceptance="meeting-detail">
        <div>
          <p className="eyebrow">Meeting details</p>
          <div className="title-with-status">
            <h1>{meeting.title}</h1>
            <StatusBadge status={meeting.status} />
          </div>
          <p className="meeting-id">ID: {meeting.id}</p>
        </div>
        <AppLink className="text-link" to="/history">
          Back to history
        </AppLink>
      </section>

      {error ? <ErrorNotice code={error.code} message={error.message} /> : null}

      <section className="detail-grid">
        <article className="content-panel">
          <p className="eyebrow">Timeline</p>
          <dl className="detail-list">
            <div>
              <dt>Created</dt>
              <dd>{formatLocalDate(meeting.createdAt)}</dd>
            </div>
            <div>
              <dt>Started</dt>
              <dd>{formatLocalDate(meeting.startedAt)}</dd>
            </div>
            <div>
              <dt>Ended</dt>
              <dd>{formatLocalDate(meeting.endedAt)}</dd>
            </div>
            <div>
              <dt>Language baseline</dt>
              <dd>{meeting.primaryLanguage}</dd>
            </div>
          </dl>
        </article>

        <article className="content-panel">
          <p className="eyebrow">Actions</p>
          {meeting.status === "draft" ? (
            <>
              {editing ? (
                <form className="inline-form" onSubmit={saveTitle}>
                  <label htmlFor="edit-meeting-title">Meeting title</label>
                  <input
                    data-acceptance="edit-title-input"
                    disabled={busyAction === "update"}
                    id="edit-meeting-title"
                    maxLength={200}
                    onChange={(event) => {
                      setTitle(event.target.value);
                      setValidationMessage(null);
                    }}
                    value={title}
                  />
                  {validationMessage ? (
                    <p className="field-error">{validationMessage}</p>
                  ) : null}
                  <div className="form-actions">
                    <button
                      className="button button--primary"
                      data-acceptance="save-title"
                      disabled={busyAction === "update"}
                      type="submit"
                    >
                      {busyAction === "update" ? "Saving…" : "Save title"}
                    </button>
                    <button
                      className="button button--ghost"
                      onClick={() => {
                        setEditing(false);
                        setTitle(meeting.title);
                      }}
                      type="button"
                    >
                      Cancel
                    </button>
                  </div>
                </form>
              ) : (
                <button
                  className="button button--secondary"
                  data-acceptance="edit-title"
                  onClick={() => setEditing(true)}
                  type="button"
                >
                  Edit title
                </button>
              )}
              <button
                className="button button--primary"
                data-acceptance="start-meeting"
                disabled={busyAction !== null}
                onClick={() => void startMeeting()}
                type="button"
              >
                {busyAction === "start" ? "Starting…" : "Start meeting"}
              </button>
            </>
          ) : null}

          {meeting.status === "recording" ? (
            <>
              <AppLink
                className="button button--primary"
                to={liveMeetingPath(meeting.id)}
              >
                Enter live meeting
              </AppLink>
              <button
                className="button button--secondary"
                disabled={busyAction !== null}
                onClick={() => void endMeeting()}
                type="button"
              >
                {busyAction === "end" ? "Ending…" : "End meeting"}
              </button>
            </>
          ) : null}

          {meeting.status === "ready" ? (
            <p className="supporting-copy">
              This synthetic meeting is complete and stored locally.
            </p>
          ) : null}

          {confirmingDelete && canDelete ? (
            <div
              className="confirm-panel"
              role="group"
              aria-label="Confirm meeting deletion"
            >
              <p>Delete this local meeting? This cannot be undone.</p>
              <button
                className="button button--danger"
                data-acceptance="confirm-delete"
                disabled={busyAction === "delete"}
                onClick={() => void deleteMeeting()}
                type="button"
              >
                {busyAction === "delete" ? "Deleting…" : "Confirm delete"}
              </button>
              <button
                className="button button--ghost"
                disabled={busyAction === "delete"}
                onClick={() => setConfirmingDelete(false)}
                type="button"
              >
                Cancel
              </button>
            </div>
          ) : canRetryDelete ? (
            <button
              className="button button--danger-outline"
              data-acceptance="retry-delete-meeting"
              disabled={busyAction !== null}
              onClick={() => void deleteMeeting()}
              type="button"
            >
              {busyAction === "delete" ? "Retrying…" : "Retry meeting deletion"}
            </button>
          ) : (
            <button
              className="button button--danger-outline"
              data-acceptance="delete-meeting"
              disabled={!canDelete || busyAction !== null}
              onClick={() => setConfirmingDelete(true)}
              type="button"
            >
              Delete meeting
            </button>
          )}

          {!canDelete && !canRetryDelete ? (
            <p className="restriction">
              {meeting.status === "deleting"
                ? "Meeting deletion is still pending · meeting_delete_pending"
                : "Finish the meeting before deleting it · meeting_invalid_state"}
            </p>
          ) : null}
          {meeting.status === "ready" ? (
            <button className="button button--ghost" disabled type="button">
              Start meeting
            </button>
          ) : null}
        </article>
      </section>
      <DocumentPanel meetingId={meeting.id} meetingStatus={meeting.status} />
    </div>
  );
}
