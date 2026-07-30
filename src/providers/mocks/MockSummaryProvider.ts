import type {
  SummaryProvider,
  SummaryRequest,
  SummaryResult,
} from "../contracts";
import { MockAsyncProviderBase, systemTimer, type MockTimer } from "./base";

export interface MockSummaryOptions {
  delayMs?: number;
  failRequestIds?: ReadonlySet<string>;
  timer?: MockTimer;
}

export class MockSummaryProvider
  extends MockAsyncProviderBase
  implements SummaryProvider
{
  readonly kind = "summary" as const;
  readonly name = "Mock summary";
  private readonly failRequestIds: ReadonlySet<string>;

  constructor(options: MockSummaryOptions = {}) {
    super("ready", options.timer ?? systemTimer, options.delayMs ?? 25);
    this.failRequestIds = options.failRequestIds ?? new Set();
  }

  generate(request: SummaryRequest): Promise<SummaryResult> {
    if (
      request.requestId.trim().length === 0 ||
      request.meeting.meetingId.trim().length === 0
    ) {
      throw this.error("provider_operation_failed", "generate");
    }

    return this.runOperation(
      request.requestId,
      "generate",
      () => ({
        requestId: request.requestId,
        overview: "Mock summary generated from synthetic in-memory input.",
        decisions: ["Mock decision: retain the local-first boundary."],
        actionItems: ["Mock action: review the next isolated milestone."],
        isMock: true,
      }),
      this.failRequestIds.has(request.requestId),
    );
  }

  async cancel(requestId: string): Promise<void> {
    this.cancelOperation(requestId);
  }

  async dispose(): Promise<void> {
    if (this.getStatus() === "disposed") return;
    this.disposeOperations();
  }
}
