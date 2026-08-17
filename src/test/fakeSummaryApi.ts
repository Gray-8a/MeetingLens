import type {
  ConfirmSummaryExternalDisclosureRequest,
  CreateSummaryRunRequest,
  DeepSeekProviderStatusResponse,
  ListSummaryEvidenceRequest,
  ListSummaryEvidenceResponse,
  ListSummaryRunsRequest,
  ListSummaryRunsResponse,
  SummaryApi,
  SummaryExternalDisclosureRequest,
  SummaryExternalDisclosureResponse,
  SummaryPolicyResponse,
  SummaryRunReferenceRequest,
  SummaryRunResponse,
} from "../api/summary";

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
  revokeError: unknown = null;
  revokePending: Promise<void> | null = null;
  revokeCallCount = 0;

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
    void request;
    throw new Error("Summary run fake is not configured.");
  }

  async createRevision(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    void request;
    throw new Error("Summary run fake is not configured.");
  }

  async executeRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    void request;
    throw new Error("Summary run fake is not configured.");
  }

  async getRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    void request;
    throw new Error("Summary run fake is not configured.");
  }

  async listRuns(
    request: ListSummaryRunsRequest,
  ): Promise<ListSummaryRunsResponse> {
    void request;
    return { items: [], nextCursor: null, hasMore: false, totalCount: 0 };
  }

  async cancelRun(
    request: SummaryRunReferenceRequest,
  ): Promise<SummaryRunResponse> {
    void request;
    throw new Error("Summary run fake is not configured.");
  }

  async listEvidence(
    request: ListSummaryEvidenceRequest,
  ): Promise<ListSummaryEvidenceResponse> {
    void request;
    return { items: [], nextCursor: null, hasMore: false, totalCount: 0 };
  }
}
