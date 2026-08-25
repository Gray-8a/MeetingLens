import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { meetingPath, useNavigation } from "../app/navigation";
import { useNavigationBlocker } from "../app/navigationBlocker";
import { summaryMinutesPath } from "../app/paths";
import { useServices } from "../app/services";
import type { MeetingResponse } from "../api/meeting";
import {
  toSummaryError,
  type DeepSeekProviderStatusResponse,
  type SummaryErrorCode,
  type SummaryExternalDisclosureResponse,
  type SummaryOutputLanguage,
  type SummaryPolicyResponse,
  type SummaryRunResponse,
  type SummaryTemplate,
} from "../api/summary";
import type { TranscriptSourceResponse } from "../api/transcript";
import { DeepSeekConsentDialog } from "../components/DeepSeekConsentDialog";
import { SummaryResultView } from "../components/SummaryResultView";
import { UnsavedChangesDialog } from "../components/UnsavedChangesDialog";
import { ErrorNotice, LoadingState } from "../components/UiState";
import { summaryErrorMessage } from "../i18n/errors";
import {
  formatFileSize,
  formatLocalDate,
  formatNumber,
} from "../i18n/formatters";
import { meetingErrorMessage, uiError } from "../lib/presentation";
import { transcriptWorkspacePath } from "../app/paths";
import type { DatabaseStatus } from "./HomePage";

const RUN_PAGE_SIZE = 20;
const POLL_INTERVAL_MS = 1_000;

interface ConsentState {
  run: SummaryRunResponse;
  disclosure: SummaryExternalDisclosureResponse;
  cancelOnDismiss: boolean;
}

interface PageToken {
  generation: number;
  meetingId: string;
}

