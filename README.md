# MeetingLens

MeetingLens is a privacy-first Windows desktop assistant with three simple entry modes backed by one shared session engine: Summary, Translation, and Meeting. Its first real-value path turns pasted or imported meeting transcripts into detailed, editable, evidence-linked minutes; live audio, translation, and screen context extend the same transcript pipeline in later small releases.

PR #1–#15 are merged. The meeting data/UI foundation, Provider mocks, authenticated Sidecar boundary, Windows SecretStore, secure document import, isolated Docling Worker, and transcript/summary specification are complete. Docling is not connected to SQLite, Tauri commands, document state, or product UI and is retained for later reference-document enrichment. The current schema is v2; transcript persistence/import and real summary generation remain unimplemented.

The Provider runtime defines Capture, Transcription, Translation, Summary, and DocumentParser capability boundaries. Current implementations are explicit deterministic in-memory mocks: they do not access devices, files, or networks, and can be replaced through an application-owned Registry without changing page business logic.

The active product direction is defined in [the simplified three-mode specification](SIMPLIFIED_MODE_SPEC.md). The previous unimplemented roadmap is paused, while all merged foundations remain reusable. Delivery starts with a real Summary mode for paste/TXT/SRT/VTT, then adds a shared realtime transcription foundation, Translation mode, Meeting mode, and advanced screen intelligence in separate reviewable PRs.

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
.\scripts\setup-docling-worker.ps1
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
npm run check:docling-worker # Locked, offline Docling worker checks
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

The current fixed port remains a development contract, but the host is locked to `127.0.0.1` and every route requires the current ephemeral token. Random ports, packaged Sidecar binaries, restart monitoring, crash recovery, and orphan cleanup remain in `feat/sidecar-supervisor`. The SecretStore is deliberately separate and is never used for the ephemeral Sidecar session token.

## Secure API key storage

On Windows, Settings can save or delete the approved DeepSeek API key through the Rust SecretStore backed by Windows Credential Manager. The UI can inspect only configured/unconfigured state and never reads a stored value back. The key is not written to SQLite, files, browser storage, logs, command-line arguments, or the Python Sidecar. This stage stores the credential only: it does not validate the key or call DeepSeek.

The production Credential Manager target is `com.gray.meetinglens/secret/deepseek-api-key`. Non-Windows builds fail closed without a plaintext fallback. Input is briefly present in WebView memory while the user enters and saves it; MeetingLens does not claim protection against malware running with the same Windows user privileges.

## Document storage boundary

The Rust `DocumentService` treats every source as untrusted and keeps source paths transient inside the Rust trust boundary. React can request the Rust-owned native picker and receives only safe document DTOs and typed progress events—never source or storage paths. The application capability grants only the six custom document commands; it does not grant WebView dialog, filesystem, shell, process, or HTTP permissions. Neither the current Python Sidecar nor DeepSeek receives file contents.

The initial allowlist is PDF, DOCX, PPTX, XLSX, TXT, Markdown, CSV, PNG, JPG, and JPEG. Limits are 100 MiB per file, 10 files per selection, 20 files per meeting, 500 MiB per meeting, and 255 UTF-8 bytes per original filename. Extension-only validation is insufficient; unknown, mismatched, empty, executable, script, macro, archive, HTML, and disguised double-extension files fail closed.

Files are streamed into private staging storage, hashed with SHA-256, validated, and atomically renamed to UUID-based paths. A process-wide mutex plus an application-lifetime cross-process storage lock prevents concurrent writers from bypassing quotas or recovery. Document and meeting deletion use persistent `deletion_jobs`, `deleting`/`delete_failed` states, same-volume trash moves, bounded startup retries, and explicit retry support. Unknown files are quarantined rather than deleted.

## Isolated document parser

PR #14 established a separate Python 3.12 environment under `workers/docling`; it is not installed into the authenticated HTTP Sidecar. Rust owns the parser process and launches one request per process with a cleared environment, no network listener, bounded standard I/O, a 150-second timeout, and a 4 GiB Windows Job Object memory cap. Inputs and outputs remain beneath Rust-approved staging roots and are revalidated by size and SHA-256. Model downloads are explicit and versioned outside the repository; see [the worker guide](workers/docling/README.md). Database-backed parsing orchestration remains deferred to `feat/document-parsing-orchestration` and is not a prerequisite for transcript import or summary generation.

