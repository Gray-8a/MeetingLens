import secretSource from "./secret.ts?raw";
import {
  createSecretApi,
  SecretApiError,
  secretCommandNames,
  type InvokeAdapter,
  type SecretStatus,
} from "./secret";

const status: SecretStatus = {
  kind: "deepseek_api_key",
  configured: false,
  available: true,
  backend: "windows_credential_manager",
};

describe("secretApi command boundary", () => {
  it("uses only the three approved commands with correctly serialized args", async () => {
    const calls: Array<{
      command: string;
      args: Record<string, unknown> | undefined;
    }> = [];
    const generatedInput = crypto.randomUUID();
    const adapter: InvokeAdapter = {
      invoke: async <T>(command: string, args?: Record<string, unknown>) => {
        calls.push({ command, args });
        if (command === secretCommandNames.delete) {
          return {
            kind: "deepseek_api_key",
            deleted: true,
            configured: false,
          } as T;
        }
        return {
          ...status,
          configured: command === secretCommandNames.set,
        } as T;
      },
    };
    const api = createSecretApi(adapter);

    await api.getStatus("deepseek_api_key");
    await api.set("deepseek_api_key", generatedInput);
    await api.delete("deepseek_api_key");

    expect(calls).toEqual([
      {
        command: "get_secret_status",
        args: { request: { kind: "deepseek_api_key" } },
      },
      {
        command: "set_secret",
        args: {
          request: { kind: "deepseek_api_key", value: generatedInput },
        },
      },
      {
        command: "delete_secret",
        args: { request: { kind: "deepseek_api_key" } },
      },
    ]);
  });

  it("does not provide get, reveal, export, or copy methods", () => {
    const api = createSecretApi(adapterReturning(status));
    const keys = Object.keys(api);
    expect(keys).toEqual(["getStatus", "set", "delete"]);
    expect(keys).not.toEqual(
      expect.arrayContaining(["get", "reveal", "export", "copy"]),
    );
  });

  it("maps stable backend errors without using backend messages", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject({
          code: "secret_access_denied",
          message: "untrusted native details",
          retryable: false,
        }),
    };

    await expect(
      createSecretApi(adapter).set("deepseek_api_key", crypto.randomUUID()),
    ).rejects.toMatchObject({
      code: "secret_access_denied",
      message: "Secure local storage denied access.",
      retryable: false,
    });
  });

  it("converts unknown failures into a safe internal error", async () => {
    const adapter: InvokeAdapter = {
      invoke: async () =>
        Promise.reject({
          credentialBlob: "private",
          targetName: "private",
          nativeError: "private",
        }),
    };

    const error = await createSecretApi(adapter)
      .getStatus("deepseek_api_key")
      .catch((reason: unknown) => reason);
    expect(error).toBeInstanceOf(SecretApiError);
    expect(error).toMatchObject({
      code: "internal_error",
      message: "MeetingLens could not complete that action.",
    });
    expect(JSON.stringify(error)).not.toContain("private");
  });

  it("rejects malformed success responses safely", async () => {
    const api = createSecretApi(
      adapterReturning({ ...status, backend: "unexpected" }),
    );
    await expect(api.getStatus("deepseek_api_key")).rejects.toMatchObject({
      code: "internal_error",
    });
  });

  it("contains no browser storage, console output, explicit any, or readback command", () => {
    expect(secretSource).not.toMatch(/localStorage|sessionStorage|indexedDB/i);
    expect(secretSource).not.toMatch(/console\./);
    expect(secretSource).not.toMatch(/\bany\b/);
    expect(secretSource).not.toMatch(
      /["'](?:get|reveal|export|copy)_secret["']/,
    );
  });
});

function adapterReturning(value: unknown): InvokeAdapter {
  return {
    invoke: async <T>() => value as T,
  };
}
