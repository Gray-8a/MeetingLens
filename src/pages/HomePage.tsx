import { useEffect, useMemo, useState } from "react";
import { AppLink } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { MeetingList } from "../components/MeetingList";
import { EmptyState, ErrorNotice, LoadingState } from "../components/UiState";
import { uiError } from "../lib/presentation";

export type DatabaseStatus = "checking" | "ready" | "unavailable";

export function HomePage({
  onDatabaseStatus,
}: {
  onDatabaseStatus: (status: DatabaseStatus) => void;
}) {
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
          <p className="eyebrow">Local meeting workspace</p>
          <h1>Turn the next conversation into clear action.</h1>
          <p>
            Start with a synthetic meeting flow. Nothing is recorded, uploaded,
            or sent to an external model in this milestone.
          </p>
        </div>
        <AppLink className="button button--primary button--large" to="/new">
          New meeting
        </AppLink>
      </section>

      <section className="metric-grid" aria-label="Meeting status summary">
        <article className="metric-card">
          <span>Draft</span>
          <strong>{counts.draft}</strong>
          <p>Ready to start</p>
        </article>
        <article className="metric-card">
          <span>Recording</span>
          <strong>{counts.recording}</strong>
          <p>Synthetic flow active</p>
        </article>
        <article className="metric-card">
          <span>Ready</span>
          <strong>{counts.ready}</strong>
          <p>Completed locally</p>
        </article>
      </section>

      <section className="content-panel" aria-labelledby="recent-title">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Recent activity</p>
            <h2 id="recent-title">Recent meetings</h2>
          </div>
          <AppLink className="text-link" to="/history">
            View all meetings
          </AppLink>
        </div>

        {loading ? <LoadingState label="Loading local meetings…" /> : null}
        {!loading && error ? (
          <ErrorNotice code={error.code} message={error.message} />
        ) : null}
        {!loading && !error && meetings.length === 0 ? (
          <EmptyState title="No meetings yet">
            <p>Create a draft meeting to test the local desktop workflow.</p>
            <AppLink className="button button--primary" to="/new">
              Create the first meeting
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
