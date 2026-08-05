import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { meetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import { DocumentPanel } from "../components/DocumentPanel";
import { StatusBadge } from "../components/StatusBadge";
import { ErrorNotice, InfoNotice, LoadingState } from "../components/UiState";
import { providerErrorMessage } from "../i18n/errors";
import { formatNumber } from "../i18n/formatters";
import {
  formatDuration,
  meetingErrorMessage,
  uiError,
} from "../lib/presentation";
import type { TranscriptionSegment } from "../providers/contracts";
import { toProviderError, type ProviderError } from "../providers/errors";
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
  const { t } = useTranslation();
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

  if (loading) return <LoadingState label={t("live.opening")} />;

  if (meeting === null) {
    return (
      <section className="content-panel narrow-panel">
        {error ? (
          <ErrorNotice
            code={error.code}
            message={meetingErrorMessage(error.code)}
          />
        ) : null}
        <button
          className="button button--secondary"
          onClick={() => navigate("/history")}
          type="button"
        >
          {t("live.backToHistory")}
        </button>
      </section>
    );
  }

  if (meeting.status === "draft") {
    return (
      <section className="content-panel narrow-panel">
        <p className="eyebrow">{t("live.notStartedEyebrow")}</p>
        <h1>{meeting.title}</h1>
        <InfoNotice>{t("live.draftNotice")}</InfoNotice>
        {error ? (
          <ErrorNotice
            code={error.code}
            message={meetingErrorMessage(error.code)}
          />
        ) : null}
        <button
          className="button button--primary"
          disabled={busy}
          onClick={() => void startMeeting()}
          type="button"
        >
          {busy ? t("common.starting") : t("meetingDetail.startMeeting")}
        </button>
      </section>
    );
  }

  if (meeting.status !== "recording") {
    return (
      <section className="content-panel narrow-panel">
        <StatusBadge status={meeting.status} />
        <h1>{t("live.noLongerRecording")}</h1>
        <button
          className="button button--primary"
          onClick={() => navigate(meetingPath(meeting.id))}
          type="button"
        >
          {t("live.viewDetails")}
        </button>
      </section>
    );
  }

  return (
    <div className="live-page" data-acceptance="live-meeting">
      {error ? (
        <ErrorNotice
          code={error.code}
          message={meetingErrorMessage(error.code)}
        />
      ) : null}
      <section className="live-grid">
        <aside className="live-sidebar content-panel">
          <p className="eyebrow">{t("live.syntheticMeeting")}</p>
          <h1>{meeting.title}</h1>
          <StatusBadge status={meeting.status} />
          <dl className="live-facts">
            <div>
              <dt>{t("live.elapsed")}</dt>
              <dd data-testid="meeting-duration">
                {formatDuration(elapsedMs)}
              </dd>
            </div>
            <div>
              <dt>{t("live.transcript")}</dt>
              <dd>{paused ? t("live.paused") : t("live.running")}</dd>
            </div>
            <div>
              <dt>{t("live.importantMarks")}</dt>
              <dd>{formatNumber(importantMarks)}</dd>
            </div>
          </dl>
          <div>
            <h2>{t("live.demoParticipants")}</h2>
            <ul className="participant-list">
              <li>
                <span>A</span> {t("live.speakerA")}
              </li>
              <li>
                <span>B</span> {t("live.speakerB")}
              </li>
              <li>
                <span>C</span> {t("live.speakerC")}
              </li>
            </ul>
          </div>
          <p className="restriction">
            <span data-acceptance="recording-delete-restriction">
              {t("live.recordingDeleteRestriction")}
            </span>
          </p>
        </aside>

        <section
          className="transcript-panel content-panel"
          aria-labelledby="transcript-title"
        >
          <div className="section-heading">
            <div>
              <p className="eyebrow">{t("live.simulationMode")}</p>
              <h2 id="transcript-title">{t("live.liveTranscript")}</h2>
            </div>
            <span className="simulation-badge">{t("live.syntheticData")}</span>
          </div>
          {transcriptError !== null ? (
            <ErrorNotice
              code={transcriptError.code}
              message={providerErrorMessage(transcriptError.code)}
            />
          ) : null}
          {!transcriptError && entries.length === 0 ? (
            <LoadingState label={t("live.startingTranscript")} />
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
                  <span>
                    {entry.isFinal ? t("live.final") : t("live.provisional")}
                  </span>
                </div>
                <p>{entry.text}</p>
              </li>
            ))}
          </ol>
        </section>

        <aside className="meeting-context content-panel">
          <p className="eyebrow">{t("live.temporaryContext")}</p>
          <label htmlFor="temporary-notes">{t("live.privateNotes")}</label>
          <textarea
            id="temporary-notes"
            placeholder={t("live.privateNotesPlaceholder")}
            rows={5}
          />
          <h2>{t("live.discussionTopic")}</h2>
          <p>{t("live.discussionBody")}</p>
          <h2>{t("live.demoAction")}</h2>
          <p>{t("live.demoActionBody")}</p>
          <p className="field-help">{t("live.memoryOnly")}</p>
        </aside>
      </section>

      <DocumentPanel
        meetingId={meeting.id}
        meetingStatus={meeting.status}
        variant="compact"
      />

      <div className="live-controls" aria-label={t("live.controls")}>
        {paused ? (
          <button
            className="button button--secondary"
            data-acceptance="resume-transcript"
            onClick={() => void resumeTranscript()}
            type="button"
          >
            {t("live.resumeTranscript")}
          </button>
        ) : (
          <button
            className="button button--secondary"
            data-acceptance="pause-transcript"
            onClick={() => void pauseTranscript()}
            type="button"
          >
            {t("live.pauseTranscript")}
          </button>
        )}
        <button
          className="button button--ghost"
          onClick={() => setImportantMarks((count) => count + 1)}
          type="button"
        >
          {t("live.addImportantMark")}
        </button>
        <button
          className="button button--danger"
          data-acceptance="end-meeting"
          disabled={busy}
          onClick={() => void endMeeting()}
          type="button"
        >
          {busy ? t("common.ending") : t("live.endMeeting")}
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
