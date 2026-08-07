import {
  TranscriptApiError,
  type AddPasteToMeetingRequest,
  type CreateMeetingFromPasteRequest,
  type DeleteTranscriptSourceResponse,
  type ListTranscriptSegmentsResponse,
  type ListTranscriptSourcesResponse,
  type ListTranscriptSpeakerGroupsResponse,
  type RenameSpeakerInSourceRequest,
  type RenameSpeakerInSourceResponse,
  type SelectAndAddToMeetingRequest,
  type SelectTranscriptAndCreateMeetingRequest,
  type TranscriptApi,
  type TranscriptImportPolicyResponse,
  type TranscriptImportResult,
  type TranscriptPageOptions,
  type TranscriptSegmentResponse,
  type TranscriptSourceResponse,
  type TranscriptSourceType,
  type TranscriptSpeakerPageOptions,
  type UpdateTranscriptSegmentRequest,
} from "../api/transcript";
import { compareSqliteBinaryText } from "../lib/sqliteBinary";
import type { FakeMeetingApi } from "./fakeMeetingApi";

const policy: TranscriptImportPolicyResponse = {
  allowedExtensions: ["txt", "srt", "vtt"],
  maxPasteBytes: 2 * 1024 * 1024,
  maxFileBytes: 10 * 1024 * 1024,
  maxSegmentsPerSource: 20_000,
  maxSegmentBytes: 64 * 1024,
};

export function syntheticTranscriptSource(
  overrides: Partial<TranscriptSourceResponse> = {},
): TranscriptSourceResponse {
  return {
    id: "55555555-5555-4555-8555-555555555555",
    meetingId: "11111111-1111-4111-8111-111111111111",
    sourceType: "pasted_text",
    displayName: "Synthetic transcript",
    originalFilename: null,
    language: null,
    baseStartedAt: null,
    segmentCount: 2,
    startOffsetMs: 0,
    endOffsetMs: 10_000,
    speakerCount: 2,
    importedAt: "2026-08-06T01:00:00.000Z",
    createdAt: "2026-08-06T01:00:00.000Z",
    updatedAt: "2026-08-06T01:00:00.000Z",
    ...overrides,
  };
}

export function syntheticTranscriptSegment(
  overrides: Partial<TranscriptSegmentResponse> = {},
): TranscriptSegmentResponse {
  return {
    id: "66666666-6666-4666-8666-666666666666",
    sourceId: "55555555-5555-4555-8555-555555555555",
    meetingId: "11111111-1111-4111-8111-111111111111",
    ordinal: 0,
    speakerLabel: "Speaker 1",
    speakerName: null,
    startOffsetMs: 0,
    endOffsetMs: 5_000,
    originalText: "Synthetic transcript segment",
    translatedText: null,
    language: "en-AU",
    confidence: null,
    isFinal: true,
    revision: 1,
    correctedAt: null,
    ...overrides,
  };
}

export class FakeTranscriptApi implements TranscriptApi {
  readonly sources: TranscriptSourceResponse[] = [];
  readonly segments = new Map<string, TranscriptSegmentResponse[]>();
  readonly createPasteCalls: CreateMeetingFromPasteRequest[] = [];
  readonly createImportCalls: SelectTranscriptAndCreateMeetingRequest[] = [];
  readonly addPasteCalls: AddPasteToMeetingRequest[] = [];
  readonly addImportCalls: SelectAndAddToMeetingRequest[] = [];
  readonly listSegmentCalls: Array<{
    meetingId: string;
    sourceId: string;
    options?: TranscriptPageOptions;
  }> = [];
  readonly renameCalls: RenameSpeakerInSourceRequest[] = [];
  readonly updateCalls: UpdateTranscriptSegmentRequest[] = [];
  readonly deleteCalls: Array<{ meetingId: string; sourceId: string }> = [];
  pickerCancelled = false;
  nextDuplicate = false;
  nextSelectedType: TranscriptSourceType = "imported_txt";
  nextError: unknown = null;
  policyError: unknown = null;
  private sequence = 100;

