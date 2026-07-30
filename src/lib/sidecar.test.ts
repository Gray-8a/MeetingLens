import { invoke } from "@tauri-apps/api/core";
import sourceCode from "./sidecar.ts?raw";
import {
  checkSidecarHealth,
  SidecarApiError,
  sidecarCommandName,
  toSidecarError,
  type SidecarStatus,
} from "./sidecar";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const authenticatedStatus: SidecarStatus = {
  status: "authenticated",
  binding: "loopback",
  session: "current",
  secretStorage: "not_implemented",
  service: "meetinglens-sidecar",
  version: "0.1.0",
  apiVersion: "v1",
};

describe("sidecar API", () => {
  beforeEach(() => invokeMock.mockReset());

  it("uses the single authenticated Rust command without request arguments", async () => {
    invokeMock.mockResolvedValue(authenticatedStatus);
    await expect(checkSidecarHealth()).resolves.toEqual(authenticatedStatus);
    expect(invokeMock).toHaveBeenCalledWith(sidecarCommandName);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("converts a stable backend error without trusting its message", () => {
    const converted = toSidecarError({
      code: "sidecar_timeout",
      message: "private backend details",
      retryable: true,
    });
    expect(converted.message).toBe(
      "The local service did not respond in time.",
    );
    expect(converted).toBeInstanceOf(SidecarApiError);
    expect(converted.code).toBe("sidecar_timeout");
    expect(converted.retryable).toBe(true);
  });

  it("converts unknown failures to a safe internal error", () => {
    const serialized = JSON.stringify(
      toSidecarError({ traceback: "private", path: "private" }),
    );
    expect(serialized).not.toContain("traceback");
    expect(serialized).not.toContain("private");
    expect(toSidecarError(null).code).toBe("internal_error");
  });

  it("defines no token, header, URL, port, or direct HTTP field", () => {
    const serialized = JSON.stringify(authenticatedStatus).toLowerCase();
    expect(serialized).not.toMatch(/token|authorization|bearer|https?:|8765/);
    expect(sourceCode).not.toMatch(/fetch\(|XMLHttpRequest|WebSocket/);
  });
});
