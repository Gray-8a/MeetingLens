# MeetingLens

MeetingLens is a privacy-first desktop workspace for meeting capture, transcription, translation, document context, speaker diarization, and verifiable AI summaries.

Phase 0 through MVP 1A and the MVP 1B-1A storage core are complete. The current `feat/document-import-ui` stage implements MVP 1B-1B: Rust-owned native file selection, a path-free Document Command API, typed import progress, and meeting document import/delete UI. Docling parsing, document preview/open, drag-and-drop, and DeepSeek document processing remain unimplemented.

The Provider runtime defines Capture, Transcription, Translation, Summary, and DocumentParser capability boundaries. Current implementations are explicit deterministic in-memory mocks: they do not access devices, files, or networks, and can be replaced through an application-owned Registry without changing page business logic.

## Prerequisites

- Node.js 22 or newer
- npm 10 or newer
- Python 3.12.x
- Rust stable and the platform-specific Tauri desktop prerequisites

On Windows 11, install Microsoft C++ Build Tools with the **Desktop development with C++** workload, including MSVC v143, a Windows 10/11 SDK, and C++ CMake tools. WebView2 is also required; current Windows 11 installations normally include it. See the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

Verify the native toolchain from a Visual Studio Developer PowerShell/Command Prompt:

```powershell
where.exe link
rustup show
cargo --version
npm --version
py -3.12 --version
```

An ordinary PowerShell session may not place `link.exe` directly on `PATH`; Cargo can still discover an installed Visual Studio toolchain. Use the Developer shell when diagnosing linker or SDK problems.

## Setup

```powershell
npm ci
py -3.12 -m pip install --requirement requirements-dev.txt
```

Keep development credentials in `.env`. The file is ignored by Git and must never be committed.

## Commands

```bash
npm run dev             # React/Vite development server
npm run tauri dev       # Tauri desktop development mode (requires Rust)
npm run sidecar         # Controlled protocol tests only; fails closed without a session token
npm run lint            # ESLint
npm run format          # Prettier + Ruff (writes files)
npm run format:check    # Prettier + Ruff verification
npm run typecheck       # TypeScript
npm run test            # React unit tests
npm run test:sidecar    # Python sidecar tests
npm run build           # Frontend production build
npm run check           # All locally runnable frontend/Python checks
```

For the integrated authenticated health check, start Tauri directly:

```powershell
npm run tauri dev
```

Rust creates an ephemeral session token, starts the Python 3.12 Sidecar, passes the token only through the child environment, and injects authentication into `/health`. The Settings page reports only safe state such as authenticated/unavailable, loopback binding, and current application session. A browser-only Vite preview has no Sidecar bridge.

An unauthenticated direct request must fail closed:

```powershell
try { Invoke-WebRequest http://127.0.0.1:8765/health } catch { $_.Exception.Response.StatusCode.value__ }
```

The current fixed port remains a development contract, but the host is locked to `127.0.0.1` and every route requires the current ephemeral token. Random ports, packaged Sidecar binaries, restart monitoring, crash recovery, and orphan cleanup remain in the MVP 1C supervisor. The SecretStore is deliberately separate and is never used for the ephemeral Sidecar session token.

## Secure API key storage

On Windows, Settings can save or delete the approved DeepSeek API key through the Rust SecretStore backed by Windows Credential Manager. The UI can inspect only configured/unconfigured state and never reads a stored value back. The key is not written to SQLite, files, browser storage, logs, command-line arguments, or the Python Sidecar. This stage stores the credential only: it does not validate the key or call DeepSeek.

The production Credential Manager target is `com.gray.meetinglens/secret/deepseek-api-key`. Non-Windows builds fail closed without a plaintext fallback. Input is briefly present in WebView memory while the user enters and saves it; MeetingLens does not claim protection against malware running with the same Windows user privileges.

## Document storage boundary

The Rust `DocumentService` treats every source as untrusted and keeps source paths transient inside the Rust trust boundary. React can request the Rust-owned native picker and receives only safe document DTOs and typed progress events—never source or storage paths. The application capability grants only the six custom document commands; it does not grant WebView dialog, filesystem, shell, process, or HTTP permissions. Neither the current Python Sidecar nor DeepSeek receives file contents.

