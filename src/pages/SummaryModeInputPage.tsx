import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useNavigation } from "../app/navigation";
import { transcriptWorkspacePath } from "../app/paths";
import { useServices } from "../app/services";
import { TranscriptInputPanel } from "../components/TranscriptInputPanel";
import type { DatabaseStatus } from "./HomePage";

export function SummaryModeInputPage({
  onDatabaseStatus,
}: {
  onDatabaseStatus: (status: DatabaseStatus) => void;
}) {
  const { t } = useTranslation();
  const { navigate } = useNavigation();
  const { meetingApi } = useServices();

  useEffect(() => {
    let active = true;
    void meetingApi
      .list()
      .then(() => {
        if (active) onDatabaseStatus("ready");
      })
      .catch(() => {
        if (active) onDatabaseStatus("unavailable");
      });
    return () => {
      active = false;
    };
  }, [meetingApi, onDatabaseStatus]);

  return (
    <div className="page-stack" data-acceptance="summary-mode-input">
      <header className="page-heading summary-mode-heading">
        <div>
          <p className="eyebrow">{t("summaryMode.eyebrow")}</p>
          <h1>{t("summaryMode.title")}</h1>
          <p>{t("summaryMode.description")}</p>
        </div>
      </header>

      <TranscriptInputPanel
        onCancel={() => navigate("/")}
        onCompleted={(result) => {
          if (!result.cancelled) {
            navigate(transcriptWorkspacePath(result.meeting.id));
          }
        }}
        target={{ kind: "create" }}
      />
    </div>
  );
}
