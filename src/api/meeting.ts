import { invoke } from "@tauri-apps/api/core";

export const meetingCommandNames = {
  create: "create_meeting",
  get: "get_meeting",
  list: "list_meetings",
  update: "update_meeting",
  start: "start_meeting",
  end: "end_meeting",
  delete: "delete_meeting",
} as const;

export type MeetingStatus =
  | "draft"
  | "recording"
  | "processing"
  | "ready"
  | "failed"
  | "archived"
  | "deleting"
  | "delete_failed";

export interface CreateMeetingRequest {
  title: string;
  primaryLanguage: string;
}

export interface UpdateMeetingRequest {
  id: string;
  title?: string;
  primaryLanguage?: string;
}

export interface MeetingResponse {
  id: string;
  title: string;
  status: MeetingStatus;
  primaryLanguage: string;
  selectedWindowJson: string | null;
  startedAt: string | null;
  endedAt: string | null;
  createdAt: string;
  updatedAt: string;
  deletionErrorCode: string | null;
}

export interface ListMeetingsResponse {
  meetings: MeetingResponse[];
}

export interface DeleteMeetingResponse {
  id: string;
  deleted: boolean;
}

export type MeetingErrorCode =
  | "validation_error"
  | "invalid_meeting_id"
  | "meeting_not_found"
  | "meeting_invalid_state"
  | "meeting_delete_failed"
  | "meeting_delete_pending"
  | "database_unavailable"
  | "persistence_error"
  | "internal_error";

const safeMessages: Record<MeetingErrorCode, string> = {
  validation_error: "The meeting details are invalid.",
  invalid_meeting_id: "The meeting ID is invalid.",
  meeting_not_found: "The meeting was not found.",
  meeting_invalid_state: "The meeting cannot be changed in its current state.",
  meeting_delete_failed: "The meeting could not be deleted safely.",
  meeting_delete_pending: "The meeting deletion is already in progress.",
  database_unavailable: "Meeting storage is unavailable.",
  persistence_error: "Meeting data could not be saved.",
  internal_error: "An internal error occurred.",
};

const errorCodes = new Set<MeetingErrorCode>(
  Object.keys(safeMessages) as MeetingErrorCode[],
);

const meetingStatuses = new Set<MeetingStatus>([
  "draft",
  "recording",
  "processing",
  "ready",
  "failed",
  "archived",
  "deleting",
  "delete_failed",
]);

export class MeetingApiError extends Error {
  readonly code: MeetingErrorCode;
  readonly retryable: boolean;

  constructor(code: MeetingErrorCode, retryable: boolean) {
    super(safeMessages[code]);
    this.name = "MeetingApiError";
    this.code = code;
    this.retryable = retryable;
  }
}

export type InvokeArguments = Record<string, unknown>;

export interface InvokeAdapter {
  invoke(command: string, args?: InvokeArguments): Promise<unknown>;
}

const tauriInvokeAdapter: InvokeAdapter = {
  invoke: (command, args) => invoke<unknown>(command, args),
};

export interface MeetingApi {
  create(request: CreateMeetingRequest): Promise<MeetingResponse>;
  get(id: string): Promise<MeetingResponse>;
  list(): Promise<ListMeetingsResponse>;
  update(request: UpdateMeetingRequest): Promise<MeetingResponse>;
  start(id: string): Promise<MeetingResponse>;
  end(id: string): Promise<MeetingResponse>;
  delete(id: string): Promise<DeleteMeetingResponse>;
}

export function createMeetingApi(
  adapter: InvokeAdapter = tauriInvokeAdapter,
): MeetingApi {
  return {
    create: (request) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.create,
        { request },
        parseMeetingResponse,
      ),
    get: (id) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.get,
        { request: { id } },
        parseMeetingResponse,
      ),
    list: () =>
      invokeAndParse(adapter, meetingCommandNames.list, undefined, parseList),
    update: (request) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.update,
        { request },
        parseMeetingResponse,
      ),
    start: (id) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.start,
        { request: { id } },
        parseMeetingResponse,
      ),
    end: (id) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.end,
        { request: { id } },
        parseMeetingResponse,
      ),
    delete: (id) =>
      invokeAndParse(
        adapter,
        meetingCommandNames.delete,
        { request: { id } },
        parseDelete,
      ),
  };
}

export const meetingApi = createMeetingApi();

async function invokeAndParse<T>(
  adapter: InvokeAdapter,
  command: string,
  args: InvokeArguments | undefined,
  parse: (value: unknown) => T,
): Promise<T> {
  try {
    return parse(await adapter.invoke(command, args));
  } catch (error: unknown) {
    if (error instanceof MeetingApiError) {
      throw error;
    }
    throw parseMeetingError(error);
  }
}

export function parseMeetingResponse(value: unknown): MeetingResponse {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      "id",
      "title",
      "status",
      "primaryLanguage",
      "selectedWindowJson",
      "startedAt",
      "endedAt",
      "createdAt",
      "updatedAt",
      "deletionErrorCode",
    ])
  ) {
    throw internalError();
  }

  const status = value.status;
  if (
    typeof value.id !== "string" ||
    typeof value.title !== "string" ||
    typeof status !== "string" ||
    !meetingStatuses.has(status as MeetingStatus) ||
    typeof value.primaryLanguage !== "string" ||
    !isNullableString(value.selectedWindowJson) ||
    !isNullableString(value.startedAt) ||
    !isNullableString(value.endedAt) ||
    typeof value.createdAt !== "string" ||
    typeof value.updatedAt !== "string" ||
    !isNullableString(value.deletionErrorCode)
  ) {
    throw internalError();
  }

  return {
    id: value.id,
    title: value.title,
    status: status as MeetingStatus,
    primaryLanguage: value.primaryLanguage,
    selectedWindowJson: value.selectedWindowJson,
    startedAt: value.startedAt,
    endedAt: value.endedAt,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
    deletionErrorCode: value.deletionErrorCode,
  };
}

function parseList(value: unknown): ListMeetingsResponse {
  if (!isRecord(value) || !Array.isArray(value.meetings)) {
    throw internalError();
  }
  return { meetings: value.meetings.map(parseMeetingResponse) };
}

function parseDelete(value: unknown): DeleteMeetingResponse {
  if (
    !isRecord(value) ||
    typeof value.id !== "string" ||
    typeof value.deleted !== "boolean"
  ) {
    throw internalError();
  }
  return { id: value.id, deleted: value.deleted };
}

function parseMeetingError(value: unknown): MeetingApiError {
  if (
    isRecord(value) &&
    typeof value.code === "string" &&
    errorCodes.has(value.code as MeetingErrorCode) &&
    typeof value.retryable === "boolean"
  ) {
    return new MeetingApiError(value.code as MeetingErrorCode, value.retryable);
  }
  return internalError();
}

function internalError(): MeetingApiError {
  return new MeetingApiError("internal_error", false);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const allowed = new Set(keys);
  return Object.keys(value).every((key) => allowed.has(key));
}
