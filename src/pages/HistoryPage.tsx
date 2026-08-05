import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppLink } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { MeetingList } from "../components/MeetingList";
import { EmptyState, ErrorNotice, LoadingState } from "../components/UiState";
import { meetingErrorMessage, uiError } from "../lib/presentation";
import { sortMeetings, type DatabaseStatus } from "./HomePage";

export function HistoryPage({
  onDatabaseStatus,
}: {
  onDatabaseStatus: (status: DatabaseStatus) => void;
}) {
  const { t } = useTranslation();
  const { meetingApi } = useServices();
  const [meetings, setMeetings] = useState<MeetingResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const loadMeetings = useCallback(async () => {
    setLoading(true);
    setError(null);
    onDatabaseStatus("checking");
    try {
      const result = await meetingApi.list();
      setMeetings(sortMeetings(result.meetings));
      onDatabaseStatus("ready");
    } catch (reason: unknown) {
      const safeError = uiError(reason);
      setError(safeError);
      if (
        safeError.code === "meeting_delete_failed" ||
        safeError.code === "meeting_delete_pending"
      ) {
        try {
          const result = await meetingApi.list();
          setMeetings(sortMeetings(result.meetings));
        } catch {
          // Keep the stable delete failure if refreshing the tombstone fails.
        }
      }
      onDatabaseStatus("unavailable");
    } finally {
      setLoading(false);
    }
  }, [meetingApi, onDatabaseStatus]);

  useEffect(() => {
    const timer = window.setTimeout(() => void loadMeetings(), 0);
    return () => window.clearTimeout(timer);
  }, [loadMeetings]);

  async function deleteMeeting(id: string) {
    setDeletingId(id);
    setError(null);
    try {
      await meetingApi.delete(id);
      setMeetings((current) => current.filter((meeting) => meeting.id !== id));
      setConfirmingId(null);
    } catch (reason: unknown) {
      setError(uiError(reason));
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <section className="content-panel" aria-labelledby="history-title">
      <div className="section-heading">
        <div>
          <p className="eyebrow">{t("history.eyebrow")}</p>
          <h1 id="history-title">{t("history.title")}</h1>
          <p>{t("history.description")}</p>
        </div>
        <AppLink className="button button--primary" to="/new">
          {t("history.newMeeting")}
        </AppLink>
      </div>

      {error ? (
        <ErrorNotice
          code={error.code}
          message={meetingErrorMessage(error.code)}
        />
      ) : null}
      {loading ? <LoadingState label={t("history.loading")} /> : null}
      {!loading && meetings.length === 0 ? (
        <EmptyState title={t("history.emptyTitle")}>
          <p>{t("history.emptyBody")}</p>
        </EmptyState>
      ) : null}
      {!loading && meetings.length > 0 ? (
        <MeetingList
          confirmingId={confirmingId}
          deletingId={deletingId}
          meetings={meetings}
          onCancelDelete={() => setConfirmingId(null)}
          onConfirmDelete={(id) => void deleteMeeting(id)}
          onRequestDelete={setConfirmingId}
          onRetryDelete={(id) => void deleteMeeting(id)}
        />
      ) : null}
    </section>
  );
}
