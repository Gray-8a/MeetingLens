import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { meetingPath, useNavigation } from "../app/navigation";
import { useServices } from "../app/services";
import {
  TranscriptApiError,
  type TranscriptApi,
  type TranscriptErrorCode,
  type TranscriptSegmentResponse,
  type TranscriptSourceResponse,
} from "../api/transcript";
import type { MeetingResponse } from "../api/meeting";
import { SpeakerRenamePanel } from "../components/SpeakerRenamePanel";
import { TranscriptInputPanel } from "../components/TranscriptInputPanel";
import { TranscriptSegmentPanel } from "../components/TranscriptSegmentPanel";
import { TranscriptSourceList } from "../components/TranscriptSourceList";
import { ErrorNotice, LoadingState } from "../components/UiState";
import { transcriptErrorMessage } from "../i18n/errors";
import { formatLocalDate, formatNumber } from "../i18n/formatters";
import {
  SPEAKER_GROUP_PAGE_SIZE,
  TRANSCRIPT_PAGE_SIZE,
  transcriptErrorCode,
} from "../lib/transcriptPresentation";
import { meetingErrorMessage, uiError } from "../lib/presentation";
import type { DatabaseStatus } from "./HomePage";

type SegmentPage = Awaited<ReturnType<TranscriptApi["listSegments"]>>;
type SpeakerPage = Awaited<ReturnType<TranscriptApi["listSpeakerGroups"]>>;
type SpeakerGroup = SpeakerPage["items"][number];

const emptySegmentPage: SegmentPage = {
  items: [],
  nextCursor: null,
  hasMore: false,
  totalCount: 0,
};

const emptySpeakerPage: SpeakerPage = {
  sourceId: "",
  items: [],
  nextCursor: null,
  hasMore: false,
  totalCount: 0,
};

