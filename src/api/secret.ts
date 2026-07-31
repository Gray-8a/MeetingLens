import { invoke } from "@tauri-apps/api/core";

export const secretCommandNames = {
  getStatus: "get_secret_status",
  set: "set_secret",
  delete: "delete_secret",
} as const;

export type SecretKind = "deepseek_api_key";
export type SecretBackend = "windows_credential_manager";

export interface SecretStatus {
  kind: SecretKind;
  configured: boolean;
  available: boolean;
  backend: SecretBackend;
}

export interface DeleteSecretResult {
  kind: SecretKind;
  deleted: boolean;
  configured: boolean;
}

export type SecretErrorCode =
  | "secret_invalid_input"
  | "secret_too_large"
  | "secret_store_unavailable"
  | "secret_write_failed"
  | "secret_read_failed"
  | "secret_delete_failed"
  | "secret_access_denied"
  | "secret_unsupported_platform"
  | "internal_error";

const safeMessages: Record<SecretErrorCode, string> = {
  secret_invalid_input: "Enter a non-empty API key.",
  secret_too_large: "The API key is too large.",
  secret_store_unavailable: "Secure local storage is unavailable.",
  secret_write_failed: "The API key could not be saved.",
  secret_read_failed: "The API key status could not be read.",
  secret_delete_failed: "The API key could not be deleted.",
  secret_access_denied: "Secure local storage denied access.",
  secret_unsupported_platform:
    "Secure local storage is unavailable on this platform.",
  internal_error: "MeetingLens could not complete that action.",
};

const secretErrorCodes = new Set<SecretErrorCode>(
  Object.keys(safeMessages) as SecretErrorCode[],
);

export class SecretApiError extends Error {
  constructor(
    readonly code: SecretErrorCode,
    readonly retryable: boolean,
  ) {
    super(safeMessages[code]);
    this.name = "SecretApiError";
  }
}

export interface InvokeAdapter {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

export interface SecretApi {
  getStatus(kind: SecretKind): Promise<SecretStatus>;
  set(kind: SecretKind, value: string): Promise<SecretStatus>;
  delete(kind: SecretKind): Promise<DeleteSecretResult>;
}

const tauriAdapter: InvokeAdapter = { invoke };

export function createSecretApi(adapter: InvokeAdapter): SecretApi {
  return {
    async getStatus(kind) {
      try {
        return parseStatus(
          await adapter.invoke(secretCommandNames.getStatus, {
            request: { kind },
          }),
        );
      } catch (reason: unknown) {
        throw toSecretError(reason);
      }
    },
    async set(kind, value) {
      try {
        return parseStatus(
          await adapter.invoke(secretCommandNames.set, {
            request: { kind, value },
          }),
        );
      } catch (reason: unknown) {
        throw toSecretError(reason);
      }
    },
    async delete(kind) {
      try {
        return parseDelete(
          await adapter.invoke(secretCommandNames.delete, {
            request: { kind },
          }),
        );
      } catch (reason: unknown) {
        throw toSecretError(reason);
      }
    },
  };
}

export const secretApi = createSecretApi(tauriAdapter);

export function toSecretError(value: unknown): SecretApiError {
  if (value instanceof SecretApiError) return value;
  if (
    isRecord(value) &&
    typeof value.code === "string" &&
    secretErrorCodes.has(value.code as SecretErrorCode) &&
    typeof value.retryable === "boolean"
  ) {
    return new SecretApiError(value.code as SecretErrorCode, value.retryable);
  }
  return new SecretApiError("internal_error", false);
}

function parseStatus(value: unknown): SecretStatus {
  if (
    !isRecord(value) ||
    value.kind !== "deepseek_api_key" ||
    typeof value.configured !== "boolean" ||
    typeof value.available !== "boolean" ||
    value.backend !== "windows_credential_manager"
  ) {
    throw new SecretApiError("internal_error", false);
  }
  return {
    kind: value.kind,
    configured: value.configured,
    available: value.available,
    backend: value.backend,
  };
}

function parseDelete(value: unknown): DeleteSecretResult {
  if (
    !isRecord(value) ||
    value.kind !== "deepseek_api_key" ||
    typeof value.deleted !== "boolean" ||
    typeof value.configured !== "boolean"
  ) {
    throw new SecretApiError("internal_error", false);
  }
  return {
    kind: value.kind,
    deleted: value.deleted,
    configured: value.configured,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
