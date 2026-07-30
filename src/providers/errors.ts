import type { ProviderKind, ProviderOperation } from "./types";

export type ProviderErrorCode =
  | "provider_not_ready"
  | "provider_already_running"
  | "provider_invalid_state"
  | "provider_initialization_failed"
  | "provider_operation_failed"
  | "provider_cancelled"
  | "provider_disposed"
  | "provider_unavailable"
  | "unsupported_capability"
  | "internal_error";

const safeMessages: Record<ProviderErrorCode, string> = {
  provider_not_ready: "The provider is not ready.",
  provider_already_running: "The provider is already running.",
  provider_invalid_state: "The provider cannot perform that operation now.",
  provider_initialization_failed: "The provider could not be initialized.",
  provider_operation_failed: "The provider could not complete that operation.",
  provider_cancelled: "The provider operation was cancelled.",
  provider_disposed: "The provider is no longer available.",
  provider_unavailable: "The provider is unavailable.",
  unsupported_capability: "That provider capability is not supported.",
  internal_error: "An internal provider error occurred.",
};

export class ProviderError extends Error {
  readonly code: ProviderErrorCode;
  readonly retryable: boolean;
  readonly provider: ProviderKind;
  readonly operation: ProviderOperation;

  constructor(
    code: ProviderErrorCode,
    provider: ProviderKind,
    operation: ProviderOperation,
    retryable = false,
  ) {
    super(safeMessages[code]);
    this.name = "ProviderError";
    this.code = code;
    this.retryable = retryable;
    this.provider = provider;
    this.operation = operation;
  }
}

export function toProviderError(
  value: unknown,
  provider: ProviderKind,
  operation: ProviderOperation,
): ProviderError {
  if (value instanceof ProviderError) return value;
  return new ProviderError("internal_error", provider, operation);
}
