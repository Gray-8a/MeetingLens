import type {
  ConfirmSummaryExternalDisclosureRequest,
  CreateSummaryRunRequest,
  DeepSeekProviderStatusResponse,
  GetSummaryEvidenceRequest,
  ListSummaryEvidenceRequest,
  ListSummaryEvidenceResponse,
  ListSummaryRunsRequest,
  ListSummaryRunsResponse,
  SummaryApi,
  SummaryExternalDisclosureRequest,
  SummaryExternalDisclosureResponse,
  SummaryMarkdownRequest,
  SummaryMarkdownResponse,
  SummaryPolicyResponse,
  SaveSummaryMarkdownResponse,
  SummaryContent,
  SummaryEvidenceResponse,
  SummaryRunReferenceRequest,
  SummaryRunResponse,
  UpdateSummaryContentRequest,
} from "../api/summary";

const syntheticTimestamp = "2026-08-17T01:00:00.000Z";
const syntheticContentVersion = "a".repeat(64);

export function syntheticSummaryContent(
  overrides: Partial<SummaryContent> = {},
): SummaryContent {
  const evidenceId = "77777777-7777-4777-8777-777777777777";
  return {
    schemaVersion: 1,
    overview: { text: "Synthetic overview", evidenceIds: [evidenceId] },
    decisions: [
      {
        text: "Synthetic decision",
        status: "confirmed",
        evidenceIds: [evidenceId],
      },
    ],
    actionItems: [
      {
        text: "Synthetic action",
        owner: "Synthetic owner",
        dueDate: null,
        status: "unconfirmed",
        evidenceIds: [evidenceId],
      },
    ],
    risks: [],
    openQuestions: [],
    ...overrides,
  };
}

export function syntheticSummaryRun(
  overrides: Partial<SummaryRunResponse> = {},
): SummaryRunResponse {
  return {
    id: "66666666-6666-4666-8666-666666666666",
    meetingId: "11111111-1111-4111-8111-111111111111",
    seriesId: "99999999-9999-4999-8999-999999999999",
    previousSummaryId: null,
    revision: 1,
    runPurpose: "final",
    triggerKind: "manual",
    template: "detailed",
    outputLanguage: "en",
    status: "completed",
    provider: "deepseek",
    providerMode: "external",
    model: "deepseek-v4-pro",
    promptVersion: "summary-deepseek-v1",
    content: syntheticSummaryContent(),
    contentVersion: syntheticContentVersion,
    errorCode: null,
    sourceCount: 1,
    inputSegmentCount: 2,
    inputBytes: 128,
    isStale: false,
    staleReason: null,
    staleAt: null,
    isUserEdited: false,
    snapshotCreatedAt: syntheticTimestamp,
    createdAt: syntheticTimestamp,
    startedAt: syntheticTimestamp,
    finishedAt: syntheticTimestamp,
    cancelRequestedAt: null,
    updatedAt: syntheticTimestamp,
    ...overrides,
  };
}

export function syntheticSummaryEvidence(
  overrides: Partial<SummaryEvidenceResponse> = {},
): SummaryEvidenceResponse {
  return {
    id: "77777777-7777-4777-8777-777777777777",
    summaryId: "66666666-6666-4666-8666-666666666666",
    meetingId: "11111111-1111-4111-8111-111111111111",
    kind: "transcript",
    sourceId: "55555555-5555-4555-8555-555555555555",
    transcriptSegmentId: "88888888-8888-4888-8888-888888888888",
    segmentRevision: 1,
    label: "Synthetic evidence",
    speakerSnapshot: "Synthetic speaker",
    startOffsetMs: 0,
    endOffsetMs: 1_000,
    ordinal: 0,
    globalOrdinal: 0,
    excerpt: "Synthetic transcript evidence",
    sourceDeleted: false,
    createdAt: syntheticTimestamp,
    ...overrides,
  };
}

export const syntheticDeepSeekStatus = (
  overrides: Partial<DeepSeekProviderStatusResponse> = {},
): DeepSeekProviderStatusResponse => ({
  keyConfigured: false,
  provider: "deepseek",
  endpointHost: "api.deepseek.com",
  model: "deepseek-v4-pro",
  consentRemembered: false,
  consentedAt: null,
  policyVersion: "deepseek-transcript-external-v1",
  executionAvailable: false,
  ...overrides,
});

