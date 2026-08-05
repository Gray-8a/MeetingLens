import {
  MeetingApiError,
  type MeetingErrorCode,
  type MeetingStatus,
} from "../api/meeting";
import i18n from "../i18n";
export { formatDuration, formatLocalDate } from "../i18n/formatters";

const statusKeys: Record<MeetingStatus, keyof typeof statusTranslations> = {
  draft: "draft",
  recording: "recording",
  processing: "processing",
  ready: "ready",
  failed: "failed",
  archived: "archived",
  deleting: "deleting",
  delete_failed: "delete_failed",
};

const statusTranslations = {
  draft: "meeting.status.draft",
  recording: "meeting.status.recording",
  processing: "meeting.status.processing",
  ready: "meeting.status.ready",
  failed: "meeting.status.failed",
  archived: "meeting.status.archived",
  deleting: "meeting.status.deleting",
  delete_failed: "meeting.status.delete_failed",
} as const;

const errorTranslations: Record<
  MeetingErrorCode,
  keyof typeof errorTranslationKeys
> = {
  validation_error: "validation_error",
  invalid_meeting_id: "invalid_meeting_id",
  meeting_not_found: "meeting_not_found",
  meeting_invalid_state: "meeting_invalid_state",
  meeting_delete_failed: "meeting_delete_failed",
  meeting_delete_pending: "meeting_delete_pending",
  database_unavailable: "database_unavailable",
  persistence_error: "persistence_error",
  internal_error: "internal_error",
};

const errorTranslationKeys = {
  validation_error: "meeting.error.validation_error",
  invalid_meeting_id: "meeting.error.invalid_meeting_id",
  meeting_not_found: "meeting.error.meeting_not_found",
  meeting_invalid_state: "meeting.error.meeting_invalid_state",
  meeting_delete_failed: "meeting.error.meeting_delete_failed",
  meeting_delete_pending: "meeting.error.meeting_delete_pending",
  database_unavailable: "meeting.error.database_unavailable",
  persistence_error: "meeting.error.persistence_error",
  internal_error: "meeting.error.internal_error",
} as const;

export function meetingErrorMessage(code: MeetingErrorCode): string {
  return i18n.t(errorTranslationKeys[errorTranslations[code]]);
}

export function statusLabel(status: string): string {
  return status in statusKeys
    ? i18n.t(statusTranslations[statusKeys[status as MeetingStatus]])
    : i18n.t("meeting.status.unknown");
}

export function uiError(error: unknown): {
  code: MeetingErrorCode;
} {
  const code = error instanceof MeetingApiError ? error.code : "internal_error";
  return { code };
}
