export function transcriptWorkspacePath(meetingId: string): string {
  return `/meetings/${encodeURIComponent(meetingId)}/transcript`;
}

export function summaryMinutesPath(
  meetingId: string,
  summaryId?: string,
): string {
  const base = `/meetings/${encodeURIComponent(meetingId)}/minutes`;
  return summaryId ? `${base}/${encodeURIComponent(summaryId)}` : base;
}