  constructor(private readonly meetings: FakeMeetingApi) {}

  seed(
    source: TranscriptSourceResponse,
    segments: TranscriptSegmentResponse[] = [],
  ): void {
    this.sources.push(cloneSource(source));
    this.segments.set(
      source.id,
      segments.map((segment) => cloneSegment(segment)),
    );
    this.refreshSourceOverview(source.id);
  }

  async createMeetingFromPaste(
    request: CreateMeetingFromPasteRequest,
  ): Promise<TranscriptImportResult> {
    this.createPasteCalls.push({ ...request });
    this.throwNextError();
    const meeting = await this.meetings.create({
      title: request.title,
      primaryLanguage: request.primaryLanguage,
    });
    return this.addSource(
      meeting.id,
      "pasted_text",
      request.displayName,
      null,
      request.pastedText,
      this.nextDuplicate,
      meeting,
    );
  }

  async selectTranscriptAndCreateMeeting(
    request: SelectTranscriptAndCreateMeetingRequest,
  ): Promise<TranscriptImportResult> {
    this.createImportCalls.push({ ...request });
    this.throwNextError();
    if (this.pickerCancelled) return cancelledImport();
    const meeting = await this.meetings.create({
      title: request.title,
      primaryLanguage: request.primaryLanguage,
    });
    return this.addSelectedSource(meeting.id, meeting);
  }

  async addPasteToMeeting(
    request: AddPasteToMeetingRequest,
  ): Promise<TranscriptImportResult> {
    this.addPasteCalls.push({ ...request });
    this.throwNextError();
    const meeting = await this.meetings.get(request.meetingId);
    return this.addSource(
      meeting.id,
      "pasted_text",
      request.displayName,
      null,
      request.pastedText,
      this.nextDuplicate,
      meeting,
    );
  }

  async selectAndAddToMeeting(
    request: SelectAndAddToMeetingRequest,
  ): Promise<TranscriptImportResult> {
    this.addImportCalls.push({ ...request });
    this.throwNextError();
    if (this.pickerCancelled) return cancelledImport();
    return this.addSelectedSource(
      request.meetingId,
      await this.meetings.get(request.meetingId),
    );
  }

  async listSources(meetingId: string): Promise<ListTranscriptSourcesResponse> {
    this.throwNextError();
    return {
      sources: this.sources
        .filter((source) => source.meetingId === meetingId)
        .map(cloneSource),
    };
  }

  async getSource(
    meetingId: string,
    sourceId: string,
  ): Promise<TranscriptSourceResponse> {
    this.throwNextError();
    const source = this.requireSource(meetingId, sourceId);
    return cloneSource(source);
  }

  async listSegments(
    meetingId: string,
    sourceId: string,
    options?: TranscriptPageOptions,
  ): Promise<ListTranscriptSegmentsResponse> {
    this.listSegmentCalls.push({ meetingId, sourceId, options });
    this.throwNextError();
    this.requireSource(meetingId, sourceId);
    const cursor = options?.cursor ?? null;
    const limit = Math.min(options?.limit ?? 100, 200);
    const all = (this.segments.get(sourceId) ?? [])
      .filter((segment) => segment.meetingId === meetingId)
      .sort((left, right) => left.ordinal - right.ordinal);
    const remaining = all.filter(
      (segment) => cursor === null || segment.ordinal > cursor,
    );
    const items = remaining.slice(0, limit).map(cloneSegment);
    const hasMore = remaining.length > items.length;
    return {
      items,
      nextCursor: hasMore ? (items[items.length - 1]?.ordinal ?? null) : null,
      hasMore,
      totalCount: all.length,
    };
  }