The validated local model inventory is 564,981,412 bytes. Windows CPU smoke currently reports every allowlisted format available with RapidOCR/ONNX Runtime, a final acceptance peak process-tree RSS of about 1.67 GB, and no network listener or residual Worker process. These capabilities are independent acceptance results; the product does not expose parsing until `feat/document-parsing-orchestration`.

## Transcript-first summary path

The first implementation supports pasted UTF-8 text and native TXT/SRT/VTT import. Transcripts are distinct from reference documents: they use a lightweight local parser, do not require Docling, and persist normalized sources and ordered segments rather than copying the original transcript file indefinitely. Platform-specific export adapters require sanitized real samples before support is claimed; video and audio-file transcription are future enhancements.

Transcript positions use optional relative millisecond offsets plus an optional source base time. Inputs without timestamps use a stable segment ordinal. Summary generation is eligible whenever the user has at least one valid input that can be frozen, including draft or in-progress meetings; it is not restricted to `ready`. Every run uses an immutable input snapshot so edits, source deletion, and regeneration cannot silently rewrite an existing or user-edited summary.

The initial templates are detailed meeting minutes, concise meeting summary, and actions/decisions. Output can be Chinese, English, or bilingual. Before any DeepSeek request, the UI must preview selected sources, segment count, time range, speakers, template, output language, and outbound data type, and require explicit confirmation.

## Project documents

- [Simplified three-mode specification](SIMPLIFIED_MODE_SPEC.md)
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
10. **MVP 1B-1B — complete:** Document commands, native picker, typed progress, and import/delete UI (`feat/document-import-ui`, PR #13).
11. **Complete:** isolated offline Docling Worker foundation (`feat/docling-worker`, PR #14); product wiring is intentionally deferred.
12. **Complete documentation baseline:** transcript/summary product and architecture alignment (`docs/transcript-and-summary-spec`, PR #15).
13. **Current direction alignment:** freeze the three simple modes and shared pipeline (`docs/simplified-three-mode-spec`).
14. **Decision gate:** the paused `feat/i18n-user-guide-foundation` PR remains untouched until the owner chooses whether to retain it. If retained, it must be updated on the new `main` and stripped of obsolete roadmap changes before merge.
15. **Summary mode foundation:** transcript data core (`feat/transcript-data-core`) and paste/TXT/SRT/VTT workspace (`feat/summary-mode-import`).
16. **First real minutes:** separate summary data/snapshot core, consent data core, DeepSeek Provider, and result workspace PRs; then editing/export.
17. **Shared live foundation:** live-session data, Sidecar supervisor, Windows audio capture, then realtime ASR into the same transcript segments.
18. **Translation mode:** language/checkpoint data core, realtime Provider, checkpoint runtime, then the user workspace.
19. **Meeting mode:** manual-note/bookmark data core, then the user workspace; screen context and advanced automation follow independently.
20. **Reference enrichment:** Docling orchestration and document evidence indexing remain optional later enhancements.

The authoritative branch mapping, exclusions, and acceptance criteria are in [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md). Database, UI, external Provider, audio, screen, and heavy-worker trust boundaries remain separate PRs. Screenpipe, if adopted, is an optional provider rather than a core dependency.

## Pull request policy

Codex may develop, test, fix CI, commit, push, and open/update a PR. Auto-merge is disabled; the repository owner makes the final merge decision. Each large phase is split into independently testable PRs.

CI success is not sufficient for Windows desktop behavior. PRs that touch native windows, credentials, files, devices, permissions, sidecar lifecycle, or packaging must include user-local acceptance with `npm run tauri dev` and the stage-specific workflow.

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.

Direct dependencies are pinned to exact versions and lockfiles are committed. Docling is isolated in its own locked worker and is never installed in the foundation Sidecar. Other heavy AI components such as Diart, WhisperX, faster-whisper, and Screenpipe remain uninstalled until their assigned feature branches validate compatibility, licensing, model downloads, and Windows resource use.
