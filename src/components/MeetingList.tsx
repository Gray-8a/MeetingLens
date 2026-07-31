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
  return (
    <div className="meeting-list" aria-label="Meetings">
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
                  <dt>Created</dt>
                  <dd>{formatLocalDate(meeting.createdAt)}</dd>
                </div>
                <div>
                  <dt>Started</dt>
                  <dd>{formatLocalDate(meeting.startedAt)}</dd>
                </div>
                <div>
                  <dt>Ended</dt>
                  <dd>{formatLocalDate(meeting.endedAt)}</dd>
                </div>
              </dl>
            </div>
            <div className="meeting-row__actions">
              <AppLink
                className="button button--secondary"
                to={meetingPath(meeting.id)}
              >
                View details
              </AppLink>
              {canRetryDelete && onRetryDelete ? (
                <button
                  className="button button--danger-outline"
                  disabled={deleting}
                  onClick={() => onRetryDelete(meeting.id)}
                  type="button"
                >
                  {deleting ? "Retrying…" : "Retry deletion"}
                </button>
              ) : onRequestDelete && onConfirmDelete && onCancelDelete ? (
                confirming ? (
                  <div
                    className="confirm-actions"
                    role="group"
                    aria-label={`Confirm deleting ${meeting.title}`}
                  >
                    <button
                      className="button button--danger"
                      disabled={deleting}
                      onClick={() => onConfirmDelete(meeting.id)}
                      type="button"
                    >
                      {deleting ? "Deleting…" : "Confirm delete"}
                    </button>
                    <button
                      className="button button--ghost"
                      disabled={deleting}
                      onClick={onCancelDelete}
                      type="button"
                    >
                      Cancel
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
                    Delete
                  </button>
                )
              ) : null}
              {!canDelete && !canRetryDelete && onRequestDelete ? (
                <p className="restriction" id={`delete-help-${meeting.id}`}>
                  {meeting.status === "deleting"
                    ? "Deletion pending · meeting_delete_pending"
                    : "Finish the meeting before deleting it · meeting_invalid_state"}
                </p>
              ) : null}
            </div>
          </article>
        );
      })}
    </div>
  );
}