  async listSpeakerGroups(
    meetingId: string,
    sourceId: string,
    options?: TranscriptSpeakerPageOptions,
  ): Promise<ListTranscriptSpeakerGroupsResponse> {
    this.throwNextError();
    this.requireSource(meetingId, sourceId);
    const groups = new Map<string, Set<string | null>>();
    const counts = new Map<string, number>();
    for (const segment of this.segments.get(sourceId) ?? []) {
      if (segment.meetingId !== meetingId || segment.speakerLabel === null)
        continue;
      const names =
        groups.get(segment.speakerLabel) ?? new Set<string | null>();
      names.add(segment.speakerName);
      groups.set(segment.speakerLabel, names);
      counts.set(
        segment.speakerLabel,
        (counts.get(segment.speakerLabel) ?? 0) + 1,
      );
    }
    const cursor = options?.cursor ?? null;
    const limit = Math.min(options?.limit ?? 100, 200);
    const all = [...groups.entries()]
      .map(([speakerLabel, names]) => ({
        speakerLabel,
        speakerName: names.size === 1 ? [...names][0] : null,
        hasMixedNames: names.size > 1,
        segmentCount: counts.get(speakerLabel) ?? 0,
      }))
      .sort((left, right) =>
        compareSqliteBinaryText(left.speakerLabel, right.speakerLabel),
      );
    const remaining = all.filter(
      (group) =>
        cursor === null ||
        compareSqliteBinaryText(group.speakerLabel, cursor) > 0,
    );
    const items = remaining.slice(0, limit);
    const hasMore = remaining.length > items.length;
    return {
      sourceId,
      items,
      nextCursor: hasMore
        ? (items[items.length - 1]?.speakerLabel ?? null)
        : null,
      hasMore,
      totalCount: all.length,
    };
  }

  async updateSegment(
    request: UpdateTranscriptSegmentRequest,
  ): Promise<TranscriptSegmentResponse> {
    this.updateCalls.push({ ...request });
    this.throwNextError();
    const entries = [...this.segments.values()].flat();
    const segment = entries.find(
      (candidate) =>
        candidate.meetingId === request.meetingId &&
        candidate.id === request.segmentId,
    );
    if (!segment) {
      throw new TranscriptApiError("transcript_segment_not_found", false);
    }
    if (segment.revision !== request.expectedRevision) {
      throw new TranscriptApiError("transcript_revision_conflict", false);
    }
    if (request.speakerName !== undefined) {
      segment.speakerName = request.speakerName;
    }
    if (request.originalText !== undefined) {
      segment.originalText = request.originalText;
    }
    segment.revision += 1;
    segment.correctedAt = "2026-08-06T01:02:00.000Z";
    this.refreshSourceOverview(segment.sourceId);
    return cloneSegment(segment);
  }

  async renameSpeakerInSource(
    request: RenameSpeakerInSourceRequest,
  ): Promise<RenameSpeakerInSourceResponse> {
    this.renameCalls.push({ ...request });
    this.throwNextError();
    this.requireSource(request.meetingId, request.sourceId);
    let updatedCount = 0;
    for (const segment of this.segments.get(request.sourceId) ?? []) {
      if (
        segment.meetingId === request.meetingId &&
        segment.speakerLabel === request.speakerLabel
      ) {
        segment.speakerName = request.speakerName;
        segment.revision += 1;
        segment.correctedAt = "2026-08-06T01:03:00.000Z";
        updatedCount += 1;
      }
    }
    this.refreshSourceOverview(request.sourceId);
    return { sourceId: request.sourceId, updatedCount };
  }

  async deleteSource(
    meetingId: string,
    sourceId: string,
  ): Promise<DeleteTranscriptSourceResponse> {
    this.deleteCalls.push({ meetingId, sourceId });
    this.throwNextError();
    const index = this.sources.findIndex(
      (source) => source.meetingId === meetingId && source.id === sourceId,
    );
    if (index < 0) {
      throw new TranscriptApiError("transcript_source_not_found", false);
    }
    this.sources.splice(index, 1);
    this.segments.delete(sourceId);
    return { sourceId, deleted: true };
  }

  async getImportPolicy(): Promise<TranscriptImportPolicyResponse> {
    if (this.policyError) throw this.policyError;
    return { ...policy, allowedExtensions: [...policy.allowedExtensions] };
  }

