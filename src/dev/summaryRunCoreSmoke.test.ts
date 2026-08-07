import { describe, expect, it } from "vitest";

import {
  browserStorageContainsForbiddenValue,
  isExternalNetworkTarget,
  summaryContentStorageProbes,
} from "./summaryRunCoreSmoke";

describe("summary run core smoke network classification", () => {
  it("treats Tauri IPC and loopback transports as local", () => {
    expect(isExternalNetworkTarget("http://ipc.localhost/command")).toBe(false);
    expect(isExternalNetworkTarget("ws://localhost:1420/hmr")).toBe(false);
    expect(isExternalNetworkTarget("http://127.0.0.1:1420/")).toBe(false);
  });

  it("counts public HTTP and WebSocket targets as external", () => {
    expect(isExternalNetworkTarget("https://example.invalid/api")).toBe(true);
    expect(isExternalNetworkTarget("wss://example.invalid/socket")).toBe(true);
  });

  it("finds forbidden atoms in browser-storage keys and values", () => {
    expect(
      browserStorageContainsForbiddenValue(
        storageWith([["safe-key", "prefix decision-text suffix"]]),
        ["decision-text"],
      ),
    ).toBe(true);

    expect(
      browserStorageContainsForbiddenValue(
        storageWith([["owner-name", "id-only"]]),
        ["owner-name"],
      ),
    ).toBe(true);
    expect(
      browserStorageContainsForbiddenValue(
        storageWith([["owner-name", "id-only"]]),
        ["due-date"],
      ),
    ).toBe(false);
  });

  it("allows ID-only browser-storage state", () => {
    expect(
      browserStorageContainsForbiddenValue(
        storageWith([
          [
            "meetinglens.summary-run-core-smoke.state.v1",
            '{"meetingId":"123e4567-e89b-42d3-a456-426614174000"}',
          ],
        ]),
        ["transcript body", "decision text", "evidence excerpt", "owner name"],
      ),
    ).toBe(false);
  });

  it("builds atomic probes for every sensitive result field", () => {
    expect(
      summaryContentStorageProbes({
        schemaVersion: 1,
        overview: { text: "overview-text", evidenceIds: [crypto.randomUUID()] },
        decisions: [
          {
            text: "decision-text",
            status: "confirmed",
            evidenceIds: [crypto.randomUUID()],
          },
        ],
        actionItems: [
          {
            text: "action-text",
            owner: "owner-name",
            dueDate: "due-date",
            status: "assigned",
            evidenceIds: [crypto.randomUUID()],
          },
        ],
        risks: [{ text: "risk-text", evidenceIds: [crypto.randomUUID()] }],
        openQuestions: [
          { text: "question-text", evidenceIds: [crypto.randomUUID()] },
        ],
      }),
    ).toEqual([
      "overview-text",
      "decision-text",
      "action-text",
      "owner-name",
      "due-date",
      "risk-text",
      "question-text",
    ]);
  });
});

function storageWith(
  entries: readonly (readonly [string, string])[],
): Pick<Storage, "length" | "key" | "getItem"> {
  const values = new Map(entries);
  const keys = [...values.keys()];
  return {
    length: keys.length,
    key: (index) => keys[index] ?? null,
    getItem: (key) => values.get(key) ?? null,
  };
}