export const syntheticExternalDisclosure = (
  overrides: Partial<SummaryExternalDisclosureResponse> = {},
): SummaryExternalDisclosureResponse => ({
  provider: "deepseek",
  endpointHost: "api.deepseek.com",
  model: "deepseek-v4-pro",
  sourceCount: 1,
  segmentCount: 2,
  inputBytes: 128,
  template: "detailed",
  outputLanguage: "en",
  dataCategories: [
    "transcript_text",
    "speaker_labels_and_names",
    "time_offsets",
    "evidence_ids",
    "summary_template",
    "output_language",
  ],
  rememberedConsentValid: false,
  requiresConfirmation: true,
  policyVersion: "deepseek-transcript-external-v1",
  ...overrides,
});

export class FakeSummaryApi implements SummaryApi {
  providerStatus = syntheticDeepSeekStatus();
  disclosure = syntheticExternalDisclosure();
  providerStatusError: unknown = null;
  disclosureError: unknown = null;
  revokeError: unknown = null;
  revokePending: Promise<void> | null = null;
  revokeCallCount = 0;
  readonly runs: SummaryRunResponse[] = [];
  readonly evidenceItems: SummaryEvidenceResponse[] = [];
  readonly createCalls: CreateSummaryRunRequest[] = [];
  readonly revisionCalls: SummaryRunReferenceRequest[] = [];
  readonly executeCalls: SummaryRunReferenceRequest[] = [];
  readonly getCalls: SummaryRunReferenceRequest[] = [];
  readonly cancelCalls: SummaryRunReferenceRequest[] = [];
  readonly updateCalls: UpdateSummaryContentRequest[] = [];
  readonly markdownCalls: SummaryMarkdownRequest[] = [];
  readonly saveCalls: SummaryMarkdownRequest[] = [];
  executeError: unknown = null;
  updateError: unknown = null;
  markdown = "# Synthetic meeting minutes\n";
  pickerCancelled = false;
  private sequence = 0;

  seedRun(run: SummaryRunResponse): void {
    this.replaceRun(run);
  }

  seedEvidence(item: SummaryEvidenceResponse): void {
    const index = this.evidenceItems.findIndex(
      (candidate) => candidate.id === item.id,
    );
    if (index >= 0) this.evidenceItems[index] = { ...item };
    else this.evidenceItems.push({ ...item });
  }

  async getPolicy(): Promise<SummaryPolicyResponse> {
    return {
      templates: ["detailed", "concise", "actions_decisions"],
      outputLanguages: ["zh", "en", "bilingual"],
      runPurpose: "final",
      triggerKind: "manual",
      provider: "deepseek",
      maxSegments: 20_000,
      maxInputBytes: 1024 * 1024,
      maxEvidenceExcerptBytes: 1_024,
      listDefaultLimit: 50,
      listMaxLimit: 100,
      evidenceDefaultLimit: 50,
      evidenceMaxLimit: 100,
    };
  }

  async getDeepSeekProviderStatus(): Promise<DeepSeekProviderStatusResponse> {
    if (this.providerStatusError !== null) throw this.providerStatusError;
    return { ...this.providerStatus };
  }

  async getExternalDisclosure(
    request: SummaryExternalDisclosureRequest,
  ): Promise<SummaryExternalDisclosureResponse> {
    void request;
    if (this.disclosureError !== null) throw this.disclosureError;
    return {
      ...this.disclosure,
      dataCategories: [...this.disclosure.dataCategories],
    };
  }

  async confirmExternalDisclosure(
    request: ConfirmSummaryExternalDisclosureRequest,
  ): Promise<SummaryExternalDisclosureResponse> {
    return {
      ...this.disclosure,
      dataCategories: [...this.disclosure.dataCategories],
      rememberedConsentValid: request.remember,
      requiresConfirmation: false,
    };
  }

  async revokeDeepSeekConsent(): Promise<DeepSeekProviderStatusResponse> {
    this.revokeCallCount += 1;
    if (this.revokePending !== null) await this.revokePending;
    if (this.revokeError !== null) throw this.revokeError;
    this.providerStatus = syntheticDeepSeekStatus({
      ...this.providerStatus,
      consentRemembered: false,
      consentedAt: null,
    });
    return { ...this.providerStatus };
  }

