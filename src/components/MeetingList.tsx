import { useTranslation } from "react-i18next";
import type { MeetingResponse } from "../api/meeting";
import { meetingPath, AppLink } from "../app/navigation";
import { formatLocalDate } from "../lib/presentation";
import { StatusBadge } from "./StatusBadge";

export interface MeetingListProps {
  meetings: readonly MeetingResponse[];
  confirmingId?: string | null;
  deletingId?: string | null;
  onRequestDelete?: (id: string) => void;
  onConfirmDelete?: (id: string) => void;
  onRetryDelete?: (id: string) => void;
  onCancelDelete?: () => void;
}

export function MeetingList({
  meetings,
  confirmingId = null,
  deletingId = null,
  onRequestDelete,
  onConfirmDelete,
  onRetryDelete,
  onCancelDelete,
}: MeetingListProps) {
  const { t } = useTranslation();

  return (
    <div className="meeting-list" aria-label={t("meetingList.ariaLabel")}>
      {meetings.map((meeting) => {
        const canDelete =
          meeting.status === "draft" || meeting.status === "ready";
        const canRetryDelete = meeting.status === "delete_failed";
        const confirming = confirmingId === meeting.id;
        const deleting = deletingId === meeting.id;

        return (
          <article
            className="meeting-row"
            data-meeting-id={meeting.id}
            data-testid={`meeting-row-${meeting.id}`}
            key={meeting.id}
          >
            <div className="meeting-row__main">
              <div className="meeting-row__heading">
                <h3>{meeting.title}</h3>
                <StatusBadge status={meeting.status} />
              </div>
              <dl className="meeting-times">
                <div>
                  <dt>{t("meetingList.created")}</dt>
                  <dd>{formatLocalDate(meeting.createdAt)}</dd>
                </div>
                <div>
                  <dt>{t("meetingList.started")}</dt>
                  <dd>{formatLocalDate(meeting.startedAt)}</dd>
                </div>
                <div>
                  <dt>{t("meetingList.ended")}</dt>
                  <dd>{formatLocalDate(meeting.endedAt)}</dd>
                </div>
              </dl>
            </div>
            <div className="meeting-row__actions">
              <AppLink
                className="button button--secondary"
                to={meetingPath(meeting.id)}
              >
                {t("meetingList.viewDetails")}
              </AppLink>
              {canRetryDelete && onRetryDelete ? (
                <button
                  className="button button--danger-outline"
                  disabled={deleting}
                  onClick={() => onRetryDelete(meeting.id)}
                  type="button"
                >
                  {deleting ? t("common.retrying") : t("common.retryDeletion")}
                </button>
              ) : onRequestDelete && onConfirmDelete && onCancelDelete ? (
                confirming ? (
                  <div
                    className="confirm-actions"
                    role="group"
                    aria-label={t("meetingList.confirmDeleting", {
                      title: meeting.title,
                    })}
                  >
                    <button
                      className="button button--danger"
                      disabled={deleting}
                      onClick={() => onConfirmDelete(meeting.id)}
                      type="button"
                    >
                      {deleting
                        ? t("common.deleting")
                        : t("common.confirmDelete")}
                    </button>
                    <button
                      className="button button--ghost"
                      disabled={deleting}
                      onClick={onCancelDelete}
                      type="button"
                    >
                      {t("common.cancel")}
                    </button>
                  </div>
                ) : (
                  <button
                    aria-describedby={
                      canDelete ? undefined : `delete-help-${meeting.id}`
                    }
                    className="button button--ghost"
                    disabled={!canDelete || deleting}
                    onClick={() => onRequestDelete(meeting.id)}
                    type="button"
                  >
                    {t("common.delete")}
                  </button>
                )
              ) : null}
              {!canDelete && !canRetryDelete && onRequestDelete ? (
                <p className="restriction" id={`delete-help-${meeting.id}`}>
                  {meeting.status === "deleting"
                    ? t("meetingList.deletionPending")
                    : t("meetingList.finishBeforeDelete")}
                </p>
              ) : null}
            </div>
          </article>
        );
      })}
    </div>
  );
}