  private async addSelectedSource(
    meetingId: string,
    meeting: Awaited<ReturnType<FakeMeetingApi["get"]>>,
  ): Promise<TranscriptImportResult> {
    const extension = this.nextSelectedType.replace("imported_", "");
    return this.addSource(
      meetingId,
      this.nextSelectedType,
      `Synthetic ${extension.toUpperCase()} transcript`,
      `synthetic.${extension}`,
      "Speaker 1: Selected synthetic transcript",
      this.nextDuplicate,
      meeting,
    );
  }

  private addSource(
    meetingId: string,
    sourceType: TranscriptSourceType,
    displayName: string,
    originalFilename: string | null,
    text: string,
    duplicate: boolean,
    meeting: Awaited<ReturnType<FakeMeetingApi["get"]>>,
  ): TranscriptImportResult {
    if (duplicate) {
      const existing = this.sources.find(
        (source) => source.meetingId === meetingId,
      );
      if (existing) {
        this.nextDuplicate = false;
        return {
          cancelled: false,
          duplicate: true,
          meeting: { ...meeting },
          source: cloneSource(existing),
          segmentCount: existing.segmentCount,
        };
      }
    }
    const sourceId = this.uuid();
    const lines = text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    const source = syntheticTranscriptSource({
      id: sourceId,
      meetingId,
      sourceType,
      displayName,
      originalFilename,
      segmentCount: Math.max(lines.length, 1),
    });
    const createdSegments = (lines.length > 0 ? lines : [text]).map(
      (line, ordinal) => {
        const speakerMatch = line.match(/^([^:：]{1,40})[:：]\s*(.+)$/);
        return syntheticTranscriptSegment({
          id: this.uuid(),
          sourceId,
          meetingId,
          ordinal,
          speakerLabel: speakerMatch?.[1] ?? null,
          originalText: speakerMatch?.[2] ?? line,
          startOffsetMs: null,
          endOffsetMs: null,
        });
      },
    );
    this.sources.push(source);
    this.segments.set(sourceId, createdSegments);
    this.refreshSourceOverview(sourceId);
    this.nextDuplicate = false;
    return {
      cancelled: false,
      duplicate: false,
      meeting: { ...meeting },
      source: cloneSource(source),
      segmentCount: createdSegments.length,
    };
  }

  private requireSource(
    meetingId: string,
    sourceId: string,
  ): TranscriptSourceResponse {
    const source = this.sources.find(
      (candidate) =>
        candidate.meetingId === meetingId && candidate.id === sourceId,
    );
    if (!source) {
      throw new TranscriptApiError("transcript_source_not_found", false);
    }
    return source;
  }

  private refreshSourceOverview(sourceId: string): void {
    const source = this.sources.find((candidate) => candidate.id === sourceId);
    if (!source) return;
    const entries = this.segments.get(sourceId) ?? [];
    source.segmentCount = entries.length;
    const starts = entries
      .map((segment) => segment.startOffsetMs)
      .filter((value): value is number => value !== null);
    const ends = entries
      .map((segment) => segment.endOffsetMs)
      .filter((value): value is number => value !== null);
    source.startOffsetMs = starts.length > 0 ? Math.min(...starts) : null;
    source.endOffsetMs = ends.length > 0 ? Math.max(...ends) : null;
    source.speakerCount = new Set(
      entries.map((segment) => segment.speakerLabel).filter(Boolean),
    ).size;
  }

  private throwNextError(): void {
    if (!this.nextError) return;
    const error = this.nextError;
    this.nextError = null;
    throw error;
  }

  private uuid(): string {
    this.sequence += 1;
    return `77777777-7777-4777-8777-${this.sequence.toString(16).padStart(12, "0")}`;
  }
}

function cancelledImport(): TranscriptImportResult {
  return {
    cancelled: true,
    duplicate: false,
    meeting: null,
    source: null,
    segmentCount: 0,
  };
}

function cloneSource(
  source: TranscriptSourceResponse,
): TranscriptSourceResponse {
  return { ...source };
}

function cloneSegment(
  segment: TranscriptSegmentResponse,
): TranscriptSegmentResponse {
  return { ...segment };
}
