import type { ProviderBase, ProviderStatus } from "./types";

export type CaptureSourceKind =
  | "microphone"
  | "systemAudio"
  | "window"
  | "screen";

export interface CaptureSource {
  id: string;
  kind: CaptureSourceKind;
  label: string;
  isMock: boolean;
}

export interface CaptureSessionRequest {
  meetingId: string;
  sourceIds: readonly string[];
}

export interface CaptureSession {
  sessionId: string;
  meetingId: string;
  startedAt: string;
  isMock: boolean;
}

export interface CaptureProvider extends ProviderBase {
  readonly kind: "capture";
  initialize(): Promise<void>;
  listSources(): Promise<readonly CaptureSource[]>;
  startSession(request: CaptureSessionRequest): Promise<CaptureSession>;
  pauseSession(): Promise<void>;
  resumeSession(): Promise<void>;
  stopSession(): Promise<void>;
}

export interface TranscriptionSessionRequest {
  meetingId: string;
  language: string;
  startedAt: string;
}

export interface TranscriptionSegment {
  segmentId: string;
  speakerLabel: string;
  startedAt: string;
  endedAt: string;
  text: string;
  language: string;
  confidence: number | null;
  isFinal: boolean;
}

export type TranscriptionListener = (segment: TranscriptionSegment) => void;

export interface TranscriptionProvider extends ProviderBase {
  readonly kind: "transcription";
  start(request: TranscriptionSessionRequest): Promise<void>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  stop(): Promise<void>;
  subscribe(listener: TranscriptionListener): () => void;
}

export interface TranslationRequest {
  segmentId: string;
  sourceText: string;
  sourceLanguage: string;
  targetLanguage: string;
}

export interface TranslationResult {
  segmentId: string;
  translatedText: string;
  sourceLanguage: string;
  targetLanguage: string;
  isFinal: boolean;
  isMock: boolean;
}

export interface TranslationProvider extends ProviderBase {
  readonly kind: "translation";
  translateSegment(request: TranslationRequest): Promise<TranslationResult>;
  cancel(segmentId: string): Promise<void>;
}

export interface SummaryMeetingMetadata {
  meetingId: string;
  title: string;
  primaryLanguage: string;
}

export interface SummaryDocumentReference {
  documentId: string;
  sectionIds: readonly string[];
}

export interface SummaryRequest {
  requestId: string;
  meeting: SummaryMeetingMetadata;
  transcriptSegments: readonly TranscriptionSegment[];
  documentReferences: readonly SummaryDocumentReference[];
  screenContext: readonly string[];
  manualNotes: string;
}

export interface SummaryResult {
  requestId: string;
  overview: string;
  decisions: readonly string[];
  actionItems: readonly string[];
  isMock: boolean;
}

export interface SummaryProvider extends ProviderBase {
  readonly kind: "summary";
  generate(request: SummaryRequest): Promise<SummaryResult>;
  cancel(requestId: string): Promise<void>;
}

export interface DocumentReference {
  artifactId: string;
  originalFilename: string;
  mimeType: string;
  sizeBytes: number;
}

export interface DocumentValidationResult {
  valid: boolean;
  warnings: readonly string[];
}

export interface ParsedDocumentSection {
  sectionId: string;
  title: string;
  text: string;
  pageNumber: number | null;
}

export interface ParsedDocument {
  requestId: string;
  sections: readonly ParsedDocumentSection[];
  pages: number;
  tables: number;
  metadata: Readonly<Record<string, string>>;
  warnings: readonly string[];
  isMock: boolean;
}

export interface DocumentParserProvider extends ProviderBase {
  readonly kind: "document-parser";
  validate(reference: DocumentReference): Promise<DocumentValidationResult>;
  parse(
    requestId: string,
    reference: DocumentReference,
  ): Promise<ParsedDocument>;
  cancel(requestId: string): Promise<void>;
}

export type AnyProvider =
  | CaptureProvider
  | TranscriptionProvider
  | TranslationProvider
  | SummaryProvider
  | DocumentParserProvider;

export interface ProviderStatusSnapshot {
  kind: AnyProvider["kind"];
  name: string;
  mode: AnyProvider["mode"];
  status: ProviderStatus;
}
