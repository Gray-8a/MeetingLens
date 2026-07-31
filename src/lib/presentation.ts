import {
  MeetingApiError,
  type MeetingErrorCode,
  type MeetingStatus,
} from "../api/meeting";

const statusLabels: Record<MeetingStatus, string> = {
  draft: "Draft",
  recording: "Recording",
  processing: "Processing",
  ready: "Ready",
  failed: "Failed",
  archived: "Archived",
  deleting: "Deleting",
  delete_failed: "Delete failed",
};

const errorMessages: Record<MeetingErrorCode, string> = {
  validation_error: "Check the meeting details and try again.",
  invalid_meeting_id: "This meeting link is invalid.",
  meeting_not_found: "This meeting no longer exists.",
  meeting_invalid_state:
    "This action is not available in the current meeting state.",
  meeting_delete_failed: "Meeting deletion failed. Try again.",
  meeting_delete_pending: "Meeting deletion is already in progress.",
  database_unavailable: "Local meeting storage is unavailable.",
  persistence_error: "Meeting data could not be saved. Try again.",
  internal_error: "MeetingLens could not complete that action.",
};

export function statusLabel(status: string): string {
  return status in statusLabels
    ? statusLabels[status as MeetingStatus]
    : "Unknown status";
}

export function uiError(error: unknown): {
  code: MeetingErrorCode;
  message: string;
} {
  const code = error instanceof MeetingApiError ? error.code : "internal_error";
  return { code, message: errorMessages[code] };
}

export function formatLocalDate(value: string | null): string {
  if (value === null) return "Not set";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Unavailable";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds]
    .map((part) => part.toString().padStart(2, "0"))
    .join(":");
}