export function SummaryMinutesPage({
  meetingId,
  summaryId,
  onDatabaseStatus,
  onMeetingStatus,
}: {
  meetingId: string;
  summaryId: string | null;
  onDatabaseStatus: (status: DatabaseStatus) => void;
  onMeetingStatus: (status: string | null) => void;
}) {
  const { t } = useTranslation();
  const { meetingApi, summaryApi, transcriptApi } = useServices();
  const { navigate } = useNavigation();
  const [meeting, setMeeting] = useState<MeetingResponse | null>(null);
  const [sources, setSources] = useState<TranscriptSourceResponse[]>([]);
  const [policy, setPolicy] = useState<SummaryPolicyResponse | null>(null);
  const [providerStatus, setProviderStatus] =
    useState<DeepSeekProviderStatusResponse | null>(null);
  const [runs, setRuns] = useState<SummaryRunResponse[]>([]);
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>([]);
  const [template, setTemplate] = useState<SummaryTemplate>("detailed");
  const [outputLanguage, setOutputLanguage] =
    useState<SummaryOutputLanguage>("zh");
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [totalCount, setTotalCount] = useState(0);
  const [injectedSummaryId, setInjectedSummaryId] = useState<string | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [error, setError] = useState<SummaryErrorCode | null>(null);
  const [providerStatusError, setProviderStatusError] =
    useState<SummaryErrorCode | null>(null);
  const [deepLinkError, setDeepLinkError] = useState<SummaryErrorCode | null>(
    null,
  );
  const [fatalMeetingError, setFatalMeetingError] = useState<ReturnType<
    typeof uiError
  > | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [consent, setConsent] = useState<ConsentState | null>(null);
  const [consentError, setConsentError] = useState<SummaryErrorCode | null>(
    null,
  );
  const [resultDirty, setResultDirty] = useState(false);
  const [pendingNavigation, setPendingNavigation] = useState<string | null>(
    null,
  );
  const actionBusyRef = useRef(false);
  const executingIdsRef = useRef(new Set<string>());
  const loadGenerationRef = useRef(0);
  const pageGenerationRef = useRef(1);
  const pageActiveRef = useRef(true);
  const pageMeetingIdRef = useRef(meetingId);
  const routeSummaryIdRef = useRef(summaryId);
  routeSummaryIdRef.current = summaryId;
  const allowNavigationRef = useRef(false);
  const detachedCancellationIdsRef = useRef(new Set<string>());
  const consentGenerationRef = useRef(0);

  const cancelDetachedRun = useCallback(
    async (run: SummaryRunResponse) => {
      if (detachedCancellationIdsRef.current.has(run.id)) return;
      detachedCancellationIdsRef.current.add(run.id);
      try {
        await summaryApi.cancelRun({
          meetingId: run.meetingId,
          summaryId: run.id,
        });
      } catch {
        // Best-effort cleanup for a newly queued run after this page was left.
      }
    },
    [summaryApi],
  );

  const proceedNavigation = useCallback(
    (path: string) => {
      allowNavigationRef.current = true;
      navigate(path);
      window.setTimeout(() => {
        allowNavigationRef.current = false;
      }, 0);
    },
    [navigate],
  );

  const handleNavigationBlocked = useCallback(
    (path: string) => {
      if (allowNavigationRef.current) return;
      if (consent) {
        const currentConsent = consent;
        consentGenerationRef.current += 1;
        setConsent(null);
        setConsentError(null);
        void (async () => {
          if (currentConsent.cancelOnDismiss) {
            await cancelDetachedRun(currentConsent.run);
          }
          if (resultDirty) setPendingNavigation(path);
          else proceedNavigation(path);
        })();
        return;
      }
      setPendingNavigation(path);
    },
    [cancelDetachedRun, consent, proceedNavigation, resultDirty],
  );

  const shouldBlockNavigation = useCallback(
    () => (resultDirty || consent !== null) && !allowNavigationRef.current,
    [consent, resultDirty],
  );

  useNavigationBlocker(shouldBlockNavigation, handleNavigationBlocked);

  const handleDirtyChange = useCallback((dirty: boolean) => {
    setResultDirty(dirty);
    if (!dirty) setPendingNavigation(null);
  }, []);

  const capturePage = useCallback(
    (): PageToken => ({
      generation: pageGenerationRef.current,
      meetingId,
    }),
    [meetingId],
  );

  const isPageCurrent = useCallback((token: PageToken): boolean => {
    return (
      pageActiveRef.current &&
      pageGenerationRef.current === token.generation &&
      pageMeetingIdRef.current === token.meetingId
    );
  }, []);

  useEffect(() => {
    if (!resultDirty) return;
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [resultDirty]);

  const upsertRun = useCallback(
    (run: SummaryRunResponse) => {
      if (!pageActiveRef.current || run.meetingId !== meetingId) return;
      setRuns((current) =>
        [run, ...current.filter((item) => item.id !== run.id)].sort(
          compareRuns,
        ),
      );
    },
    [meetingId],
  );

  const loadInitial = useCallback(async () => {
    const pageToken = capturePage();
    const generation = ++loadGenerationRef.current;
    const stillCurrent = () =>
      generation === loadGenerationRef.current && isPageCurrent(pageToken);
    setLoading(true);
    setError(null);
    setProviderStatusError(null);
    setDeepLinkError(null);
    setFatalMeetingError(null);
    onDatabaseStatus("checking");
    try {
      const providerResultPromise = summaryApi.getDeepSeekProviderStatus().then(
        (status) => ({ status, error: null as SummaryErrorCode | null }),
        (reason: unknown) => ({
          status: null,
          error: toSummaryError(reason).code,
        }),
      );
      const [loadedMeeting, sourcePage, loadedPolicy, runPage, providerResult] =
        await Promise.all([
          meetingApi.get(meetingId),
          transcriptApi.listSources(meetingId),
          summaryApi.getPolicy(),
          summaryApi.listRuns({ meetingId, limit: RUN_PAGE_SIZE }),
          providerResultPromise,
        ]);
      if (!stillCurrent()) return;
      let loadedRuns = runPage.items;
      let injectedId: string | null = null;
      let selectionFailure: SummaryErrorCode | null = null;
      if (summaryId && !loadedRuns.some((run) => run.id === summaryId)) {
        try {
          const injectedRun = await summaryApi.getRun({ meetingId, summaryId });
          if (!stillCurrent()) return;
          loadedRuns = [injectedRun, ...loadedRuns];
          injectedId = summaryId;
        } catch (reason: unknown) {
          if (!stillCurrent()) return;
          selectionFailure = toSummaryError(reason).code;
        }
      }
      if (!stillCurrent()) return;
      setMeeting(loadedMeeting);
      setSources(sourcePage.sources);
      setPolicy(loadedPolicy);
      setProviderStatus(providerResult.status);
      setProviderStatusError(providerResult.error);
      setDeepLinkError(selectionFailure);
      setRuns(uniqueRuns(loadedRuns).sort(compareRuns));
      setSelectedSourceIds(
        sourcePage.sources
          .filter((source) => source.segmentCount > 0)
          .map((source) => source.id),
      );
      setNextCursor(runPage.nextCursor);
      setHasMore(runPage.hasMore);
      setTotalCount(runPage.totalCount);
      setInjectedSummaryId(injectedId);
      onMeetingStatus(loadedMeeting.status);
      onDatabaseStatus("ready");
    } catch (reason: unknown) {
      if (!stillCurrent()) return;
      const meetingFailure = uiError(reason);
      if (meetingFailure.code === "meeting_not_found") {
        setFatalMeetingError(meetingFailure);
        onMeetingStatus(null);
        onDatabaseStatus("ready");
      } else {
        setError(toSummaryError(reason).code);
        onDatabaseStatus("unavailable");
      }
    } finally {
      if (stillCurrent()) setLoading(false);
    }
  }, [
    capturePage,
    isPageCurrent,
    meetingApi,
    meetingId,
    onDatabaseStatus,
    onMeetingStatus,
    summaryApi,
    summaryId,
    transcriptApi,
  ]);

  useEffect(() => {
    pageActiveRef.current = true;
    pageMeetingIdRef.current = meetingId;
    return () => {
      pageActiveRef.current = false;
      pageGenerationRef.current += 1;
      consentGenerationRef.current += 1;
      onMeetingStatus(null);
    };
  }, [meetingId, onMeetingStatus]);

  useEffect(() => {
    const timer = window.setTimeout(() => void loadInitial(), 0);
    return () => {
      window.clearTimeout(timer);
      loadGenerationRef.current += 1;
    };
  }, [loadInitial]);

  const selectedRun = useMemo(
    () =>
      (summaryId ? runs.find((run) => run.id === summaryId) : runs[0]) ?? null,
    [runs, summaryId],
  );
  const latestBySeries = useMemo(() => {
    const latest = new Map<string, SummaryRunResponse>();
    for (const run of runs) {
      const previous = latest.get(run.seriesId);
      if (!previous || run.revision > previous.revision)
        latest.set(run.seriesId, run);
    }
    return latest;
  }, [runs]);
  const activeRunIdsKey = useMemo(
    () =>
      runs
        .filter((run) => run.status === "queued" || run.status === "running")
        .map((run) => run.id)
        .sort()
        .join(","),
    [runs],
  );

  useEffect(() => {
    if (activeRunIdsKey.length === 0) return;
    const pageToken = capturePage();
    const activeRunIds = activeRunIdsKey.split(",");
    let active = true;
    const poll = async () => {
      const results = await Promise.allSettled(
        activeRunIds.map((activeSummaryId) =>
          summaryApi.getRun({
            meetingId,
            summaryId: activeSummaryId,
          }),
        ),
      );
      if (!active || !isPageCurrent(pageToken)) return;
      for (const result of results) {
        if (result.status === "fulfilled") upsertRun(result.value);
      }
      const failed = results.find(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      );
      if (failed) {
        setError(toSummaryError(failed.reason).code);
      }
    };
    const timer = window.setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [
    activeRunIdsKey,
    capturePage,
    isPageCurrent,
    meetingId,
    summaryApi,
    upsertRun,
  ]);

  function beginAction(): PageToken | null {
    if (actionBusyRef.current || !pageActiveRef.current) return null;
    const pageToken = capturePage();
    actionBusyRef.current = true;
    setActionBusy(true);
    setError(null);
    return pageToken;
  }

  function endAction(pageToken: PageToken) {
    if (!isPageCurrent(pageToken)) return;
    actionBusyRef.current = false;
    setActionBusy(false);
  }

  function selectRun(run: SummaryRunResponse) {
    if (run.meetingId !== meetingId || !pageActiveRef.current) return;
    navigate(summaryMinutesPath(meetingId, run.id));
  }

  function selectCreatedRun(run: SummaryRunResponse) {
    if (run.meetingId !== meetingId || !pageActiveRef.current) return;
    routeSummaryIdRef.current = run.id;
    navigate(summaryMinutesPath(meetingId, run.id));
  }

  function launchExecution(run: SummaryRunResponse) {
    if (
      run.meetingId !== meetingId ||
      !pageActiveRef.current ||
      executingIdsRef.current.has(run.id)
    )
      return;
    const pageToken = capturePage();
    executingIdsRef.current.add(run.id);
    void summaryApi
      .executeRun({ meetingId, summaryId: run.id })
      .then((updated) => {
        if (isPageCurrent(pageToken)) upsertRun(updated);
      })
      .catch(async (reason: unknown) => {
        if (!isPageCurrent(pageToken)) return;
        setError(toSummaryError(reason).code);
        try {
          const reconciled = await summaryApi.getRun({
            meetingId,
            summaryId: run.id,
          });
          if (isPageCurrent(pageToken)) upsertRun(reconciled);
        } catch {
          // Keep the stable execute error if status reconciliation also fails.
        }
      })
      .finally(() => executingIdsRef.current.delete(run.id));
  }

  async function prepareExecution(
    run: SummaryRunResponse,
    cancelOnDismiss: boolean,
  ) {
    const pageToken = beginAction();
    if (!pageToken) return;
    if (run.meetingId !== meetingId) {
      endAction(pageToken);
      return;
    }
    const routeSummaryId = routeSummaryIdRef.current;
    try {
      const disclosure = await summaryApi.getExternalDisclosure({
        summaryId: run.id,
      });
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      ) {
        if (cancelOnDismiss) await cancelDetachedRun(run);
        return;
      }
      if (disclosure.requiresConfirmation) {
        consentGenerationRef.current += 1;
        setConsent({ run, disclosure, cancelOnDismiss });
        setConsentError(null);
      } else {
        launchExecution(run);
      }
    } catch (reason: unknown) {
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      ) {
        if (cancelOnDismiss) await cancelDetachedRun(run);
        return;
      }
      setError(toSummaryError(reason).code);
      if (cancelOnDismiss) await cancelAndReconcile(run, false);
    } finally {
      endAction(pageToken);
    }
  }

  async function createRun() {
    if (resultDirty) return;
    const pageToken = beginAction();
    if (!pageToken) return;
    const routeSummaryId = routeSummaryIdRef.current;
    let created: SummaryRunResponse | null = null;
    try {
      const status = await summaryApi.getDeepSeekProviderStatus();
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      )
        return;
      setProviderStatus(status);
      setProviderStatusError(null);
      if (!status.keyConfigured || !status.executionAvailable) {
        setError("deepseek_api_key_missing");
        return;
      }
      created = await summaryApi.createRun({
        meetingId,
        sourceIds: selectedSourceIds,
        template,
        outputLanguage,
      });
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      ) {
        await cancelDetachedRun(created);
        return;
      }
      upsertRun(created);
      selectCreatedRun(created);
      const disclosure = await summaryApi.getExternalDisclosure({
        summaryId: created.id,
      });
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== created.id
      ) {
        await cancelDetachedRun(created);
        return;
      }
      if (disclosure.requiresConfirmation) {
        consentGenerationRef.current += 1;
        setConsent({ run: created, disclosure, cancelOnDismiss: true });
        setConsentError(null);
      } else {
        launchExecution(created);
      }
    } catch (reason: unknown) {
      if (
        !isPageCurrent(pageToken) ||
        (created === null
          ? routeSummaryIdRef.current !== routeSummaryId
          : routeSummaryIdRef.current !== created.id)
      ) {
        if (created) await cancelDetachedRun(created);
        return;
      }
      setError(toSummaryError(reason).code);
      if (created) await cancelAndReconcile(created, false);
    } finally {
      endAction(pageToken);
    }
  }

  async function createRevision(base: SummaryRunResponse) {
    const pageToken = beginAction();
    if (!pageToken) return;
    if (base.meetingId !== meetingId) {
      endAction(pageToken);
      return;
    }
    if (
      routeSummaryIdRef.current !== null &&
      routeSummaryIdRef.current !== base.id
    ) {
      endAction(pageToken);
      return;
    }
    const routeSummaryId = routeSummaryIdRef.current;
    let created: SummaryRunResponse | null = null;
    try {
      const status = await summaryApi.getDeepSeekProviderStatus();
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      )
        return;
      setProviderStatus(status);
      setProviderStatusError(null);
      if (!status.keyConfigured || !status.executionAvailable) {
        setError("deepseek_api_key_missing");
        return;
      }
      created = await summaryApi.createRevision({
        meetingId,
        summaryId: base.id,
      });
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== routeSummaryId
      ) {
        await cancelDetachedRun(created);
        return;
      }
      upsertRun(created);
      selectCreatedRun(created);
      const disclosure = await summaryApi.getExternalDisclosure({
        summaryId: created.id,
      });
      if (
        !isPageCurrent(pageToken) ||
        routeSummaryIdRef.current !== created.id
      ) {
        await cancelDetachedRun(created);
        return;
      }
      if (disclosure.requiresConfirmation) {
        consentGenerationRef.current += 1;
        setConsent({ run: created, disclosure, cancelOnDismiss: true });
        setConsentError(null);
      } else {
        launchExecution(created);
      }
    } catch (reason: unknown) {
      if (
        !isPageCurrent(pageToken) ||
        (created === null
          ? routeSummaryIdRef.current !== routeSummaryId
          : routeSummaryIdRef.current !== created.id)
      ) {
        if (created) await cancelDetachedRun(created);
        return;
      }
      setError(toSummaryError(reason).code);
      if (created) await cancelAndReconcile(created, false);
    } finally {
      endAction(pageToken);
    }
  }

  async function cancelAndReconcile(run: SummaryRunResponse, visible = true) {
    const pageToken = visible ? beginAction() : capturePage();
    if (!pageToken) return;
    if (run.meetingId !== meetingId) {
      if (visible) endAction(pageToken);
      return;
    }
    try {
      const cancelled = await summaryApi.cancelRun({
        meetingId,
        summaryId: run.id,
      });
      if (isPageCurrent(pageToken)) upsertRun(cancelled);
    } catch (reason: unknown) {
      if (visible && isPageCurrent(pageToken))
        setError(toSummaryError(reason).code);
      try {
        const reconciled = await summaryApi.getRun({
          meetingId,
          summaryId: run.id,
        });
        if (isPageCurrent(pageToken)) upsertRun(reconciled);
      } catch {
        // Preserve the original stable cancellation outcome.
      }
    } finally {
      if (visible) endAction(pageToken);
    }
  }

  async function confirmConsent(remember: boolean) {
    if (!consent) return;
    const pageToken = beginAction();
    if (!pageToken) return;
    const currentConsent = consent;
    const consentGeneration = consentGenerationRef.current;
    try {
      await summaryApi.confirmExternalDisclosure({
        summaryId: currentConsent.run.id,
        remember,
      });
      if (
        !isPageCurrent(pageToken) ||
        consentGenerationRef.current !== consentGeneration
      ) {
        if (currentConsent.cancelOnDismiss)
          await cancelDetachedRun(currentConsent.run);
        return;
      }
      const run = currentConsent.run;
      consentGenerationRef.current += 1;
      setConsent(null);
      setConsentError(null);
      launchExecution(run);
    } catch (reason: unknown) {
      if (
        !isPageCurrent(pageToken) ||
        consentGenerationRef.current !== consentGeneration
      )
        return;
      const code = toSummaryError(reason).code;
      setConsentError(code);
      throw reason;
    } finally {
      endAction(pageToken);
    }
  }

  function dismissConsent() {
    const current = consent;
    consentGenerationRef.current += 1;
    setConsent(null);
    setConsentError(null);
    if (current?.cancelOnDismiss) void cancelAndReconcile(current.run);
  }

  function discardDraftAndNavigate() {
    const target = pendingNavigation;
    if (!target) return;
    setResultDirty(false);
    setPendingNavigation(null);
    proceedNavigation(target);
  }

  async function loadMoreRuns() {
    if (!nextCursor || historyLoading) return;
    const pageToken = capturePage();
    setHistoryLoading(true);
    setError(null);
    try {
      const page = await summaryApi.listRuns({
        meetingId,
        cursor: nextCursor,
        limit: RUN_PAGE_SIZE,
      });
      if (!isPageCurrent(pageToken)) return;
      setRuns((current) =>
        uniqueRuns([...current, ...page.items]).sort(compareRuns),
      );
      setNextCursor(page.nextCursor);
      setHasMore(page.hasMore);
      setTotalCount(page.totalCount);
    } catch (reason: unknown) {
      if (isPageCurrent(pageToken)) setError(toSummaryError(reason).code);
    } finally {
      if (isPageCurrent(pageToken)) setHistoryLoading(false);
    }
  }

  if (loading) return <LoadingState label={t("summaryMinutes.loading")} />;
  if (!meeting) {
    return (
      <section className="content-panel narrow-panel">
        {fatalMeetingError ? (
          <ErrorNotice
            code={fatalMeetingError.code}
            message={meetingErrorMessage(fatalMeetingError.code)}
          />
        ) : null}
        {!fatalMeetingError && error ? (
          <ErrorNotice code={error} message={summaryErrorMessage(error)} />
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

  const validSources = sources.filter((source) => source.segmentCount > 0);

  return (
    <div
      className="page-stack summary-minutes"
      data-acceptance="summary-minutes-workspace"
    >
      <header className="detail-hero">
        <div>
          <p className="eyebrow">{t("summaryMinutes.eyebrow")}</p>
          <h1>{meeting.title}</h1>
          <p>{t("summaryMinutes.description")}</p>
        </div>
        <div className="form-actions">
          <button
            className="text-link"
            onClick={() => navigate(transcriptWorkspacePath(meeting.id))}
            type="button"
          >
            {t("summaryMinutes.backToTranscript")}
          </button>
          <button
            className="text-link"
            onClick={() => navigate(meetingPath(meeting.id))}
            type="button"
          >
            {t("transcriptWorkspace.backToMeeting")}
          </button>
        </div>
      </header>

      {error ? (
        <ErrorNotice code={error} message={summaryErrorMessage(error)} />
      ) : null}
      {deepLinkError ? (
        <ErrorNotice
          code={deepLinkError}
          message={summaryErrorMessage(deepLinkError)}
        />
      ) : null}

      <div className="summary-minutes-grid">
        <aside className="content-panel summary-generation-panel">
          <div className="section-heading">
            <div>
              <p className="eyebrow">{t("summaryMinutes.generate.eyebrow")}</p>
              <h2>{t("summaryMinutes.generate.title")}</h2>
            </div>
          </div>
          {providerStatus && !providerStatus.keyConfigured ? (
            <div className="notice notice--info">
              <p>{t("summaryMinutes.generate.keyRequired")}</p>
              <button
                className="button button--secondary"
                onClick={() => navigate("/settings")}
                type="button"
              >
                {t("summaryMinutes.generate.openSettings")}
              </button>
            </div>
          ) : null}
          {providerStatusError ? (
            <div className="notice notice--info">
              <ErrorNotice
                code={providerStatusError}
                message={summaryErrorMessage(providerStatusError)}
              />
              <button
                className="button button--secondary"
                onClick={() => navigate("/settings")}
                type="button"
              >
                {t("summaryMinutes.generate.openSettings")}
              </button>
            </div>
          ) : null}
          <fieldset disabled={actionBusy || resultDirty}>
            <legend>{t("summaryMinutes.generate.sources")}</legend>
            {sources.length === 0 ? (
              <p>{t("summaryMinutes.generate.noSources")}</p>
            ) : (
              sources.map((source) => (
                <label className="summary-source-option" key={source.id}>
                  <input
                    checked={selectedSourceIds.includes(source.id)}
                    disabled={source.segmentCount === 0}
                    onChange={(event) =>
                      setSelectedSourceIds((current) =>
                        event.currentTarget.checked
                          ? [...current, source.id]
                          : current.filter((id) => id !== source.id),
                      )
                    }
                    type="checkbox"
                  />
                  <span>
                    <strong>{source.displayName}</strong>
                    <small>
                      {t("summaryMinutes.generate.segmentCount", {
                        count: source.segmentCount,
                        formattedCount: formatNumber(source.segmentCount),
                      })}
                    </small>
                    {selectedSourceIds.includes(source.id) ? (
                      <small>
                        {t("summaryMinutes.generate.selectionOrder", {
                          number: selectedSourceIds.indexOf(source.id) + 1,
                        })}
                      </small>
                    ) : null}
                  </span>
                </label>
              ))
            )}
          </fieldset>
          <label>
            {t("summaryMinutes.generate.template")}
            <select
              disabled={actionBusy || resultDirty}
              onChange={(event) =>
                setTemplate(event.currentTarget.value as SummaryTemplate)
              }
              value={template}
            >
              {(policy?.templates ?? []).map((value) => (
                <option key={value} value={value}>
                  {t(`summaryConsent.templateValue.${value}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t("summaryMinutes.generate.outputLanguage")}
            <select
              disabled={actionBusy || resultDirty}
              onChange={(event) =>
                setOutputLanguage(
                  event.currentTarget.value as SummaryOutputLanguage,
                )
              }
              value={outputLanguage}
            >
              {(policy?.outputLanguages ?? []).map((value) => (
                <option key={value} value={value}>
                  {t(`summaryConsent.outputLanguageValue.${value}`)}
                </option>
              ))}
            </select>
          </label>
          <p className="supporting-copy">
            {t("summaryMinutes.generate.externalNotice", {
              host: providerStatus?.endpointHost ?? "api.deepseek.com",
              model: providerStatus?.model ?? "deepseek-v4-pro",
            })}
          </p>
          <button
            className="button button--primary"
            data-acceptance="generate-summary-minutes"
            disabled={
              actionBusy ||
              resultDirty ||
              selectedSourceIds.length === 0 ||
              validSources.length === 0 ||
              !providerStatus?.keyConfigured
            }
            onClick={() => void createRun()}
            type="button"
          >
            {actionBusy
              ? t("summaryMinutes.actions.preparing")
              : t("summaryMinutes.actions.generate")}
          </button>
        </aside>

        <div className="summary-minutes-main">
          {selectedRun ? (
            <SummaryResultView
              key={`${selectedRun.id}:${selectedRun.status}:${selectedRun.contentVersion ?? "none"}`}
              actionBusy={actionBusy}
              isLatestRevision={
                latestBySeries.get(selectedRun.seriesId)?.id ===
                  selectedRun.id &&
                !(injectedSummaryId === selectedRun.id && hasMore)
              }
              latestRevisionKnown={
                !(
                  injectedSummaryId === selectedRun.id &&
                  hasMore &&
                  latestBySeries.get(selectedRun.seriesId)?.id ===
                    selectedRun.id
                )
              }
              onCancel={(run) => cancelAndReconcile(run)}
              onContinue={(run) => prepareExecution(run, false)}
              onOpenLatest={(seriesId) => {
                const latest = latestBySeries.get(seriesId);
                if (latest && latest.id !== selectedRun.id) selectRun(latest);
                else if (hasMore) void loadMoreRuns();
              }}
              onDirtyChange={handleDirtyChange}
              onRegenerate={createRevision}
              onRunUpdated={upsertRun}
              run={selectedRun}
            />
          ) : (
            <section className="content-panel">
              <h2>{t("summaryMinutes.empty.title")}</h2>
              <p>{t("summaryMinutes.empty.body")}</p>
              {summaryId && runs[0] ? (
                <button
                  className="button button--secondary"
                  onClick={() => selectRun(runs[0]!)}
                  type="button"
                >
                  {t("summaryMinutes.actions.openNewestLoaded")}
                </button>
              ) : null}
            </section>
          )}
        </div>
      </div>

      <section className="content-panel summary-history">
        <div className="section-heading">
          <div>
            <p className="eyebrow">{t("summaryMinutes.history.eyebrow")}</p>
            <h2>{t("summaryMinutes.history.title")}</h2>
          </div>
          <span>
            {t("summaryMinutes.history.count", {
              count: totalCount,
              formattedCount: formatNumber(totalCount),
            })}
          </span>
        </div>
        {runs.length === 0 ? (
          <p>{t("summaryMinutes.history.empty")}</p>
        ) : (
          <ol>
            {runs.map((run) => (
              <li key={run.id}>
                <button
                  aria-current={selectedRun?.id === run.id ? "true" : undefined}
                  className={
                    selectedRun?.id === run.id
                      ? "summary-history-item summary-history-item--active"
                      : "summary-history-item"
                  }
                  onClick={() => selectRun(run)}
                  type="button"
                >
                  <span>
                    {t("summaryMinutes.result.revision", {
                      revision: run.revision,
                    })}
                  </span>
                  <span>{t(`summaryMinutes.status.${run.status}`)}</span>
                  <small>
                    {formatLocalDate(run.createdAt)} ·{" "}
                    {formatFileSize(run.inputBytes)}
                  </small>
                  <small>
                    {t("summaryMinutes.history.details", {
                      count: run.sourceCount,
                      formattedCount: formatNumber(run.sourceCount),
                      language: t(
                        `summaryConsent.outputLanguageValue.${run.outputLanguage}`,
                      ),
                      template: t(
                        `summaryConsent.templateValue.${run.template}`,
                      ),
                    })}
                  </small>
                </button>
              </li>
            ))}
          </ol>
        )}
        {hasMore ? (
          <button
            className="button button--secondary"
            disabled={historyLoading}
            onClick={() => void loadMoreRuns()}
            type="button"
          >
            {historyLoading
              ? t("common.loading")
              : t("summaryMinutes.history.loadMore")}
          </button>
        ) : null}
      </section>

      {consent ? (
        <DeepSeekConsentDialog
          disclosure={consent.disclosure}
          errorCode={consentError}
          onCancel={dismissConsent}
          onConfirm={confirmConsent}
        />
      ) : null}
      {pendingNavigation ? (
        <UnsavedChangesDialog
          onCancel={() => setPendingNavigation(null)}
          onDiscard={discardDraftAndNavigate}
        />
      ) : null}
    </div>
  );
}

function compareRuns(
  left: SummaryRunResponse,
  right: SummaryRunResponse,
): number {
  const dateOrder = Date.parse(right.createdAt) - Date.parse(left.createdAt);
  return dateOrder !== 0 ? dateOrder : left.id.localeCompare(right.id);
}

function uniqueRuns(runs: SummaryRunResponse[]): SummaryRunResponse[] {
  return [...new Map(runs.map((run) => [run.id, run])).values()];
}