The initial allowlist is PDF, DOCX, PPTX, XLSX, TXT, Markdown, CSV, PNG, JPG, and JPEG. Limits are 100 MiB per file, 10 files per selection, 20 files per meeting, 500 MiB per meeting, and 255 UTF-8 bytes per original filename. Extension-only validation is insufficient; unknown, mismatched, empty, executable, script, macro, archive, HTML, and disguised double-extension files fail closed.

Files are streamed into private staging storage, hashed with SHA-256, validated, and atomically renamed to UUID-based paths. A process-wide mutex plus an application-lifetime cross-process storage lock prevents concurrent writers from bypassing quotas or recovery. Document and meeting deletion use persistent `deletion_jobs`, `deleting`/`delete_failed` states, same-volume trash moves, bounded startup retries, and explicit retry support. Unknown files are quarantined rather than deleted.

## Project documents

- [Product specification](PRODUCT_SPEC.md)
- [Architecture](ARCHITECTURE.md)
- [Data model](DATA_MODEL.md)
- [Security](SECURITY.md)
- [Development plan](DEVELOPMENT_PLAN.md)
- [Agent workflow](AGENTS.md)

## Delivery roadmap

1. **Phase 0 — complete:** specification and runnable technical baseline.
2. **MVP 1A-1 — complete:** SQLite, migrations, meeting CRUD, deletion semantics, and persistence (`feat/meeting-data-core`, PR #3).
3. **MVP 1A-2-1 — complete:** Meeting Tauri Command API and typed TypeScript client (`feat/meeting-command-api`, PR #6).
4. **MVP 1A-2-2 — complete:** desktop UI and complete synthetic meeting flow (`feat/meeting-ui-mock`, PR #7).
5. **MVP 1A-3A — complete:** Provider contracts, Registry, and deterministic in-memory mocks (`feat/provider-contracts-mocks`, PR #8).
6. **MVP 1A-3B-1 — complete:** authenticated local Sidecar session and loopback boundary (`feat/local-sidecar-auth`, PR #9).
7. **MVP 1A-3B-2 — complete:** SecretStore and Windows Credential Manager (`feat/provider-security-foundation`, PR #10).
8. **MVP 1B-0 — complete:** document import, storage, lifecycle, and deletion specification (`docs/define-document-import-lifecycle`, PR #11).
9. **MVP 1B-1A — complete:** document storage core, safe import, deduplication, and recoverable deletion (`feat/document-storage-core`, PR #12).
10. **MVP 1B-1B — current:** Document commands, native picker, typed progress, and import/delete UI (`feat/document-import-ui`).
11. **MVP 1B-2:** isolated Docling Worker and structured parsing (`feat/docling-worker`).
12. **MVP 1B-3:** DeepSeek SummaryProvider, evidence links, and export (`feat/deepseek-summary-provider`).
13. **MVP 1C:** sidecar supervisor, Windows audio, and realtime transcription in separate PRs.
14. **MVP 2:** realtime translation and speaker diarization.
15. **MVP 3:** meeting-window selection, OCR, and screen context.
16. **MVP 4:** Windows packaging and product hardening.

The authoritative branch mapping, exclusions, and acceptance criteria are in [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md). Storage, import UI, Docling, and DeepSeek are separate PRs. Screenpipe, if adopted, is an optional provider rather than a core dependency.

## Pull request policy

Codex may develop, test, fix CI, commit, push, and open/update a PR. Auto-merge is disabled; the repository owner makes the final merge decision. Each large phase is split into independently testable PRs.

CI success is not sufficient for Windows desktop behavior. PRs that touch native windows, credentials, files, devices, permissions, sidecar lifecycle, or packaging must include user-local acceptance with `npm run tauri dev` and the stage-specific workflow.

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.

Direct dependencies are pinned to exact versions and lockfiles are committed. Heavy AI components such as Docling, Diart, WhisperX, faster-whisper, and Screenpipe are deliberately not installed in the foundation sidecar; their compatibility, licensing, model downloads, and Windows resource use must be validated in their assigned feature branches.
