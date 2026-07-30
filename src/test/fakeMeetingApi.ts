import {
  MeetingApiError,
  type CreateMeetingRequest,
  type DeleteMeetingResponse,
  type ListMeetingsResponse,
  type MeetingApi,
  type MeetingResponse,
  type UpdateMeetingRequest,
} from "../api/meeting";

export const meetingIds = {
  draft: "11111111-1111-4111-8111-111111111111",
  recording: "22222222-2222-4222-8222-222222222222",
  ready: "33333333-3333-4333-8333-333333333333",
};

export function syntheticMeeting(
  overrides: Partial<MeetingResponse> = {},
): MeetingResponse {
  return {
    id: meetingIds.draft,
    title: "Synthetic planning meeting",
    status: "draft",
    primaryLanguage: "en-AU",
    selectedWindowJson: null,
    startedAt: null,
    endedAt: null,
    createdAt: "2026-07-30T01:00:00.000Z",
    updatedAt: "2026-07-30T01:00:00.000Z",
    ...overrides,
  };
}

export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

export function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

export class FakeMeetingApi implements MeetingApi {
  meetings: MeetingResponse[];
  createCalls: CreateMeetingRequest[] = [];
  updateCalls: UpdateMeetingRequest[] = [];
  startCalls: string[] = [];
  endCalls: string[] = [];
  deleteCalls: string[] = [];
  listError: unknown = null;
  getError: unknown = null;
  createError: unknown = null;
  updateError: unknown = null;
  startError: unknown = null;
  endError: unknown = null;
  deleteError: unknown = null;
  createDeferred: Deferred<MeetingResponse> | null = null;

  constructor(meetings: MeetingResponse[] = []) {
    this.meetings = meetings.map(cloneMeeting);
  }

  async create(request: CreateMeetingRequest): Promise<MeetingResponse> {
    this.createCalls.push({ ...request });
    if (this.createError) throw this.createError;
    if (this.createDeferred) return this.createDeferred.promise;

    const created = syntheticMeeting({
      id: "44444444-4444-4444-8444-444444444444",
      title: request.title.trim(),
      primaryLanguage: request.primaryLanguage,
    });
    this.meetings.push(created);
    return cloneMeeting(created);
  }

  async get(id: string): Promise<MeetingResponse> {
    if (this.getError) throw this.getError;
    const meeting = this.meetings.find((candidate) => candidate.id === id);
    if (!meeting) throw new MeetingApiError("meeting_not_found", false);
    return cloneMeeting(meeting);
  }

  async list(): Promise<ListMeetingsResponse> {
    if (this.listError) throw this.listError;
    return { meetings: this.meetings.map(cloneMeeting) };
  }

  async update(request: UpdateMeetingRequest): Promise<MeetingResponse> {
    this.updateCalls.push({ ...request });
    if (this.updateError) throw this.updateError;
    const meeting = this.requireMeeting(request.id);
    if (request.title !== undefined) meeting.title = request.title.trim();
    if (request.primaryLanguage !== undefined) {
      meeting.primaryLanguage = request.primaryLanguage.trim();
    }
    meeting.updatedAt = "2026-07-30T01:01:00.000Z";
    return cloneMeeting(meeting);
  }

  async start(id: string): Promise<MeetingResponse> {
    this.startCalls.push(id);
    if (this.startError) throw this.startError;
    const meeting = this.requireMeeting(id);
    if (meeting.status !== "draft") {
      throw new MeetingApiError("meeting_invalid_state", false);
    }
    meeting.status = "recording";
    meeting.startedAt = "2026-07-30T01:02:00.000Z";
    meeting.updatedAt = meeting.startedAt;
    return cloneMeeting(meeting);
  }

  async end(id: string): Promise<MeetingResponse> {
    this.endCalls.push(id);
    if (this.endError) throw this.endError;
    const meeting = this.requireMeeting(id);
    if (meeting.status !== "recording") {
      throw new MeetingApiError("meeting_invalid_state", false);
    }
    meeting.status = "ready";
    meeting.endedAt = "2026-07-30T01:03:00.000Z";
    meeting.updatedAt = meeting.endedAt;
    return cloneMeeting(meeting);
  }

  async delete(id: string): Promise<DeleteMeetingResponse> {
    this.deleteCalls.push(id);
    if (this.deleteError) throw this.deleteError;
    const meeting = this.requireMeeting(id);
    if (meeting.status !== "draft" && meeting.status !== "ready") {
      throw new MeetingApiError("meeting_invalid_state", false);
    }
    this.meetings = this.meetings.filter((candidate) => candidate.id !== id);
    return { id, deleted: true };
  }

  private requireMeeting(id: string): MeetingResponse {
    const meeting = this.meetings.find((candidate) => candidate.id === id);
    if (!meeting) throw new MeetingApiError("meeting_not_found", false);
    return meeting;
  }
}

function cloneMeeting(meeting: MeetingResponse): MeetingResponse {
  return { ...meeting };
}
