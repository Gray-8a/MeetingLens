import meetingSource from "./meeting.ts?raw";
import {
  createMeetingApi,
  MeetingApiError,
  meetingCommandNames,
  type InvokeAdapter,
  type MeetingResponse,
} from "./meeting";

const meetingId = "9f59d7ba-31f1-42ea-89d8-8f1d64a1ef60";

function meeting(overrides: Partial<MeetingResponse> = {}): MeetingResponse {
  return {
    id: meetingId,
    title: "Synthetic API meeting",
    status: "draft",
    primaryLanguage: "en-AU",
    selectedWindowJson: null,
    startedAt: null,
    endedAt: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function adapterReturning(value: unknown): InvokeAdapter {
  return { invoke: async () => value };
}

describe("meetingApi command routing", () => {
  const cases: Array<{
    name: keyof typeof meetingCommandNames;
    run: (api: ReturnType<typeof createMeetingApi>) => Promise<unknown>;
    expectedArgs: Record<string, unknown> | undefined;
    response: unknown;
  }> = [
    {
      name: "create",
      run: (api) =>
        api.create({
          title: "Synthetic API meeting",
          primaryLanguage: "en-AU",
        }),
      expectedArgs: {
        request: { title: "Synthetic API meeting", primaryLanguage: "en-AU" },
      },
      response: meeting(),
    },
    {
      name: "get",
      run: (api) => api.get(meetingId),
      expectedArgs: { request: { id: meetingId } },
      response: meeting(),
    },
    {
      name: "list",
      run: (api) => api.list(),
      expectedArgs: undefined,
      response: { meetings: [meeting()] },
    },
    {
      name: "update",
      run: (api) =>
        api.update({ id: meetingId, title: "Synthetic updated meeting" }),
      expectedArgs: {
        request: { id: meetingId, title: "Synthetic updated meeting" },
      },
      response: meeting({ title: "Synthetic updated meeting" }),
    },
    {
      name: "start",
      run: (api) => api.start(meetingId),
      expectedArgs: { request: { id: meetingId } },
      response: meeting({
        status: "recording",
        startedAt: "2026-01-01T00:00:01.000Z",
      }),
    },
    {
      name: "end",
      run: (api) => api.end(meetingId),
      expectedArgs: { request: { id: meetingId } },
      response: meeting({
        status: "ready",
        startedAt: "2026-01-01T00:00:01.000Z",
        endedAt: "2026-01-01T00:00:02.000Z",
      }),
    },
    {
      name: "delete",
      run: (api) => api.delete(meetingId),
      expectedArgs: { request: { id: meetingId } },
      response: { id: meetingId, deleted: true },
    },
  ];

  it.each(cases)("uses the centralized $name command", async (testCase) => {
    const calls: Array<{
      command: string;
      args: Record<string, unknown> | undefined;
    }> = [];
    const adapter: InvokeAdapter = {
      invoke: async (command, args) => {
        calls.push({ command, args });
        return testCase.response;
      },
    };
    const api = createMeetingApi(adapter);

    await testCase.run(api);

    expect(calls).toEqual([
      {
        command: meetingCommandNames[testCase.name],
        args: testCase.expectedArgs,
      },
    ]);
  });
});

describe("meetingApi response and error handling", () => {
  it("parses successful responses with null lifecycle times", async () => {
    const result = await createMeetingApi(adapterReturning(meeting())).get(
      meetingId,
    );

    expect(result.startedAt).toBeNull();
    expect(result.endedAt).toBeNull();
    expect(result.primaryLanguage).toBe("en-AU");
  });

  it("parses list responses through the same meeting validator", async () => {
    const result = await createMeetingApi(
      adapterReturning({ meetings: [meeting(), meeting({ id: "second" })] }),
    ).list();

    expect(result.meetings).toHaveLength(2);
  });

  it("converts meeting_not_found without inspecting its message", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject({
          code: "meeting_not_found",
          message: "untrusted backend wording",
          retryable: false,
        }),
    };

    await expect(
      createMeetingApi(adapter).get(meetingId),
    ).rejects.toMatchObject({
      code: "meeting_not_found",
      message: "The meeting was not found.",
      retryable: false,
    });
  });

  it("converts meeting_invalid_state to the typed error", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject({
          code: "meeting_invalid_state",
          message: "ignored",
          retryable: false,
        }),
    };

    await expect(
      createMeetingApi(adapter).delete(meetingId),
    ).rejects.toBeInstanceOf(MeetingApiError);
    await expect(
      createMeetingApi(adapter).delete(meetingId),
    ).rejects.toMatchObject({ code: "meeting_invalid_state" });
  });

  it("turns invoke rejection into a safe internal error", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject(new Error("private path and raw database details")),
    };

    await expect(
      createMeetingApi(adapter).get(meetingId),
    ).rejects.toMatchObject({
      code: "internal_error",
      message: "An internal error occurred.",
      retryable: false,
    });
  });

  it("does not expose an unknown rejected object", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject({
          sql: "private query",
          path: "private database path",
          backtrace: "private stack",
        }),
    };

    const error = await createMeetingApi(adapter)
      .get(meetingId)
      .catch((reason: unknown) => reason);
    expect(error).toEqual(
      expect.objectContaining({
        code: "internal_error",
        message: "An internal error occurred.",
      }),
    );
    expect(JSON.stringify(error)).not.toContain("private");
  });

  it("rejects malformed success payloads safely", async () => {
    const api = createMeetingApi(adapterReturning({ id: meetingId }));

    await expect(api.get(meetingId)).rejects.toMatchObject({
      code: "internal_error",
      message: "An internal error occurred.",
    });
  });

  it("supports an injected adapter without accessing the Tauri runtime", async () => {
    const api = createMeetingApi(adapterReturning(meeting()));

    await expect(api.get(meetingId)).resolves.toEqual(meeting());
  });

  it("contains no explicit any type", () => {
    expect(meetingSource).not.toMatch(/\bany\b/);
  });
});
