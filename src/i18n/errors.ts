import type { DocumentErrorCode } from "../api/document";
import type { SecretErrorCode } from "../api/secret";
import type { SidecarErrorCode } from "../lib/sidecar";
import type { ProviderErrorCode } from "../providers/errors";
import i18n from ".";

const documentErrorKeys = {
  document_invalid_type: "error.document.document_invalid_type",
  document_type_mismatch: "error.document.document_type_mismatch",
  document_empty: "error.document.document_empty",
  document_too_large: "error.document.document_too_large",
  document_quota_exceeded: "error.document.document_quota_exceeded",
  document_filename_too_long: "error.document.document_filename_too_long",
  document_import_failed: "error.document.document_import_failed",
  document_not_found: "error.document.document_not_found",
  document_already_imported: "error.document.document_already_imported",
  document_invalid_state: "error.document.document_invalid_state",
  document_storage_unavailable: "error.document.document_storage_unavailable",
  document_integrity_failed: "error.document.document_integrity_failed",
  document_delete_pending: "error.document.document_delete_pending",
  document_delete_failed: "error.document.document_delete_failed",
  meeting_delete_pending: "error.document.meeting_delete_pending",
  meeting_delete_failed: "error.document.meeting_delete_failed",
  meeting_invalid_state: "error.document.meeting_invalid_state",
  internal_error: "error.document.internal_error",
} as const satisfies Record<DocumentErrorCode, string>;

const secretErrorKeys = {
  secret_invalid_input: "error.secret.secret_invalid_input",
  secret_too_large: "error.secret.secret_too_large",
  secret_store_unavailable: "error.secret.secret_store_unavailable",
  secret_write_failed: "error.secret.secret_write_failed",
  secret_read_failed: "error.secret.secret_read_failed",
  secret_delete_failed: "error.secret.secret_delete_failed",
  secret_access_denied: "error.secret.secret_access_denied",
  secret_unsupported_platform: "error.secret.secret_unsupported_platform",
  internal_error: "error.secret.internal_error",
} as const satisfies Record<SecretErrorCode, string>;

const providerErrorKeys = {
  provider_not_ready: "error.provider.provider_not_ready",
  provider_already_running: "error.provider.provider_already_running",
  provider_invalid_state: "error.provider.provider_invalid_state",
  provider_initialization_failed:
    "error.provider.provider_initialization_failed",
  provider_operation_failed: "error.provider.provider_operation_failed",
  provider_cancelled: "error.provider.provider_cancelled",
  provider_disposed: "error.provider.provider_disposed",
  provider_unavailable: "error.provider.provider_unavailable",
  unsupported_capability: "error.provider.unsupported_capability",
  internal_error: "error.provider.internal_error",
} as const satisfies Record<ProviderErrorCode, string>;

const sidecarErrorKeys = {
  sidecar_unauthorized: "error.sidecar.sidecar_unauthorized",
  sidecar_unavailable: "error.sidecar.sidecar_unavailable",
  sidecar_start_failed: "error.sidecar.sidecar_start_failed",
  sidecar_request_failed: "error.sidecar.sidecar_request_failed",
  sidecar_timeout: "error.sidecar.sidecar_timeout",
  sidecar_invalid_response: "error.sidecar.sidecar_invalid_response",
  internal_error: "error.sidecar.internal_error",
} as const satisfies Record<SidecarErrorCode, string>;

export function documentErrorMessage(code: DocumentErrorCode): string {
  return i18n.t(documentErrorKeys[code]);
}

export function secretErrorMessage(code: SecretErrorCode): string {
  return i18n.t(secretErrorKeys[code]);
}

export function providerErrorMessage(code: ProviderErrorCode): string {
  return i18n.t(providerErrorKeys[code]);
}

export function sidecarErrorMessage(code: SidecarErrorCode): string {
  return i18n.t(sidecarErrorKeys[code]);
}
