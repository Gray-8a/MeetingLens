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
          <p className="eyebrow">{t("modes.eyebrow")}</p>
          <h1>{t("modes.title")}</h1>
          <p>{t("modes.description")}</p>
        </div>
        <AppLink
          className="button button--primary button--large"
          data-acceptance="open-summary-mode"
          to="/summary"
        >
          {t("modes.summaryAction")}
        </AppLink>
      </section>

      <section className="mode-grid" aria-label={t("modes.title")}>
        <article className="mode-card mode-card--available">
          <div className="mode-card__header">
            <span className="mode-card__mark" aria-hidden="true">
              SM
            </span>
            <span className="mode-card__status">
              {t("modes.summaryAvailable")}
            </span>
          </div>
          <h2>{t("modes.summaryTitle")}</h2>
          <p>{t("modes.summaryBody")}</p>
          <AppLink
            className="button button--primary"
            data-acceptance="summary-mode-card-action"
            to="/summary"
          >
            {t("modes.summaryAction")}
          </AppLink>
        </article>

        <article className="mode-card mode-card--coming-soon">
          <div className="mode-card__header">
            <span className="mode-card__mark" aria-hidden="true">
              TR
            </span>
            <span className="mode-card__status">{t("modes.comingSoon")}</span>
          </div>
          <h2>{t("modes.translationTitle")}</h2>
          <p>{t("modes.translationBody")}</p>
          <button className="button button--secondary" disabled type="button">
            {t("modes.comingSoon")}
          </button>
        </article>

        <article className="mode-card mode-card--coming-soon">
          <div className="mode-card__header">
            <span className="mode-card__mark" aria-hidden="true">
              MT
            </span>
            <span className="mode-card__status">{t("modes.comingSoon")}</span>
          </div>
          <h2>{t("modes.meetingTitle")}</h2>
          <p>{t("modes.meetingBody")}</p>
          <button className="button button--secondary" disabled type="button">
            {t("modes.comingSoon")}
          </button>
        </article>
      </section>

      <section className="advanced-entry">
        <div>
          <h2>{t("modes.advancedTitle")}</h2>
          <p>{t("modes.advancedBody")}</p>
        </div>
        <AppLink className="button button--ghost" to="/new">
          {t("modes.advancedAction")}
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
