import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppLink } from "../app/navigation";
import { transcriptWorkspacePath } from "../app/paths";
import { useServices } from "../app/services";
import type { MeetingStatus } from "../api/meeting";
import type {
  TranscriptErrorCode,
  TranscriptSourceResponse,
} from "../api/transcript";
import { transcriptErrorMessage } from "../i18n/errors";
import { formatNumber } from "../i18n/formatters";
import { transcriptErrorCode } from "../lib/transcriptPresentation";
import { ErrorNotice, LoadingState } from "./UiState";
import { TranscriptInputPanel } from "./TranscriptInputPanel";

export function MeetingTranscriptPanel({
  meetingId,
  meetingStatus,
}: {
  meetingId: string;
  meetingStatus: MeetingStatus;
}) {
  const { t } = useTranslation();
  const { transcriptApi } = useServices();
  const [sources, setSources] = useState<TranscriptSourceResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [errorCode, setErrorCode] = useState<TranscriptErrorCode | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await transcriptApi.listSources(meetingId);
      setSources(result.sources);
      setErrorCode(null);
    } catch (reason: unknown) {
      setErrorCode(transcriptErrorCode(reason));
    } finally {
      setLoading(false);
    }
  }, [meetingId, transcriptApi]);

  useEffect(() => {
    let active = true;
    void transcriptApi
      .listSources(meetingId)
      .then((result) => {
        if (!active) return;
        setSources(result.sources);
        setErrorCode(null);
      })
      .catch((reason: unknown) => {
        if (active) setErrorCode(transcriptErrorCode(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [meetingId, transcriptApi]);

  const inputAllowed =
    meetingStatus === "draft" ||
    meetingStatus === "recording" ||
    meetingStatus === "ready";

  return (
    <section
      className="content-panel meeting-transcript-panel"
      data-acceptance="meeting-transcript-panel"
    >
      <div className="section-heading">
        <div>
          <p className="eyebrow">{t("transcriptWorkspace.sourcesTitle")}</p>
          <h2>{t("transcriptWorkspace.sourcesTitle")}</h2>
        </div>
        <div className="meeting-transcript-panel__summary">
          <span>
            {t("transcriptWorkspace.sourceCount", {
              count: sources.length,
              formattedCount: formatNumber(sources.length),
            })}
          </span>
          <AppLink
            className="button button--secondary"
            to={transcriptWorkspacePath(meetingId)}
          >
            {t("transcriptWorkspace.view")}
          </AppLink>
        </div>
      </div>

      {loading ? (
        <LoadingState label={t("transcriptWorkspace.loading")} />
      ) : null}
      {errorCode ? (
        <ErrorNotice
          code={errorCode}
          message={transcriptErrorMessage(errorCode)}
        />
      ) : null}

      {inputAllowed ? (
        <TranscriptInputPanel
          onCompleted={() => void refresh()}
          target={{ kind: "existing", meetingId }}
        />
      ) : (
        <p className="restriction">
          {t("transcriptWorkspace.inputRestricted")}
        </p>
      )}
    </section>
  );
}
