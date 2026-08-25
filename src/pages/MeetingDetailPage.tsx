import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { AppLink, liveMeetingPath, useNavigation } from "../app/navigation";
import { summaryMinutesPath } from "../app/paths";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { DocumentPanel } from "../components/DocumentPanel";
import { MeetingTranscriptPanel } from "../components/MeetingTranscriptPanel";
import { StatusBadge } from "../components/StatusBadge";
import { ErrorNotice, LoadingState } from "../components/UiState";
import {
  formatLocalDate,
  meetingErrorMessage,
  uiError,
} from "../lib/presentation";
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
  const { t } = useTranslation();
  const { meetingApi } = useServices();
  const { navigate } = useNavigation();
  const [meeting, setMeeting] = useState<MeetingResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");
  const [titleMissing, setTitleMissing] = useState(false);
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
      setTitleMissing(true);
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
      setTitleMissing(false);
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

  if (loading) return <LoadingState label={t("meetingDetail.loading")} />;

  if (meeting === null) {
    return (
      <section className="content-panel narrow-panel">
        {error ? (
          <ErrorNotice
            code={error.code}
            message={meetingErrorMessage(error.code)}
          />
        ) : null}
        <AppLink className="button button--secondary" to="/history">
          {t("meetingDetail.backToHistory")}
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
          <p className="eyebrow">{t("meetingDetail.eyebrow")}</p>
          <div className="title-with-status">
            <h1>{meeting.title}</h1>
            <StatusBadge status={meeting.status} />
          </div>
          <p className="meeting-id">
            {t("meetingDetail.meetingId", { id: meeting.id })}
          </p>
        </div>
        <AppLink className="text-link" to="/history">
          {t("meetingDetail.backToHistory")}
        </AppLink>
      </section>

      {error ? (
        <ErrorNotice
          code={error.code}
          message={meetingErrorMessage(error.code)}
        />
      ) : null}

      <section className="detail-grid">
        <article className="content-panel">
          <p className="eyebrow">{t("meetingDetail.timeline")}</p>
          <dl className="detail-list">
            <div>
              <dt>{t("meetingDetail.created")}</dt>
              <dd>{formatLocalDate(meeting.createdAt)}</dd>
            </div>
            <div>
              <dt>{t("meetingDetail.started")}</dt>
              <dd>{formatLocalDate(meeting.startedAt)}</dd>
            </div>
            <div>
              <dt>{t("meetingDetail.ended")}</dt>
              <dd>{formatLocalDate(meeting.endedAt)}</dd>
            </div>
            <div>
              <dt>{t("meetingDetail.languageBaseline")}</dt>
              <dd>{meeting.primaryLanguage}</dd>
            </div>
          </dl>
        </article>

        <article className="content-panel">
          <p className="eyebrow">{t("meetingDetail.actions")}</p>
          {meeting.status === "draft" ? (
            <>
              {editing ? (
                <form className="inline-form" onSubmit={saveTitle}>
                  <label htmlFor="edit-meeting-title">
                    {t("meetingDetail.meetingTitle")}
                  </label>
                  <input
                    data-acceptance="edit-title-input"
                    disabled={busyAction === "update"}
                    id="edit-meeting-title"
                    maxLength={200}
                    onChange={(event) => {
                      setTitle(event.target.value);
                      setTitleMissing(false);
                    }}
                    value={title}
                  />
                  {titleMissing ? (
                    <p className="field-error">
                      {t("newMeeting.titleRequired")}
                    </p>
                  ) : null}
                  <div className="form-actions">
                    <button
                      className="button button--primary"
                      data-acceptance="save-title"
                      disabled={busyAction === "update"}
                      type="submit"
                    >
                      {busyAction === "update"
                        ? t("common.saving")
                        : t("meetingDetail.saveTitle")}
                    </button>
                    <button
                      className="button button--ghost"
                      onClick={() => {
                        setEditing(false);
                        setTitle(meeting.title);
                        setTitleMissing(false);
                      }}
                      type="button"
                    >
                      {t("common.cancel")}
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
                  {t("meetingDetail.editTitle")}
                </button>
              )}
              <button
                className="button button--primary"
                data-acceptance="start-meeting"
                disabled={busyAction !== null}
                onClick={() => void startMeeting()}
                type="button"
              >
                {busyAction === "start"
                  ? t("common.starting")
                  : t("meetingDetail.startMeeting")}
              </button>
            </>
          ) : null}

          {meeting.status === "recording" ? (
            <>
              <AppLink
                className="button button--primary"
                to={liveMeetingPath(meeting.id)}
              >
                {t("meetingDetail.enterLive")}
              </AppLink>
              <button
                className="button button--secondary"
                disabled={busyAction !== null}
                onClick={() => void endMeeting()}
                type="button"
              >
                {busyAction === "end"
                  ? t("common.ending")
                  : t("meetingDetail.endMeeting")}
              </button>
            </>
          ) : null}

          {meeting.status === "ready" ? (
            <p className="supporting-copy">{t("meetingDetail.readyBody")}</p>
          ) : null}

          {confirmingDelete && canDelete ? (
            <div
              className="confirm-panel"
              role="group"
              aria-label={t("meetingDetail.confirmDeleteAria")}
            >
              <p>{t("meetingDetail.confirmDeleteBody")}</p>
              <button
                className="button button--danger"
                data-acceptance="confirm-delete"
                disabled={busyAction === "delete"}
                onClick={() => void deleteMeeting()}
                type="button"
              >
                {busyAction === "delete"
                  ? t("common.deleting")
                  : t("common.confirmDelete")}
              </button>
              <button
                className="button button--ghost"
                disabled={busyAction === "delete"}
                onClick={() => setConfirmingDelete(false)}
                type="button"
              >
                {t("common.cancel")}
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
              {busyAction === "delete"
                ? t("common.retrying")
                : t("meetingDetail.retryMeetingDeletion")}
            </button>
          ) : (
            <button
              className="button button--danger-outline"
              data-acceptance="delete-meeting"
              disabled={!canDelete || busyAction !== null}
              onClick={() => setConfirmingDelete(true)}
              type="button"
            >
              {t("meetingDetail.deleteMeeting")}
            </button>
          )}

          {!canDelete && !canRetryDelete ? (
            <p className="restriction">
              {meeting.status === "deleting"
                ? t("meetingDetail.deletionPending")
                : t("meetingDetail.finishBeforeDelete")}
            </p>
          ) : null}
          {meeting.status === "draft" ||
          meeting.status === "recording" ||
          meeting.status === "ready" ? (
            <AppLink
              className="button button--primary"
              to={summaryMinutesPath(meeting.id)}
            >
              {t("meetingDetail.openMinutes")}
            </AppLink>
          ) : null}
        </article>
      </section>
      <MeetingTranscriptPanel
        meetingId={meeting.id}
        meetingStatus={meeting.status}
      />
      <DocumentPanel meetingId={meeting.id} meetingStatus={meeting.status} />
    </div>
  );
}
