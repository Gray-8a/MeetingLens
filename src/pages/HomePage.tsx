import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppLink } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { MeetingList } from "../components/MeetingList";
import { EmptyState, ErrorNotice, LoadingState } from "../components/UiState";
import { meetingErrorMessage, uiError } from "../lib/presentation";
import { formatNumber } from "../i18n/formatters";

export type DatabaseStatus = "checking" | "ready" | "unavailable";

export function HomePage({
  onDatabaseStatus,
}: {
  onDatabaseStatus: (status: DatabaseStatus) => void;
}) {
  const { t } = useTranslation();
  const { meetingApi } = useServices();
  const [meetings, setMeetings] = useState<MeetingResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);

  useEffect(() => {
    let active = true;
    void meetingApi
      .list()
      .then(({ meetings: loadedMeetings }) => {
        if (!active) return;
        setMeetings(sortMeetings(loadedMeetings));
        onDatabaseStatus("ready");
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setError(uiError(reason));
        onDatabaseStatus("unavailable");
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [meetingApi, onDatabaseStatus]);

  const counts = useMemo(
    () => ({
      draft: meetings.filter((meeting) => meeting.status === "draft").length,
      recording: meetings.filter((meeting) => meeting.status === "recording")
        .length,
      ready: meetings.filter((meeting) => meeting.status === "ready").length,
    }),
    [meetings],
  );

  return (
    <div className="page-stack">
      <section className="welcome-panel">
        <div>
          <p className="eyebrow">{t("home.eyebrow")}</p>
          <h1>{t("home.title")}</h1>
          <p>{t("home.description")}</p>
        </div>
        <AppLink className="button button--primary button--large" to="/new">
          {t("home.newMeeting")}
        </AppLink>
      </section>

      <section className="metric-grid" aria-label={t("home.statusSummary")}>
        <article className="metric-card">
          <span>{t("home.metrics.draftLabel")}</span>
          <strong>{formatNumber(counts.draft)}</strong>
          <p>{t("home.metrics.draftBody")}</p>
        </article>
        <article className="metric-card">
          <span>{t("home.metrics.recordingLabel")}</span>
          <strong>{formatNumber(counts.recording)}</strong>
          <p>{t("home.metrics.recordingBody")}</p>
        </article>
        <article className="metric-card">
          <span>{t("home.metrics.readyLabel")}</span>
          <strong>{formatNumber(counts.ready)}</strong>
          <p>{t("home.metrics.readyBody")}</p>
        </article>
      </section>

      <section className="content-panel" aria-labelledby="recent-title">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t("home.recentEyebrow")}</p>
            <h2 id="recent-title">{t("home.recentTitle")}</h2>
          </div>
          <AppLink className="text-link" to="/history">
            {t("home.viewAll")}
          </AppLink>
        </div>

        {loading ? <LoadingState label={t("home.loading")} /> : null}
        {!loading && error ? (
          <ErrorNotice
            code={error.code}
            message={meetingErrorMessage(error.code)}
          />
        ) : null}
        {!loading && !error && meetings.length === 0 ? (
          <EmptyState title={t("home.emptyTitle")}>
            <p>{t("home.emptyBody")}</p>
            <AppLink className="button button--primary" to="/new">
              {t("home.createFirst")}
            </AppLink>
          </EmptyState>
        ) : null}
        {!loading && !error && meetings.length > 0 ? (
          <MeetingList meetings={meetings.slice(0, 5)} />
        ) : null}
      </section>
    </div>
  );
}

export function sortMeetings(meetings: readonly MeetingResponse[]) {
  return [...meetings].sort((left, right) =>
    right.createdAt.localeCompare(left.createdAt),
  );
}
