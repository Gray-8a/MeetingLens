import type {
  TranslationProvider,
  TranslationRequest,
  TranslationResult,
} from "../contracts";
import { MockAsyncProviderBase, systemTimer, type MockTimer } from "./base";

export interface MockTranslationOptions {
  delayMs?: number;
  failSegmentIds?: ReadonlySet<string>;
  timer?: MockTimer;
}

export class MockTranslationProvider
  extends MockAsyncProviderBase
  implements TranslationProvider
{
  readonly kind = "translation" as const;
  readonly name = "Mock translation";
  private readonly failSegmentIds: ReadonlySet<string>;

  constructor(options: MockTranslationOptions = {}) {
    super("ready", options.timer ?? systemTimer, options.delayMs ?? 25);
    this.failSegmentIds = options.failSegmentIds ?? new Set();
  }

  translateSegment(request: TranslationRequest): Promise<TranslationResult> {
    if (
      request.segmentId.trim().length === 0 ||
      request.sourceText.trim().length === 0 ||
      request.sourceLanguage.trim().length === 0 ||
      request.targetLanguage.trim().length === 0
    ) {
      throw this.error("provider_operation_failed", "translate");
    }

    return this.runOperation(
      request.segmentId,
      "translate",
      () => ({
        segmentId: request.segmentId,
        translatedText: `[Mock ${request.targetLanguage}] ${request.sourceText}`,
        sourceLanguage: request.sourceLanguage,
        targetLanguage: request.targetLanguage,
        isFinal: true,
        isMock: true,
      }),
      this.failSegmentIds.has(request.segmentId),
    );
  }

  async cancel(segmentId: string): Promise<void> {
    this.cancelOperation(segmentId);
  }

  async dispose(): Promise<void> {
    if (this.getStatus() === "disposed") return;
    this.disposeOperations();
  }
}