  async createRun(
    request: CreateSummaryRunRequest,
  ): Promise<SummaryRunResponse> {
    this.createCalls.push({ ...request, sourceIds: [...request.sourceIds] });
    this.sequence += 1;
    const id = `66666666-6666-4666-8666-${String(this.sequence).padStart(12, "0")}`;
    const run = syntheticSummaryRun({
      id,
      meetingId: request.meetingId,
      seriesId: `99999999-9999-4999-8999-${String(this.sequence).padStart(12, "0")}`,
      template: request.template,
      outputLanguage: request.outputLanguage,
      status: "queued",
      content: null,
      contentVersion: null,
      startedAt: null,
      finishedAt: null,
      sourceCount: request.sourceIds.length,
      inputSegmentCount: Math.max(request.sourceIds.length, 1),
    });
    this.replaceRun(run);
    return cloneRun(run);
  }

  async createRevision(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    this.revisionCalls.push({ ...request });
    const base = this.requireRun(request);
    this.sequence += 1;
    const run = syntheticSummaryRun({
      ...base,
      id: `66666666-6666-4666-8666-${String(this.sequence).padStart(12, "0")}`,
      previousSummaryId: base.id,
      revision: base.revision + 1,
      status: "queued",
      content: null,
      contentVersion: null,
      errorCode: null,
      isUserEdited: false,
      startedAt: null,
      finishedAt: null,
      createdAt: "2026-08-17T01:00:01.000Z",
      updatedAt: "2026-08-17T01:00:01.000Z",
    });
    this.replaceRun(run);
    return cloneRun(run);
  }

