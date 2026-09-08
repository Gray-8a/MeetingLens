import type { AudioTranscriptionErrorCode } from "../api/audioTranscription";
import type { DocumentErrorCode } from "../api/document";
import type { SecretErrorCode } from "../api/secret";
import type { SummaryErrorCode } from "../api/summary";
import type { TranscriptErrorCode } from "../api/transcript";
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
  document_parse_interrupted: "error.document.document_parse_interrupted",
  document_not_found: "error.document.document_not_found",
  document_already_imported: "error.document.document_already_imported",
  document_invalid_state: "error.document.document_invalid_state",
  document_storage_unavailable: "error.document.document_storage_unavailable",
  document_integrity_failed: "error.document.document_integrity_failed",
  document_delete_pending: "error.document.document_delete_pending",
  document_delete_failed: "error.document.document_delete_failed",
  parser_unavailable: "error.document.parser_unavailable",
  parser_environment_missing: "error.document.parser_environment_missing",
  parser_dependency_incompatible:
    "error.document.parser_dependency_incompatible",
  parser_models_missing: "error.document.parser_models_missing",
  parser_models_incompatible: "error.document.parser_models_incompatible",
  parser_invalid_input: "error.document.parser_invalid_input",
  parser_unsupported_type: "error.document.parser_unsupported_type",
  parser_ocr_unavailable: "error.document.parser_ocr_unavailable",
  parser_protocol_error: "error.document.parser_protocol_error",
  parser_worker_start_failed: "error.document.parser_worker_start_failed",
  parser_worker_crashed: "error.document.parser_worker_crashed",
  parser_timeout: "error.document.parser_timeout",
  parser_cancelled: "error.document.parser_cancelled",
  parser_resource_limit: "error.document.parser_resource_limit",
  parser_page_limit_exceeded: "error.document.parser_page_limit_exceeded",
  parser_output_too_large: "error.document.parser_output_too_large",
  parser_partial_result: "error.document.parser_partial_result",
  parser_integrity_failed: "error.document.parser_integrity_failed",
  parser_failed: "error.document.parser_failed",
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

const summaryErrorKeys = {
  deepseek_access_denied: "error.summary.deepseek_access_denied",
  deepseek_api_key_missing: "error.summary.deepseek_api_key_missing",
  deepseek_auth_failed: "error.summary.deepseek_auth_failed",
  deepseek_consent_invalid: "error.summary.deepseek_consent_invalid",
  deepseek_consent_required: "error.summary.deepseek_consent_required",
  deepseek_content_filtered: "error.summary.deepseek_content_filtered",
  deepseek_insufficient_balance: "error.summary.deepseek_insufficient_balance",
  deepseek_model_unavailable: "error.summary.deepseek_model_unavailable",
  deepseek_network_failed: "error.summary.deepseek_network_failed",
  deepseek_output_truncated: "error.summary.deepseek_output_truncated",
  deepseek_rate_limited: "error.summary.deepseek_rate_limited",
  deepseek_request_invalid: "error.summary.deepseek_request_invalid",
  deepseek_response_empty: "error.summary.deepseek_response_empty",
  deepseek_response_invalid: "error.summary.deepseek_response_invalid",
  deepseek_service_unavailable: "error.summary.deepseek_service_unavailable",
  deepseek_timeout: "error.summary.deepseek_timeout",
  summary_input_insufficient: "error.summary.summary_input_insufficient",
  summary_input_too_large: "error.summary.summary_input_too_large",
  summary_provider_input_too_large:
    "error.summary.summary_provider_input_too_large",
  summary_evidence_invalid: "error.summary.summary_evidence_invalid",
  summary_edit_conflict: "error.summary.summary_edit_conflict",
  summary_edit_too_large: "error.summary.summary_edit_too_large",
  summary_export_failed: "error.summary.summary_export_failed",
  summary_markdown_too_large: "error.summary.summary_markdown_too_large",
  internal_error: "error.summary.internal_error",
  summary_invalid_input: "error.summary.summary_invalid_input",
  summary_invalid_state: "error.summary.summary_invalid_state",
  meeting_invalid_state: "error.summary.meeting_invalid_state",
  meeting_not_found: "error.summary.meeting_not_found",
  summary_persistence_failed: "error.summary.summary_persistence_failed",
  summary_provider_failed: "error.summary.summary_provider_failed",
  summary_provider_output_invalid:
    "error.summary.summary_provider_output_invalid",
  summary_revision_conflict: "error.summary.summary_revision_conflict",
  summary_run_not_found: "error.summary.summary_run_not_found",
  summary_segment_limit_exceeded:
    "error.summary.summary_segment_limit_exceeded",
  summary_snapshot_missing: "error.summary.summary_snapshot_missing",
  summary_source_not_found: "error.summary.summary_source_not_found",
} as const satisfies Record<SummaryErrorCode, string>;

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

