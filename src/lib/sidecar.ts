import { invoke } from "@tauri-apps/api/core";

export const sidecarCommandName = "check_sidecar_health";

export interface SidecarStatus {
  status: "authenticated";
  binding: "loopback";
  session: "current";
  secretStorage: "not_implemented";
  service: "meetinglens-sidecar";
  version: string;
  apiVersion: "v1";
}

export type SidecarErrorCode =
  | "sidecar_unauthorized"
  | "sidecar_unavailable"
  | "sidecar_start_failed"
  | "sidecar_request_failed"
  | "sidecar_timeout"
  | "sidecar_invalid_response"
  | "internal_error";

const safeMessages: Record<SidecarErrorCode, string> = {
  sidecar_unauthorized: "The local service could not authenticate.",
  sidecar_unavailable: "The local service is unavailable.",
  sidecar_start_failed: "The local service could not be started.",
  sidecar_request_failed: "The local service request failed.",
  sidecar_timeout: "The local service did not respond in time.",
  sidecar_invalid_response: "The local service returned an invalid response.",
  internal_error: "An internal local service error occurred.",
};

const sidecarErrorCodes = new Set<SidecarErrorCode>(
  Object.keys(safeMessages) as SidecarErrorCode[],
);

export class SidecarApiError extends Error {
  constructor(
    readonly code: SidecarErrorCode,
    readonly retryable: boolean,
  ) {
    super(safeMessages[code]);
    this.name = "SidecarApiError";
  }
}

export function toSidecarError(value: unknown): SidecarApiError {
  if (value instanceof SidecarApiError) return value;
  if (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    typeof value.code === "string" &&
    sidecarErrorCodes.has(value.code as SidecarErrorCode)
  ) {
    return new SidecarApiError(
      value.code as SidecarErrorCode,
      "retryable" in value && value.retryable === true,
    );
  }
  return new SidecarApiError("internal_error", false);
}

export async function checkSidecarHealth(): Promise<SidecarStatus> {
  try {
    return await invoke<SidecarStatus>(sidecarCommandName);
  } catch (reason: unknown) {
    throw toSidecarError(reason);
  }
}
