import type {
  DocumentParserProvider,
  DocumentReference,
  DocumentValidationResult,
  ParsedDocument,
} from "../contracts";
import { MockAsyncProviderBase, systemTimer, type MockTimer } from "./base";

export interface MockDocumentParserOptions {
  delayMs?: number;
  failRequestIds?: ReadonlySet<string>;
  timer?: MockTimer;
}

const mockMimeTypes = new Set([
  "application/pdf",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  "text/plain",
  "text/markdown",
]);

export class MockDocumentParserProvider
  extends MockAsyncProviderBase
  implements DocumentParserProvider
{
  readonly kind = "document-parser" as const;
  readonly name = "Mock document parser";
  private readonly failRequestIds: ReadonlySet<string>;

  constructor(options: MockDocumentParserOptions = {}) {
    super("ready", options.timer ?? systemTimer, options.delayMs ?? 25);
    this.failRequestIds = options.failRequestIds ?? new Set();
  }

  async validate(
    reference: DocumentReference,
  ): Promise<DocumentValidationResult> {
    this.ensureUsable("validate");
    return this.validateReference(reference);
  }

  parse(
    requestId: string,
    reference: DocumentReference,
  ): Promise<ParsedDocument> {
    this.ensureUsable("parse");
    const validation = this.validateReference(reference);
    if (!validation.valid || requestId.trim().length === 0) {
      throw this.error("unsupported_capability", "parse");
    }

    return this.runOperation(
      requestId,
      "parse",
      () => ({
        requestId,
        sections: [
          {
            sectionId: "mock-section-1",
            title: "Mock section",
            text: "Synthetic in-memory document content.",
            pageNumber: 1,
          },
        ],
        pages: 1,
        tables: 0,
        metadata: { source: "mock-memory" },
        warnings: ["Mock output; the referenced file was not accessed."],
        isMock: true,
      }),
      this.failRequestIds.has(requestId),
    );
  }

  async cancel(requestId: string): Promise<void> {
    this.cancelOperation(requestId);
  }

  async dispose(): Promise<void> {
    if (this.getStatus() === "disposed") return;
    this.disposeOperations();
  }

  private validateReference(
    reference: DocumentReference,
  ): DocumentValidationResult {
    const safeName =
      reference.originalFilename.length > 0 &&
      reference.originalFilename.length <= 255 &&
      !reference.originalFilename.includes("/") &&
      !reference.originalFilename.includes("\\") &&
      !reference.originalFilename.includes("..") &&
      ![...reference.originalFilename].some((character) => {
        const codePoint = character.codePointAt(0) ?? 0;
        return codePoint < 32 || codePoint === 127;
      });
    const valid =
      reference.artifactId.trim().length > 0 &&
      reference.sizeBytes > 0 &&
      reference.sizeBytes <= 10_000_000 &&
      mockMimeTypes.has(reference.mimeType) &&
      safeName;
    return {
      valid,
      warnings: valid
        ? ["Mock validation only; no file was opened or inspected."]
        : ["The synthetic document reference is not supported."],
    };
  }
}
