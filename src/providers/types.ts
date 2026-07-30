export type ProviderKind =
  | "capture"
  | "transcription"
  | "translation"
  | "summary"
  | "document-parser";

export type ProviderMode = "mock" | "local" | "external" | "disabled";

export type ProviderStatus =
  | "idle"
  | "initializing"
  | "ready"
  | "running"
  | "paused"
  | "stopping"
  | "stopped"
  | "error"
  | "disposed";

export type ProviderOperation =
  | "initialize"
  | "get_status"
  | "list_sources"
  | "start"
  | "pause"
  | "resume"
  | "stop"
  | "subscribe"
  | "translate"
  | "generate"
  | "validate"
  | "parse"
  | "cancel"
  | "dispose";

export interface ProviderDescriptor {
  kind: ProviderKind;
  mode: ProviderMode;
  name: string;
  status: ProviderStatus;
  enabled: boolean;
}

export type ProviderStatusListener = (status: ProviderStatus) => void;

export interface ProviderBase {
  readonly kind: ProviderKind;
  readonly mode: ProviderMode;
  readonly name: string;
  getStatus(): ProviderStatus;
  subscribeStatus(listener: ProviderStatusListener): () => void;
  dispose(): Promise<void>;
}

const providerStatuses = new Set<ProviderStatus>([
  "idle",
  "initializing",
  "ready",
  "running",
  "paused",
  "stopping",
  "stopped",
  "error",
  "disposed",
]);

export function safeProviderStatus(value: unknown): ProviderStatus | "unknown" {
  return typeof value === "string" &&
    providerStatuses.has(value as ProviderStatus)
    ? (value as ProviderStatus)
    : "unknown";
}

export function providerStatusLabel(value: unknown): string {
  const status = safeProviderStatus(value);
  if (status === "unknown") return "Unknown status";
  return status.charAt(0).toUpperCase() + status.slice(1);
}