  async executeRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    this.executeCalls.push({ ...request });
    if (this.executeError !== null) throw this.executeError;
    const current = this.requireRun(request);
    const run = syntheticSummaryRun({
      ...current,
      status: "completed",
      content: syntheticSummaryContent(),
      contentVersion: syntheticContentVersion,
      startedAt: syntheticTimestamp,
      finishedAt: syntheticTimestamp,
      updatedAt: syntheticTimestamp,
    });
    this.replaceRun(run);
    if (!this.evidenceItems.some((item) => item.summaryId === run.id)) {
      this.seedEvidence(
        syntheticSummaryEvidence({
          summaryId: run.id,
          meetingId: run.meetingId,
        }),
      );
    }
    return cloneRun(run);
  }

  async getRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    this.getCalls.push({ ...request });
    return cloneRun(this.requireRun(request));
  }

  async listRuns(
    request: ListSummaryRunsRequest,
  ): Promise<ListSummaryRunsResponse> {
    const all = this.runs
      .filter((run) => run.meetingId === request.meetingId)
      .sort(
        (left, right) =>
          right.createdAt.localeCompare(left.createdAt) ||
          left.id.localeCompare(right.id),
      );
    const cursorIndex = request.cursor
      ? all.findIndex((run) => run.id === request.cursor) + 1
      : 0;
    const limit = request.limit ?? 50;
    const items = all.slice(cursorIndex, cursorIndex + limit);
    const hasMore = cursorIndex + items.length < all.length;
    return {
      items: items.map(cloneRun),
      nextCursor: hasMore ? (items[items.length - 1]?.id ?? null) : null,
      hasMore,
      totalCount: all.length,
    };
  }

  async cancelRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    this.cancelCalls.push({ ...request });
    const current = this.requireRun(request);
    const run: SummaryRunResponse = {
      ...current,
      status: "cancelled",
      content: null,
      contentVersion: null,
      errorCode: null,
      finishedAt: syntheticTimestamp,
      cancelRequestedAt: syntheticTimestamp,
      updatedAt: syntheticTimestamp,
    };
    this.replaceRun(run);
    return cloneRun(run);
  }

  async listEvidence(
    request: ListSummaryEvidenceRequest,
  ): Promise<ListSummaryEvidenceResponse> {
    const all = this.evidenceItems
      .filter(
        (item) =>
          item.meetingId === request.meetingId &&
          item.summaryId === request.summaryId,
      )
      .sort((left, right) => left.globalOrdinal - right.globalOrdinal);
    const remaining = all.filter(
      (item) => request.cursor == null || item.globalOrdinal > request.cursor,
    );
    const items = remaining.slice(0, request.limit ?? 50);
    const hasMore = remaining.length > items.length;
    return {
      items: items.map((item) => ({ ...item })),
      nextCursor: hasMore
        ? (items[items.length - 1]?.globalOrdinal ?? null)
        : null,
      hasMore,
      totalCount: all.length,
    };
  }

  async getEvidence(
    request: GetSummaryEvidenceRequest,
  ): Promise<ListSummaryEvidenceResponse["items"][number]> {
    const item = this.evidenceItems.find(
      (candidate) =>
        candidate.id === request.evidenceId &&
        candidate.summaryId === request.summaryId &&
        candidate.meetingId === request.meetingId,
    );
    if (!item) throw new Error("Summary evidence fake is not configured.");
    return { ...item };
  }

  async updateContent(
    request: UpdateSummaryContentRequest,
  ): Promise<SummaryRunResponse> {
    this.updateCalls.push({
      ...request,
      editableContent: {
        ...request.editableContent,
        decisionTexts: [...request.editableContent.decisionTexts],
        actionItems: request.editableContent.actionItems.map((item) => ({
          ...item,
        })),
        riskTexts: [...request.editableContent.riskTexts],
        openQuestionTexts: [...request.editableContent.openQuestionTexts],
      },
    });
    if (this.updateError !== null) throw this.updateError;
    const current = this.requireRun(request);
    if (
      !current.content ||
      current.contentVersion !== request.expectedContentVersion
    ) {
      throw new Error("Summary edit conflict.");
    }
    const run: SummaryRunResponse = {
      ...current,
      content: {
        ...current.content,
        overview: {
          ...current.content.overview,
          text: request.editableContent.overviewText,
        },
        decisions: current.content.decisions.map((item, index) => ({
          ...item,
          text: request.editableContent.decisionTexts[index] ?? item.text,
        })),
        actionItems: current.content.actionItems.map((item, index) => ({
          ...item,
          ...(request.editableContent.actionItems[index] ?? {}),
        })),
        risks: current.content.risks.map((item, index) => ({
          ...item,
          text: request.editableContent.riskTexts[index] ?? item.text,
        })),
        openQuestions: current.content.openQuestions.map((item, index) => ({
          ...item,
          text: request.editableContent.openQuestionTexts[index] ?? item.text,
        })),
      },
      contentVersion: "b".repeat(64),
      isUserEdited: true,
    };
    this.replaceRun(run);
    return cloneRun(run);
  }

  async getMarkdown(
    request: SummaryMarkdownRequest,
  ): Promise<SummaryMarkdownResponse> {
    this.markdownCalls.push({ ...request });
    return {
      summaryId: request.summaryId,
      contentVersion: request.expectedContentVersion,
      markdown: this.markdown,
      bytes: new TextEncoder().encode(this.markdown).length,
    };
  }

  async saveMarkdown(
    request: SummaryMarkdownRequest,
  ): Promise<SaveSummaryMarkdownResponse> {
    this.saveCalls.push({ ...request });
    return {
      summaryId: request.summaryId,
      contentVersion: request.expectedContentVersion,
      saved: !this.pickerCancelled,
      cancelled: this.pickerCancelled,
      bytesWritten: this.pickerCancelled
        ? 0
        : new TextEncoder().encode(this.markdown).length,
    };
  }

  private requireRun(request: SummaryRunReferenceRequest): SummaryRunResponse {
    const run = this.runs.find(
      (candidate) =>
        candidate.id === request.summaryId &&
        candidate.meetingId === request.meetingId,
    );
    if (!run) throw new Error("Summary run fake is not configured.");
    return run;
  }

  private replaceRun(run: SummaryRunResponse): void {
    const index = this.runs.findIndex((candidate) => candidate.id === run.id);
    if (index >= 0) this.runs[index] = cloneRun(run);
    else this.runs.push(cloneRun(run));
  }
}

function cloneRun(run: SummaryRunResponse): SummaryRunResponse {
  return {
    ...run,
    content: run.content
      ? {
          ...run.content,
          overview: {
            ...run.content.overview,
            evidenceIds: [...run.content.overview.evidenceIds],
          },
          decisions: run.content.decisions.map((item) => ({
            ...item,
            evidenceIds: [...item.evidenceIds],
          })),
          actionItems: run.content.actionItems.map((item) => ({
            ...item,
            evidenceIds: [...item.evidenceIds],
          })),
          risks: run.content.risks.map((item) => ({
            ...item,
            evidenceIds: [...item.evidenceIds],
          })),
          openQuestions: run.content.openQuestions.map((item) => ({
            ...item,
            evidenceIds: [...item.evidenceIds],
          })),
        }
      : null,
  };
}