const transcriptErrorKeys = {
  transcript_already_imported: "error.transcript.transcript_already_imported",
  transcript_empty: "error.transcript.transcript_empty",
  transcript_input_insufficient:
    "error.transcript.transcript_input_insufficient",
  transcript_invalid_encoding: "error.transcript.transcript_invalid_encoding",
  transcript_invalid_format: "error.transcript.transcript_invalid_format",
  transcript_invalid_input: "error.transcript.transcript_invalid_input",
  transcript_invalid_state: "error.transcript.transcript_invalid_state",
  transcript_invalid_timestamp: "error.transcript.transcript_invalid_timestamp",
  transcript_persistence_failed:
    "error.transcript.transcript_persistence_failed",
  transcript_revision_conflict: "error.transcript.transcript_revision_conflict",
  transcript_segment_limit_exceeded:
    "error.transcript.transcript_segment_limit_exceeded",
  transcript_segment_not_found: "error.transcript.transcript_segment_not_found",
  transcript_segment_too_large: "error.transcript.transcript_segment_too_large",
  transcript_source_in_use: "error.transcript.transcript_source_in_use",
  transcript_source_not_found: "error.transcript.transcript_source_not_found",
  transcript_too_large: "error.transcript.transcript_too_large",
  transcript_picker_unavailable:
    "error.transcript.transcript_picker_unavailable",
  meeting_not_found: "error.transcript.meeting_not_found",
  meeting_invalid_state: "error.transcript.meeting_invalid_state",
  internal_error: "error.transcript.internal_error",
} as const satisfies Record<TranscriptErrorCode, string>;

const audioTranscriptionErrorKeys = {
  audio_picker_unavailable: "error.audio.audio_picker_unavailable",
  audio_invalid_input: "error.audio.audio_invalid_input",
  audio_unsupported_type: "error.audio.audio_unsupported_type",
  audio_type_mismatch: "error.audio.audio_type_mismatch",
  audio_empty: "error.audio.audio_empty",
  audio_too_large: "error.audio.audio_too_large",
  audio_too_long: "error.audio.audio_too_long",
  audio_stream_missing: "error.audio.audio_stream_missing",
  audio_model_missing: "error.audio.audio_model_missing",
  audio_model_incompatible: "error.audio.audio_model_incompatible",
  audio_worker_unavailable: "error.audio.audio_worker_unavailable",
  audio_worker_start_failed: "error.audio.audio_worker_start_failed",
  audio_worker_crashed: "error.audio.audio_worker_crashed",
  audio_protocol_error: "error.audio.audio_protocol_error",
  audio_output_too_large: "error.audio.audio_output_too_large",
  audio_integrity_failed: "error.audio.audio_integrity_failed",
  audio_timeout: "error.audio.audio_timeout",
  audio_cancelled: "error.audio.audio_cancelled",
  audio_transcription_busy: "error.audio.audio_transcription_busy",
  audio_persistence_failed: "error.audio.audio_persistence_failed",
  meeting_not_found: "error.audio.meeting_not_found",
  meeting_invalid_state: "error.audio.meeting_invalid_state",
  internal_error: "error.audio.internal_error",
} as const satisfies Record<AudioTranscriptionErrorCode, string>;

export function documentErrorMessage(code: DocumentErrorCode): string {
  return i18n.t(documentErrorKeys[code]);
}

export function secretErrorMessage(code: SecretErrorCode): string {
  return i18n.t(secretErrorKeys[code]);
}

export function summaryErrorMessage(code: SummaryErrorCode): string {
  return i18n.t(summaryErrorKeys[code]);
}

export function providerErrorMessage(code: ProviderErrorCode): string {
  return i18n.t(providerErrorKeys[code]);
}

export function sidecarErrorMessage(code: SidecarErrorCode): string {
  return i18n.t(sidecarErrorKeys[code]);
}

export function transcriptErrorMessage(code: TranscriptErrorCode): string {
  return i18n.t(transcriptErrorKeys[code]);
}

export function audioTranscriptionErrorMessage(
  code: AudioTranscriptionErrorCode,
): string {
  return i18n.t(audioTranscriptionErrorKeys[code]);
}
