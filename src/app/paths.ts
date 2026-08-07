export function transcriptWorkspacePath(meetingId: string): string {
  return `/meetings/${encodeURIComponent(meetingId)}/transcript`;
}
