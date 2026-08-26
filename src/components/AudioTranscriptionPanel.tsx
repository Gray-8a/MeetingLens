import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  AudioTranscriptionErrorCode,
  AudioTranscriptionPolicyResponse,
  AudioTranscriptionProgress,
  AudioTranscriptionResult,
} from "../api/audioTranscription";
import { AudioTranscriptionApiError } from "../api/audioTranscription";
import { useServices } from "../app/services";
import { audioTranscriptionErrorMessage } from "../i18n/errors";
import {
  formatDuration,
  formatFileSize,
  formatPercent,
} from "../i18n/formatters";
import { ErrorNotice, InfoNotice } from "./UiState";

interface CreateTarget {
  kind: "create";
  meetingTitle: () => string;
  primaryLanguage: () => string;
}

interface ExistingTarget {
  kind: "existing";
  meetingId: string;
}

export function AudioTranscriptionPanel({
  target,
  interactionDisabled = false,
  onBusyChange,
  onCompleted,
}: {
  target: CreateTarget | ExistingTarget;
  interactionDisabled?: boolean;
  onBusyChange?: (busy: boolean) => void;
  onCompleted: (result: AudioTranscriptionResult) => void;
}) {
  const { t } = useTranslation();
  const { audioTranscriptionApi } = useServices();
  const [policy, setPolicy] = useState<AudioTranscriptionPolicyResponse | null>(
    null,
  );
  const [progress, setProgress] = useState<AudioTranscriptionProgress | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [cancelled, setCancelled] = useState(false);
  const [errorCode, setErrorCode] =
    useState<AudioTranscriptionErrorCode | null>(null);
  const mounted = useRef(true);
  const activeJobId = useRef<string | null>(null);
  const cancelRequested = useRef(false);

  useEffect(() => {
    mounted.current = true;
    void audioTranscriptionApi
      .getPolicy()
      .then((result) => {
        if (!mounted.current) return;
        setPolicy(result);
        if (!result.workerAvailable) {
          setErrorCode("audio_worker_unavailable");
        } else if (!result.modelAvailable) {
          setErrorCode("audio_model_missing");
        } else if (result.busy) {
          setErrorCode("audio_transcription_busy");
        }
      })
      .catch((reason: unknown) => {
        if (mounted.current) setErrorCode(audioErrorCode(reason));
      });
    return () => {
      mounted.current = false;
      const jobId = activeJobId.current;
      activeJobId.current = null;
      if (jobId !== null) {
        void audioTranscriptionApi.cancel(jobId).catch(() => undefined);
      }
    };
  }, [audioTranscriptionApi]);

  useEffect(() => {
    onBusyChange?.(busy);
    return () => {
      if (busy) onBusyChange?.(false);
    };
  }, [busy, onBusyChange]);

  const ready =
    policy !== null &&
    policy.workerAvailable &&
    policy.modelAvailable &&
    !policy.busy;
  const ratio = progressRatio(progress);

  async function selectAndTranscribe() {
    if (busy || interactionDisabled || !ready) return;
    setBusy(true);
    setCancelling(false);
    setCancelled(false);
    setErrorCode(null);
    setProgress(null);
    activeJobId.current = null;
    cancelRequested.current = false;

    const onProgress = (next: AudioTranscriptionProgress) => {
      activeJobId.current = next.jobId;
      if (!mounted.current) return;
      setProgress(next);
      if (next.status === "cancelled") setCancelled(true);
      if (next.status === "failed" && next.errorCode !== null) {
        setErrorCode(next.errorCode);
      }
    };

    try {
      const result =
        target.kind === "create"
          ? await audioTranscriptionApi.selectAudioAndCreateMeeting(
              {
                title: target.meetingTitle(),
                primaryLanguage: target.primaryLanguage(),
              },
              onProgress,
            )
          : await audioTranscriptionApi.selectAndAddAudioToMeeting(
              { meetingId: target.meetingId },
              onProgress,
            );
      if (!mounted.current) return;
      if (result.cancelled) {
        if (result.jobId !== null || cancelRequested.current) {
          setCancelled(true);
        }
        return;
      }
      activeJobId.current = null;
      setProgress(null);
      onCompleted(result);
    } catch (reason: unknown) {
      if (!mounted.current) return;
      const code = audioErrorCode(reason);
      if (code === "audio_cancelled" && cancelRequested.current) {
        setCancelled(true);
      } else {
        setErrorCode(code);
      }
    } finally {
      activeJobId.current = null;
      if (mounted.current) {
        setBusy(false);
        setCancelling(false);
      }
    }
  }

  async function cancel() {
    const jobId = activeJobId.current;
    if (jobId === null || cancelling || !busy) return;
    cancelRequested.current = true;
    setCancelling(true);
    setErrorCode(null);
    try {
      const result = await audioTranscriptionApi.cancel(jobId);
      if (!mounted.current) return;
      setProgress(result);
      if (result.status === "cancelled") setCancelled(true);
    } catch (reason: unknown) {
      if (mounted.current) setErrorCode(audioErrorCode(reason));
    } finally {
      if (mounted.current) setCancelling(false);
    }
  }

  return (
    <section
      className="transcript-input-card transcript-input-card--audio"
      data-acceptance={`audio-transcription-${target.kind}`}
    >
      <div>
        <h2>{t("audioTranscription.title")}</h2>
        <p>{t("audioTranscription.description")}</p>
        <p className="field-help">{t("audioTranscription.localOnly")}</p>
        {policy ? (
          <div className="audio-transcription-policy">
            <span>
              {t("audioTranscription.runtime", {
                model: policy.model,
              })}
            </span>
            <span>
              {t("audioTranscription.limits", {
                size: formatFileSize(policy.maxFileBytes),
                duration: formatDuration(policy.maxDurationMs),
              })}
            </span>
          </div>
        ) : null}
      </div>

      {errorCode ? (
        <ErrorNotice
          code={errorCode}
          message={audioTranscriptionErrorMessage(errorCode)}
        />
      ) : null}
      {cancelled ? (
        <InfoNotice>
          <strong>{t("audioTranscription.cancelled")}</strong>
          <span>{t("audioTranscription.cancelledBody")}</span>
        </InfoNotice>
      ) : null}

      {progress && busy && progress.status === "running" ? (
        <div className="audio-transcription-progress" role="status">
          <div>
            <strong>{t(`audioTranscription.phase.${progress.phase}`)}</strong>
            {ratio !== null ? <span>{formatPercent(ratio)}</span> : null}
          </div>
          {ratio !== null ? (
            <progress
              aria-label={t("audioTranscription.progressLabel")}
              max={1}
              value={ratio}
            />
          ) : (
            <span className="spinner" aria-hidden="true" />
          )}
        </div>
      ) : null}

      <div className="form-actions">
        <button
          className="button button--secondary"
          data-acceptance={`select-audio-${target.kind}`}
          disabled={busy || interactionDisabled || !ready}
          onClick={() => void selectAndTranscribe()}
          type="button"
        >
          {busy
            ? t("audioTranscription.transcribing")
            : t("audioTranscription.button")}
        </button>
        {busy && progress?.status === "running" ? (
          <button
            className="button button--ghost"
            data-acceptance="cancel-audio-transcription"
            disabled={cancelling}
            onClick={() => void cancel()}
            type="button"
          >
            {cancelling
              ? t("audioTranscription.cancelling")
              : t("common.cancel")}
          </button>
        ) : null}
      </div>
    </section>
  );
}

function audioErrorCode(reason: unknown): AudioTranscriptionErrorCode {
  return reason instanceof AudioTranscriptionApiError
    ? reason.code
    : "internal_error";
}

function progressRatio(
  progress: AudioTranscriptionProgress | null,
): number | null {
  if (progress === null) return null;
  if (progress.durationMs !== null && progress.durationMs > 0) {
    return (
      Math.min(progress.processedMs ?? 0, progress.durationMs) /
      progress.durationMs
    );
  }
  if (progress.totalBytes !== null && progress.totalBytes > 0) {
    return (
      Math.min(progress.bytesProcessed, progress.totalBytes) /
      progress.totalBytes
    );
  }
  return null;
}
