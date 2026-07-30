import type {
  CaptureProvider,
  DocumentParserProvider,
  ProviderStatusSnapshot,
  SummaryProvider,
  TranscriptionProvider,
  TranslationProvider,
} from "./contracts";
import { MockCaptureProvider } from "./mocks/MockCaptureProvider";
import { MockDocumentParserProvider } from "./mocks/MockDocumentParserProvider";
import { MockSummaryProvider } from "./mocks/MockSummaryProvider";
import { MockTranscriptionProvider } from "./mocks/MockTranscriptionProvider";
import { MockTranslationProvider } from "./mocks/MockTranslationProvider";

export interface ProviderSet {
  capture: CaptureProvider;
  transcription: TranscriptionProvider;
  translation: TranslationProvider;
  summary: SummaryProvider;
  "document-parser": DocumentParserProvider;
}

export type ProviderOverrides = Partial<ProviderSet>;

export function createMockProviderSet(
  overrides: ProviderOverrides = {},
): ProviderSet {
  return {
    capture: overrides.capture ?? new MockCaptureProvider(),
    transcription: overrides.transcription ?? new MockTranscriptionProvider(),
    translation: overrides.translation ?? new MockTranslationProvider(),
    summary: overrides.summary ?? new MockSummaryProvider(),
    "document-parser":
      overrides["document-parser"] ?? new MockDocumentParserProvider(),
  };
}

export class ProviderRegistry {
  private providers: ProviderSet;
  private disposed = false;

  constructor(overrides: ProviderOverrides = {}) {
    this.providers = createMockProviderSet(overrides);
  }

  get<K extends keyof ProviderSet>(kind: K): ProviderSet[K] {
    return this.providers[kind];
  }

  snapshots(): readonly ProviderStatusSnapshot[] {
    return Object.values(this.providers).map((provider) => ({
      kind: provider.kind,
      name: provider.name,
      mode: provider.mode,
      status: provider.getStatus(),
    }));
  }

  async replace<K extends keyof ProviderSet>(
    kind: K,
    provider: ProviderSet[K],
  ): Promise<void> {
    if (provider.kind !== kind) {
      throw new Error("Provider kind mismatch.");
    }
    await this.providers[kind].dispose();
    this.providers[kind] = provider;
    this.disposed = false;
  }

  async reset(overrides: ProviderOverrides = {}): Promise<void> {
    await this.disposeAll();
    this.providers = createMockProviderSet(overrides);
    this.disposed = false;
  }

  async disposeAll(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true;
    await Promise.all(
      Object.values(this.providers).map((provider) => provider.dispose()),
    );
  }
}
