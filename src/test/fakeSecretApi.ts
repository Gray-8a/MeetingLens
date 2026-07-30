import type {
  DeleteSecretResult,
  SecretApi,
  SecretKind,
  SecretStatus,
} from "../api/secret";

export class FakeSecretApi implements SecretApi {
  configured = false;
  available = true;
  getStatusError: unknown = null;
  setError: unknown = null;
  deleteError: unknown = null;
  setPending: Promise<void> | null = null;
  setCallCount = 0;
  deleteCallCount = 0;

  async getStatus(kind: SecretKind): Promise<SecretStatus> {
    if (this.getStatusError !== null) throw this.getStatusError;
    return this.status(kind);
  }

  async set(kind: SecretKind, value: string): Promise<SecretStatus> {
    void value;
    this.setCallCount += 1;
    if (this.setPending !== null) await this.setPending;
    if (this.setError !== null) throw this.setError;
    this.configured = true;
    return this.status(kind);
  }

  async delete(kind: SecretKind): Promise<DeleteSecretResult> {
    this.deleteCallCount += 1;
    if (this.deleteError !== null) throw this.deleteError;
    const deleted = this.configured;
    this.configured = false;
    return { kind, deleted, configured: false };
  }

  private status(kind: SecretKind): SecretStatus {
    return {
      kind,
      configured: this.configured,
      available: this.available,
      backend: "windows_credential_manager",
    };
  }
}
