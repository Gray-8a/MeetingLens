import { useCallback, useEffect, useState } from "react";
import { meetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { DocumentPanel } from "../components/DocumentPanel";
import { StatusBadge } from "../components/StatusBadge";
import { ErrorNotice, InfoNotice, LoadingState } from "../components/UiState";
import { formatDuration, uiError } from "../lib/presentation";
import { toProviderError, type ProviderError } from "../providers/errors";
import type { TranscriptionSegment } from "../providers/contracts";
import type { DatabaseStatus } from "./HomePage";

interface LiveMeetingPageProps {
  meetingId: string;
  onDatabaseStatus: (status: DatabaseStatus) => void;
  onMeetingStatus: (status: string | null) => void;
}

export function LiveMeetingPage({
  meetingId,
  onDatabaseStatus,
  onMeetingStatus,
}: LiveMeetingPageProps) {
  const services = useServices();
  const { navigate } = useNavigation();
  const [meeting, setMeeting] = useState<MeetingResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ReturnType<typeof uiError> | null>(null);
  const [transcriptError, setTranscriptError] = useState<ProviderError | null>(
    null,
  );
  const [entries, setEntries] = useState<readonly TranscriptionSegment[]>([]);
  const [paused, setPaused] = useState(false);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [importantMarks, setImportantMarks] = useState(0);

  const loadMeeting = useCallback(async () => {
    setLoading(true);
    setError(null);
    onDatabaseStatus("checking");
    try {
      const result = await services.meetingApi.get(meetingId);
      setMeeting(result);
      onMeetingStatus(result.status);
      onDatabaseStatus("ready");
    } catch (reason: unknown) {
      const safeError = uiError(reason);
      setError(safeError);
      setMeeting(null);
      onMeetingStatus(null);
      onDatabaseStatus(
        safeError.code === "meeting_not_found" ? "ready" : "unavailable",
      );
    } finally {
      setLoading(false);
    }
  }, [meetingId, onDatabaseStatus, onMeetingStatus, services.meetingApi]);

  useEffect(() => {
    const timer = window.setTimeout(() => void loadMeeting(), 0);
    return () => {
      window.clearTimeout(timer);
      onMeetingStatus(null);
    };
  }, [loadMeeting, onMeetingStatus]);

  useEffect(() => {
    if (meeting?.status !== "recording") return;
    const provider = services.providers.get("transcription");
    let active = true;
    const currentSegments = new Map<string, TranscriptionSegment>();
    const unsubscribe = provider.subscribe((segment) => {
      if (!active) return;
      currentSegments.set(segment.segmentId, segment);
      setEntries([...currentSegments.values()]);
    });
    const startedAt = meeting.startedAt;

    if (startedAt === null) {
      queueMicrotask(() => {
        if (active) {
          setTranscriptError(toProviderError(null, "transcription", "start"));
        }
      });
    } else {
      void provider
        .start({
          meetingId: meeting.id,
          language: meeting.primaryLanguage,
          startedAt,
        })
        .catch((reason: unknown) => {
          if (active) {
            setTranscriptError(
              toProviderError(reason, "transcription", "start"),
            );
          }
        });
    }

    return () => {
      active = false;
      unsubscribe();
      if (
        provider.getStatus() === "running" ||
        provider.getStatus() === "paused"
      ) {
        void provider.stop();
      }
    };
  }, [
    meeting?.id,
    meeting?.primaryLanguage,
    meeting?.startedAt,
    meeting?.status,
    services.providers,
  ]);

  useEffect(() => {
    if (meeting?.status !== "recording" || meeting.startedAt === null) return;
    const startedAt = new Date(meeting.startedAt).getTime();
    const updateElapsed = () =>
      setElapsedMs(Math.max(0, Date.now() - startedAt));
    updateElapsed();
    const timer = window.setInterval(updateElapsed, 1_000);
    return () => window.clearInterval(timer);
  }, [meeting?.startedAt, meeting?.status]);

  async function startMeeting() {
    if (!meeting || busy) return;
    setBusy(true);
    setError(null);
    try {
      const recording = await services.meetingApi.start(meeting.id);
      setMeeting(recording);
      onMeetingStatus(recording.status);
    } catch (reason: unknown) {
      setError(uiError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function endMeeting() {
    if (!meeting || busy) return;
    setBusy(true);
    setError(null);
    try {
      const ready = await services.meetingApi.end(meeting.id);
      const provider = services.providers.get("transcription");
      if (
        provider.getStatus() === "running" ||
        provider.getStatus() === "paused"
      ) {
        await provider.stop();
      }
      setMeeting(ready);
      onMeetingStatus(ready.status);
      navigate(meetingPath(ready.id));
    } catch (reason: unknown) {
      setError(uiError(reason));
      setBusy(false);
    }
  }

  async function pauseTranscript() {
    try {
      await services.providers.get("transcription").pause();
      setPaused(true);
    } catch (reason: unknown) {
      setTranscriptError(toProviderError(reason, "transcription", "pause"));
    }
  }

  async function resumeTranscript() {
    try {
      await services.providers.get("transcription").resume();
      setPaused(false);
    } catch (reason: unknown) {
      setTranscriptError(toProviderError(reason, "transcription", "resume"));
    }
  }

  if (loading) return <LoadingState label="Opening the meeting…" />;

  if (meeting === null) {
    return (
      <section className="content-panel narrow-panel">
        {error ? (
          <ErrorNotice code={error.code} message={error.message} />
        ) : null}
        <button
          className="button button--secondary"
          onClick={() => navigate("/history")}
          type="button"
        >
          Back to history
        </button>
      </section>
    );
  }

  if (meeting.status === "draft") {
    return (
      <section className="content-panel narrow-panel">
        <p className="eyebrow">Meeting not started</p>
        <h1>{meeting.title}</h1>
        <InfoNotice>
          A draft cannot display the recording workspace until the backend start
          command succeeds.
        </InfoNotice>
        {error ? (
          <ErrorNotice code={error.code} message={error.message} />
        ) : null}
        <button
          className="button button--primary"
          disabled={busy}
          onClick={() => void startMeeting()}
          type="button"
        >
          {busy ? "Starting…" : "Start meeting"}
        </button>
      </section>
    );
  }

  if (meeting.status !== "recording") {
    return (
      <section className="content-panel narrow-panel">
        <StatusBadge status={meeting.status} />
        <h1>This meeting is no longer recording.</h1>
        <button
          className="button button--primary"
          onClick={() => navigate(meetingPath(meeting.id))}
          type="button"
        >
          View meeting details
        </button>
      </section>
    );
  }

  return (
    <div className="live-page" data-acceptance="live-meeting">
      {error ? <ErrorNotice code={error.code} message={error.message} /> : null}
      <section className="live-grid">
        <aside className="live-sidebar content-panel">
          <p className="eyebrow">Synthetic meeting</p>
          <h1>{meeting.title}</h1>
          <StatusBadge status={meeting.status} />
          <dl className="live-facts">
            <div>
              <dt>Elapsed</dt>
              <dd data-testid="meeting-duration">
                {formatDuration(elapsedMs)}
              </dd>
            </div>
            <div>
              <dt>Transcript</dt>
              <dd>{paused ? "Paused" : "Running"}</dd>
            </div>
            <div>
              <dt>Important marks</dt>
              <dd>{importantMarks}</dd>
            </div>
          </dl>
          <div>
            <h2>Demo participants</h2>
            <ul className="participant-list">
              <li>
                <span>A</span> Speaker A
              </li>
              <li>
                <span>B</span> Speaker B
              </li>
              <li>
                <span>C</span> Speaker C
              </li>
            </ul>
          </div>
          <p className="restriction">
            <span data-acceptance="recording-delete-restriction">
              Recording meetings cannot be deleted · meeting_invalid_state
            </span>
          </p>
        </aside>

        <section
          className="transcript-panel content-panel"
          aria-labelledby="transcript-title"
        >
          <div className="section-heading">
            <div>
              <p className="eyebrow">Simulation mode</p>
              <h2 id="transcript-title">Live transcript</h2>
            </div>
            <span className="simulation-badge">Synthetic data</span>
          </div>
          {transcriptError !== null ? (
            <ErrorNotice
              code={transcriptError.code}
              message={transcriptError.message}
            />
          ) : null}
          {!transcriptError && entries.length === 0 ? (
            <LoadingState label="Starting synthetic transcript…" />
          ) : null}
          <ol
            className="transcript-stream"
            data-testid="transcript-stream"
            aria-live="polite"
          >
            {entries.map((entry) => (
              <li
                className={`transcript-entry transcript-entry--${entry.isFinal ? "final" : "provisional"}`}
                key={entry.segmentId}
              >
                <div className="transcript-entry__meta">
                  <span>
                    {formatTranscriptOffset(entry, meeting.startedAt)}
                  </span>
                  <strong>{entry.speakerLabel}</strong>
                  <span>{entry.isFinal ? "final" : "provisional"}</span>
                </div>
                <p>{entry.text}</p>
              </li>
            ))}
          </ol>
        </section>

        <aside className="meeting-context content-panel">
          <p className="eyebrow">Temporary context</p>
          <label htmlFor="temporary-notes">Private temporary notes</label>
          <textarea
            id="temporary-notes"
            placeholder="Not saved to the database"
            rows={5}
          />
          <h2>Discussion topic</h2>
          <p>Validate the local desktop meeting experience.</p>
          <h2>Demo action item</h2>
          <p>Review the Provider security boundary in the next milestone.</p>
          <p className="field-help">This panel exists in memory only.</p>
        </aside>
      </section>

      <DocumentPanel
        meetingId={meeting.id}
        meetingStatus={meeting.status}
        variant="compact"
      />

      <div className="live-controls" aria-label="Meeting controls">
        {paused ? (
          <button
            className="button button--secondary"
            data-acceptance="resume-transcript"
            onClick={() => void resumeTranscript()}
            type="button"
          >
            Resume transcript
          </button>
        ) : (
          <button
            className="button button--secondary"
            data-acceptance="pause-transcript"
            onClick={() => void pauseTranscript()}
            type="button"
          >
            Pause transcript
          </button>
        )}
        <button
          className="button button--ghost"
          onClick={() => setImportantMarks((count) => count + 1)}
          type="button"
        >
          Add important mark
        </button>
        <button
          className="button button--danger"
          data-acceptance="end-meeting"
          disabled={busy}
          onClick={() => void endMeeting()}
          type="button"
        >
          {busy ? "Ending…" : "End meeting"}
        </button>
      </div>
    </div>
  );
}

function formatTranscriptOffset(
  segment: TranscriptionSegment,
  meetingStartedAt: string | null,
): string {
  if (meetingStartedAt === null) return formatDuration(0);
  const offset = Date.parse(segment.startedAt) - Date.parse(meetingStartedAt);
  return formatDuration(Number.isFinite(offset) ? Math.max(0, offset) : 0);
}