export function TranscriptWorkspacePage({
  meetingId,
  onDatabaseStatus,
  onMeetingStatus,
}: {
  meetingId: string;
  onDatabaseStatus: (status: DatabaseStatus) => void;
  onMeetingStatus: (status: string | null) => void;
}) {
  const { t } = useTranslation();
  const { meetingApi, transcriptApi } = useServices();
  const { navigate } = useNavigation();
  const [meeting, setMeeting] = useState<MeetingResponse | null>(null);
  const [sources, setSources] = useState<TranscriptSourceResponse[]>([]);
  const [selectedSourceId, setSelectedSourceId] = useState<string | null>(null);
  const [maxSegmentBytes, setMaxSegmentBytes] = useState(64 * 1024);
  const [loading, setLoading] = useState(true);
  const [fatalError, setFatalError] = useState<ReturnType<
    typeof uiError
  > | null>(null);
  const [operationError, setOperationError] =
    useState<TranscriptErrorCode | null>(null);
  const [deletingSourceId, setDeletingSourceId] = useState<string | null>(null);
  const [segmentPage, setSegmentPage] = useState<SegmentPage>(emptySegmentPage);
  const [segmentLoading, setSegmentLoading] = useState(false);
  const [segmentCursors, setSegmentCursors] = useState<(number | null)[]>([
    null,
  ]);
  const [speakerPage, setSpeakerPage] = useState<SpeakerPage>(emptySpeakerPage);
  const [speakerLoading, setSpeakerLoading] = useState(false);
  const [mutationBusy, setMutationBusy] = useState(false);
  const [speakerCursors, setSpeakerCursors] = useState<(string | null)[]>([
    null,
  ]);
  const segmentRequestGeneration = useRef(0);
  const speakerRequestGeneration = useRef(0);
  const mutationBusyRef = useRef(false);
  const selectedSourceIdRef = useRef<string | null>(null);
  const segmentCursorRef = useRef<number | null>(null);
  const speakerCursorRef = useRef<string | null>(null);

  useEffect(() => {
    selectedSourceIdRef.current = selectedSourceId;
    segmentCursorRef.current =
      segmentCursors[segmentCursors.length - 1] ?? null;
    speakerCursorRef.current =
      speakerCursors[speakerCursors.length - 1] ?? null;
  }, [segmentCursors, selectedSourceId, speakerCursors]);

  const selectedSource = useMemo(
    () => sources.find((source) => source.id === selectedSourceId) ?? null,
    [selectedSourceId, sources],
  );

  const resetSourceReview = useCallback(() => {
    segmentRequestGeneration.current += 1;
    speakerRequestGeneration.current += 1;
    setSegmentCursors([null]);
    setSpeakerCursors([null]);
    setSegmentPage(emptySegmentPage);
    setSpeakerPage(emptySpeakerPage);
    setSegmentLoading(false);
    setSpeakerLoading(false);
  }, []);

  const refreshSources = useCallback(
    async (preferredSourceId?: string) => {
      const result = await transcriptApi.listSources(meetingId);
      setSources(result.sources);
      const preferred = preferredSourceId ?? selectedSourceId;
      const nextSourceId =
        preferred && result.sources.some((source) => source.id === preferred)
          ? preferred
          : (result.sources[result.sources.length - 1]?.id ?? null);
      if (nextSourceId !== selectedSourceId) {
        resetSourceReview();
        setSelectedSourceId(nextSourceId);
      }
    },
    [meetingId, resetSourceReview, selectedSourceId, transcriptApi],
  );

  useEffect(() => {
    let active = true;
    queueMicrotask(() => {
      if (!active) return;
      setLoading(true);
      setFatalError(null);
      setMeeting(null);
      onDatabaseStatus("checking");
      void Promise.all([
        meetingApi.get(meetingId),
        transcriptApi.listSources(meetingId),
        transcriptApi.getImportPolicy(),
      ])
        .then(([loadedMeeting, sourceResult, policy]) => {
          if (!active) return;
          resetSourceReview();
          setMeeting(loadedMeeting);
          setSources(sourceResult.sources);
          setSelectedSourceId(
            sourceResult.sources[sourceResult.sources.length - 1]?.id ?? null,
          );
          setMaxSegmentBytes(policy.maxSegmentBytes);
          onMeetingStatus(loadedMeeting.status);
          onDatabaseStatus("ready");
        })
        .catch((reason: unknown) => {
          if (!active) return;
          const safeError = uiError(reason);
          setFatalError(safeError);
          onMeetingStatus(null);
          onDatabaseStatus(
            safeError.code === "meeting_not_found" ? "ready" : "unavailable",
          );
        })
        .finally(() => {
          if (active) setLoading(false);
        });
    });
    return () => {
      active = false;
      onMeetingStatus(null);
    };
  }, [
    meetingApi,
    meetingId,
    onDatabaseStatus,
    onMeetingStatus,
    resetSourceReview,
    transcriptApi,
  ]);

  const loadSegments = useCallback(async () => {
    const generation = ++segmentRequestGeneration.current;
    if (!selectedSourceId) {
      setSegmentPage(emptySegmentPage);
      setSegmentLoading(false);
      return;
    }
    setSegmentLoading(true);
    setOperationError(null);
    try {
      const result = await transcriptApi.listSegments(
        meetingId,
        selectedSourceId,
        {
          cursor: segmentCursors[segmentCursors.length - 1] ?? null,
          limit: TRANSCRIPT_PAGE_SIZE,
        },
      );
      if (generation !== segmentRequestGeneration.current) return;
      setSegmentPage(result);
    } catch (reason: unknown) {
      if (generation !== segmentRequestGeneration.current) return;
      setOperationError(transcriptErrorCode(reason));
      setSegmentPage(emptySegmentPage);
    } finally {
      if (generation === segmentRequestGeneration.current) {
        setSegmentLoading(false);
      }
    }
  }, [meetingId, segmentCursors, selectedSourceId, transcriptApi]);

  const loadSpeakerGroups = useCallback(async () => {
    const generation = ++speakerRequestGeneration.current;
    if (!selectedSourceId) {
      setSpeakerPage(emptySpeakerPage);
      setSpeakerLoading(false);
      return;
    }
    setSpeakerLoading(true);
    setOperationError(null);
    try {
      const result = await transcriptApi.listSpeakerGroups(
        meetingId,
        selectedSourceId,
        {
          cursor: speakerCursors[speakerCursors.length - 1] ?? null,
          limit: SPEAKER_GROUP_PAGE_SIZE,
        },
      );
      if (generation !== speakerRequestGeneration.current) return;
      setSpeakerPage(result);
    } catch (reason: unknown) {
      if (generation !== speakerRequestGeneration.current) return;
      setOperationError(transcriptErrorCode(reason));
      setSpeakerPage(emptySpeakerPage);
    } finally {
      if (generation === speakerRequestGeneration.current) {
        setSpeakerLoading(false);
      }
    }
  }, [meetingId, selectedSourceId, speakerCursors, transcriptApi]);

  useEffect(() => {
    void Promise.resolve().then(loadSegments);
  }, [loadSegments]);

  useEffect(() => {
    void Promise.resolve().then(loadSpeakerGroups);
  }, [loadSpeakerGroups]);

  function beginMutation() {
    if (mutationBusyRef.current) {
      throw new TranscriptApiError("transcript_invalid_state", false);
    }
    mutationBusyRef.current = true;
    setMutationBusy(true);
  }

  function endMutation() {
    mutationBusyRef.current = false;
    setMutationBusy(false);
  }

  async function deleteSource(source: TranscriptSourceResponse) {
    beginMutation();
    setDeletingSourceId(source.id);
    setOperationError(null);
    try {
      await transcriptApi.deleteSource(meetingId, source.id);
      await refreshSources();
    } catch (reason: unknown) {
      setOperationError(transcriptErrorCode(reason));
      throw reason;
    } finally {
      setDeletingSourceId(null);
      endMutation();
    }
  }

  async function updateSegment(
    segment: TranscriptSegmentResponse,
    changes: { speakerName: string | null; originalText: string },
  ) {
    const sourceId = selectedSourceIdRef.current;
    const cursor = segmentCursorRef.current;
    const generation = segmentRequestGeneration.current;
    if (!sourceId || segment.sourceId !== sourceId) {
      throw new TranscriptApiError("transcript_invalid_input", false);
    }
    beginMutation();
    try {
      const updated = await transcriptApi.updateSegment({
        meetingId,
        segmentId: segment.id,
        expectedRevision: segment.revision,
        speakerName: changes.speakerName,
        originalText: changes.originalText,
      });
      if (
        selectedSourceIdRef.current !== sourceId ||
        segmentCursorRef.current !== cursor ||
        segmentRequestGeneration.current !== generation
      ) {
        return;
      }
      setSegmentPage((current) => ({
        ...current,
        items: current.items.map((item) =>
          item.id === updated.id ? updated : item,
        ),
      }));
      await loadSpeakerGroups();
    } finally {
      endMutation();
    }
  }

  async function renameSpeaker(
    group: SpeakerGroup,
    speakerName: string | null,
  ) {
    const sourceId = selectedSourceIdRef.current;
    const cursor = speakerCursorRef.current;
    const generation = speakerRequestGeneration.current;
    if (!sourceId || speakerPage.sourceId !== sourceId) {
      throw new TranscriptApiError("transcript_invalid_input", false);
    }
    beginMutation();
    try {
      await transcriptApi.renameSpeakerInSource({
        meetingId,
        sourceId,
        speakerLabel: group.speakerLabel,
        speakerName,
      });
      if (
        selectedSourceIdRef.current !== sourceId ||
        speakerCursorRef.current !== cursor ||
        speakerRequestGeneration.current !== generation
      ) {
        return;
      }
      await Promise.all([
        loadSegments(),
        loadSpeakerGroups(),
        refreshSources(),
      ]);
    } finally {
      endMutation();
    }
  }

  if (loading) return <LoadingState label={t("transcriptWorkspace.loading")} />;

  if (meeting === null) {
    return (
      <section className="content-panel narrow-panel">
        {fatalError ? (
          <ErrorNotice
            code={fatalError.code}
            message={meetingErrorMessage(fatalError.code)}
          />
        ) : null}
        <button
          className="button button--secondary"
          onClick={() => navigate("/history")}
          type="button"
        >
          {t("meetingDetail.backToHistory")}
        </button>
      </section>
    );
  }

  const readiness =
    sources.length === 0
      ? "empty"
      : sources.some((source) => source.segmentCount === 0)
        ? "partial"
        : "ready";
  const inputAllowed =
    meeting.status === "draft" ||
    meeting.status === "recording" ||
    meeting.status === "ready";

  return (
    <div className="page-stack" data-acceptance="transcript-workspace">
      <header className="detail-hero transcript-workspace-hero">
        <div>
          <p className="eyebrow">{t("transcriptWorkspace.sourcesTitle")}</p>
          <h1>{meeting.title}</h1>
          <p>
            {t("meetingDetail.created")} · {formatLocalDate(meeting.createdAt)}
          </p>
        </div>
        <button
          className="text-link"
          data-acceptance="back-to-meeting-details"
          onClick={() => navigate(meetingPath(meeting.id))}
          type="button"
        >
          {t("transcriptWorkspace.backToMeeting")}
        </button>
      </header>

      {operationError ? (
        <ErrorNotice
          code={operationError}
          message={transcriptErrorMessage(operationError)}
        />
      ) : null}

      <section className={`readiness-card readiness-card--${readiness}`}>
        <div>
          <span className="readiness-card__dot" aria-hidden="true" />
          <strong>{t(`transcriptWorkspace.readiness.${readiness}`)}</strong>
        </div>
        <button className="button button--primary" disabled type="button">
          {t("transcriptWorkspace.summaryComingSoon")}
        </button>
      </section>

      <section className="content-panel transcript-add-panel">
        {inputAllowed ? (
          <TranscriptInputPanel
            interactionDisabled={mutationBusy}
            onCompleted={(result) => {
              if (!result.cancelled) void refreshSources(result.source.id);
            }}
            target={{ kind: "existing", meetingId }}
          />
        ) : (
          <p className="restriction">
            {t("transcriptWorkspace.inputRestricted")}
          </p>
        )}
      </section>

      <section className="content-panel transcript-sources-panel">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t("transcriptWorkspace.sourcesTitle")}</p>
            <h2>{t("transcriptWorkspace.sourcesTitle")}</h2>
          </div>
          <span>
            {t("transcriptWorkspace.sourceCount", {
              count: sources.length,
              formattedCount: formatNumber(sources.length),
            })}
          </span>
        </div>
        {sources.length === 0 ? (
          <div className="transcript-empty-copy">
            <h3>{t("transcriptWorkspace.emptySourcesTitle")}</h3>
            <p>{t("transcriptWorkspace.emptySourcesBody")}</p>
          </div>
        ) : (
          <TranscriptSourceList
            deletingSourceId={deletingSourceId}
            interactionDisabled={mutationBusy}
            onDelete={deleteSource}
            onSelect={(source) => {
              if (source.id !== selectedSourceId) {
                resetSourceReview();
                setSelectedSourceId(source.id);
              }
            }}
            selectedSourceId={selectedSourceId}
            sources={sources}
          />
        )}
      </section>

      {selectedSource ? (
        <div className="transcript-review-grid">
          <TranscriptSegmentPanel
            canGoBack={segmentCursors.length > 1}
            hasMore={segmentPage.hasMore}
            items={segmentPage.items}
            key={`segments:${selectedSource.id}:${segmentCursors.length}`}
            loading={segmentLoading}
            maxSegmentBytes={maxSegmentBytes}
            mutationBusy={mutationBusy}
            onNext={() => {
              if (segmentPage.nextCursor !== null) {
                setSegmentCursors((current) => [
                  ...current,
                  segmentPage.nextCursor,
                ]);
              }
            }}
            onPrevious={() =>
              setSegmentCursors((current) => current.slice(0, -1))
            }
            onReload={loadSegments}
            onUpdate={updateSegment}
            page={segmentCursors.length}
            sourceName={selectedSource.displayName}
            totalCount={segmentPage.totalCount}
          />
          <SpeakerRenamePanel
            canGoBack={speakerCursors.length > 1}
            groups={speakerPage.items}
            hasMore={speakerPage.hasMore}
            key={`speakers:${selectedSource.id}:${speakerCursors.length}`}
            loading={speakerLoading}
            mutationBusy={mutationBusy}
            onNext={() => {
              if (speakerPage.nextCursor !== null) {
                setSpeakerCursors((current) => [
                  ...current,
                  speakerPage.nextCursor,
                ]);
              }
            }}
            onPrevious={() =>
              setSpeakerCursors((current) => current.slice(0, -1))
            }
            onRename={renameSpeaker}
            page={speakerCursors.length}
          />
        </div>
      ) : null}
    </div>
  );
}
