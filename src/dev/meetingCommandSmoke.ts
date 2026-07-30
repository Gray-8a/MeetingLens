import { meetingApi, MeetingApiError } from "../api/meeting";

const smokeMeetingKey = "meetinglens.command-api-smoke.meeting-id";
const smokeTitle = "Synthetic command smoke meeting";
const updatedSmokeTitle = "Synthetic updated command smoke meeting";

export async function runMeetingCommandSmoke(): Promise<void> {
  try {
    const existingId = window.localStorage.getItem(smokeMeetingKey);
    if (existingId === null) {
      await runFirstLaunch();
    } else {
      await runSecondLaunch(existingId);
    }
  } catch (error: unknown) {
    const code =
      error instanceof MeetingApiError ? error.code : "internal_error";
    document.title = `MeetingLens smoke: failed:${code}`;
  }
}

async function runFirstLaunch(): Promise<void> {
  const created = await meetingApi.create({
    title: smokeTitle,
    primaryLanguage: "en-AU",
  });
  requireCondition(created.status === "draft");

  const listed = await meetingApi.list();
  requireCondition(
    listed.meetings.some((meeting) => meeting.id === created.id),
  );

  const fetched = await meetingApi.get(created.id);
  requireCondition(fetched.status === "draft");

  const updated = await meetingApi.update({
    id: created.id,
    title: updatedSmokeTitle,
  });
  requireCondition(updated.title === updatedSmokeTitle);
  requireCondition(updated.status === "draft");

  const recording = await meetingApi.start(created.id);
  requireCondition(recording.status === "recording");
  requireCondition(recording.startedAt !== null);

  await expectMeetingError(
    meetingApi.delete(created.id),
    "meeting_invalid_state",
  );

  const ready = await meetingApi.end(created.id);
  requireCondition(ready.status === "ready");
  requireCondition(ready.startedAt !== null);
  requireCondition(ready.endedAt !== null);
  requireCondition(ready.endedAt >= ready.startedAt);

  window.localStorage.setItem(smokeMeetingKey, created.id);
  document.title = "MeetingLens smoke: restart";
}

async function runSecondLaunch(id: string): Promise<void> {
  const ready = await meetingApi.get(id);
  requireCondition(ready.status === "ready");
  requireCondition(ready.title === updatedSmokeTitle);

  const listed = await meetingApi.list();
  requireCondition(listed.meetings.some((meeting) => meeting.id === id));

  const deleted = await meetingApi.delete(id);
  requireCondition(deleted.id === id && deleted.deleted);
  await expectMeetingError(meetingApi.get(id), "meeting_not_found");

  window.localStorage.removeItem(smokeMeetingKey);
  document.title = "MeetingLens smoke: passed";
}

async function expectMeetingError(
  promise: Promise<unknown>,
  expectedCode: "meeting_invalid_state" | "meeting_not_found",
): Promise<void> {
  try {
    await promise;
  } catch (error: unknown) {
    if (error instanceof MeetingApiError && error.code === expectedCode) {
      return;
    }
    throw error;
  }
  throw new Error("smoke_operation_unexpectedly_succeeded");
}

function requireCondition(condition: boolean): asserts condition {
  if (!condition) {
    throw new Error("smoke_assertion_failed");
  }
}
