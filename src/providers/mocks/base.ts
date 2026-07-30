import { ProviderError, type ProviderErrorCode } from "../errors";
import type {
  ProviderKind,
  ProviderMode,
  ProviderOperation,
  ProviderStatus,
  ProviderStatusListener,
} from "../types";

export type TimerHandle = ReturnType<typeof setTimeout>;

export interface MockTimer {
  setTimeout(handler: () => void, delayMs: number): TimerHandle;
  clearTimeout(handle: TimerHandle): void;
}

export const systemTimer: MockTimer = {
  setTimeout: (handler, delayMs) => setTimeout(handler, delayMs),
  clearTimeout: (handle) => clearTimeout(handle),
};

export abstract class MockProviderBase {
  abstract readonly kind: ProviderKind;
  readonly mode: ProviderMode = "mock";
  abstract readonly name: string;
  private readonly statusListeners = new Set<ProviderStatusListener>();
  protected status: ProviderStatus;

  protected constructor(initialStatus: ProviderStatus) {
    this.status = initialStatus;
  }

  getStatus(): ProviderStatus {
    return this.status;
  }

  subscribeStatus(listener: ProviderStatusListener): () => void {
    this.statusListeners.add(listener);
    return () => this.statusListeners.delete(listener);
  }

  protected setStatus(status: ProviderStatus): void {
    this.status = status;
    for (const listener of this.statusListeners) listener(status);
  }

  protected ensureUsable(operation: ProviderOperation): void {
    if (this.status === "disposed") {
      throw this.error("provider_disposed", operation);
    }
    if (this.status === "error") {
      throw this.error("provider_unavailable", operation, true);
    }
  }

  protected error(
    code: ProviderErrorCode,
    operation: ProviderOperation,
    retryable = false,
  ): ProviderError {
    return new ProviderError(code, this.kind, operation, retryable);
  }

  protected markDisposed(): void {
    this.setStatus("disposed");
    this.statusListeners.clear();
  }
}

export abstract class MockAsyncProviderBase extends MockProviderBase {
  private readonly pending = new Map<
    string,
    { handle: TimerHandle; reject: (reason: ProviderError) => void }
  >();

  protected constructor(
    initialStatus: ProviderStatus,
    private readonly timer: MockTimer,
    private readonly delayMs: number,
  ) {
    super(initialStatus);
  }

  protected runOperation<T>(
    key: string,
    operation: ProviderOperation,
    output: () => T,
    shouldFail = false,
  ): Promise<T> {
    this.ensureUsable(operation);
    if (this.pending.has(key)) {
      throw this.error("provider_already_running", operation);
    }
    this.setStatus("running");

    return new Promise<T>((resolve, reject) => {
      const handle = this.timer.setTimeout(() => {
        this.pending.delete(key);
        if (shouldFail) {
          this.setStatus("error");
          reject(this.error("provider_operation_failed", operation, true));
          return;
        }
        if (this.pending.size === 0 && this.status !== "error") {
          this.setStatus("ready");
        }
        resolve(output());
      }, this.delayMs);
      this.pending.set(key, { handle, reject });
    });
  }

  protected cancelOperation(key: string): void {
    this.ensureUsable("cancel");
    const pending = this.pending.get(key);
    if (!pending) throw this.error("provider_invalid_state", "cancel");
    this.timer.clearTimeout(pending.handle);
    this.pending.delete(key);
    pending.reject(this.error("provider_cancelled", "cancel"));
    if (this.pending.size === 0) this.setStatus("ready");
  }

  protected disposeOperations(): void {
    for (const pending of this.pending.values()) {
      this.timer.clearTimeout(pending.handle);
      pending.reject(this.error("provider_disposed", "dispose"));
    }
    this.pending.clear();
    this.markDisposed();
  }
}
