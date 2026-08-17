# MeetingLens

MeetingLens is a privacy-first, Windows-first desktop assistant currently following a **Personal Edition** baseline. Its near-term product path turns pasted or imported meeting transcripts into detailed, editable, evidence-linked minutes that can be copied or saved as Markdown. Summary is the only mode under active near-term development; Translation and the independent Meeting workspace remain a shared-engine long-term vision that is frozen/deferred rather than deleted.

PR #1–#22 are merged. The meeting data/UI foundation, Provider mocks, authenticated Sidecar boundary, Windows SecretStore, secure document import, isolated Docling Worker, transcript/summary specification, packaged zh-CN/en-US UI, Help page, bilingual preview guides, schema-v3 Transcript Data Core, safe Transcript Command API, Summary Mode Input UI, and Summary Run Core are complete. PR #21 delivered schema v4, persistent transcript snapshots, Summary series/revisions, run-owned evidence, recovery, and a Rust zero-network deterministic Mock; PR #22 aligned the completed-core status documentation. PR #23 is this documentation-only Personal Edition re-baseline. The sole planned next implementation stage is `feat/summary-provider-deepseek`, which has not been created or started and will require separate user approval; it will combine the Rust DeepSeek Provider with minimal outbound confirmation. The user-facing Generate action remains disabled; real DeepSeek requests, outbound confirmation, real AI minutes, and the formal result workspace are not available.

The TypeScript preview Provider runtime defines Capture, Transcription, Translation, Summary, and DocumentParser capability boundaries. Its current implementations are explicit deterministic in-memory mocks: they do not access devices, files, or networks, and can be replaced through an application-owned Registry without changing page business logic. Persistent Summary runs use the separate Rust deterministic Mock as their only current execution Provider.

The long-term three-mode vision is described in [the simplified three-mode specification](SIMPLIFIED_MODE_SPEC.md), product behavior in [the product specification](PRODUCT_SPEC.md), and the authoritative Personal Edition implementation order in [the development plan](DEVELOPMENT_PLAN.md). The shortest active route is the DeepSeek Provider with minimal consent, then the complete Summary minutes workspace, batch audio-file transcription, simplified Docling orchestration, and an optional user-confirmed window-context stage. Realtime capture/ASR, realtime translation, speaker diarization, Translation/Meeting workspaces, structured visuals, commercial distribution, and DOCX/PDF export are frozen/deferred.

Development-preview user guides are available in [简体中文](docs/USER_GUIDE.zh-CN.md) and [English](docs/USER_GUIDE.en-US.md).

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

Do not place real DeepSeek credentials in `.env` or environment variables. Configure the key through Settings so Rust stores it in Windows Credential Manager; any `.env` used for non-sensitive local flags must remain ignored and untracked.

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

The current fixed port remains a development contract, but the host is locked to `127.0.0.1` and every route requires the current ephemeral token. Random ports, packaged Sidecar binaries, restart monitoring, crash recovery, and orphan cleanup belong to the frozen/deferred realtime Sidecar stack and are not part of the current Personal Edition roadmap. The SecretStore is deliberately separate and is never used for the ephemeral Sidecar session token.

## Secure API key storage

On Windows, Settings can save or delete the approved DeepSeek API key through the Rust SecretStore backed by Windows Credential Manager. The UI can inspect only configured/unconfigured state and never reads a stored value back. The key is not written to SQLite, files, browser storage, logs, command-line arguments, or the Python Sidecar. This stage stores the credential only: it does not validate the key or call DeepSeek.

The production Credential Manager target is `com.gray.meetinglens/secret/deepseek-api-key`. Non-Windows builds fail closed without a plaintext fallback. Input is briefly present in WebView memory while the user enters and saves it; MeetingLens does not claim protection against malware running with the same Windows user privileges.

## Document storage boundary

The Rust `DocumentService` treats every source as untrusted and keeps source paths transient inside the Rust trust boundary. React can request the Rust-owned native picker and receives only safe document DTOs and typed progress events—never source or storage paths. The application capability grants only the six custom document commands; it does not grant WebView dialog, filesystem, shell, process, or HTTP permissions. Neither the current Python Sidecar nor DeepSeek receives file contents.

The initial allowlist is PDF, DOCX, PPTX, XLSX, TXT, Markdown, CSV, PNG, JPG, and JPEG. Limits are 100 MiB per file, 10 files per selection, 20 files per meeting, 500 MiB per meeting, and 255 UTF-8 bytes per original filename. Extension-only validation is insufficient; unknown, mismatched, empty, executable, script, macro, archive, HTML, and disguised double-extension files fail closed.

Files are streamed into private staging storage, hashed with SHA-256, validated, and atomically renamed to UUID-based paths. A process-wide mutex plus an application-lifetime cross-process storage lock prevents concurrent writers from bypassing quotas or recovery. Document and meeting deletion use persistent `deletion_jobs`, `deleting`/`delete_failed` states, same-volume trash moves, bounded startup retries, and explicit retry support. Unknown files are quarantined rather than deleted.

## Isolated document parser

PR #14 established a separate Python 3.12 environment under `workers/docling`; it is not installed into the authenticated HTTP Sidecar. Rust owns the parser process and launches one request per process with a cleared environment, no network listener, bounded standard I/O, a 150-second timeout, and a 4 GiB Windows Job Object memory cap. Inputs and outputs remain beneath Rust-approved staging roots and are revalidated by size and SHA-256. Model downloads are explicit and versioned outside the repository; see [the worker guide](workers/docling/README.md). Product parsing remains not yet implemented and is planned for the simplified `feat/document-parsing-orchestration` stage after batch audio-file transcription; it is not a prerequisite for transcript import or the first usable minutes.

The validated local model inventory is 564,981,412 bytes. Windows CPU smoke currently reports every allowlisted format available with RapidOCR/ONNX Runtime, a final acceptance peak process-tree RSS of about 1.67 GB, and no network listener or residual Worker process. These capabilities are independent acceptance results; the product does not expose parsing until `feat/document-parsing-orchestration`.

## Transcript-first summary path

MeetingLens supports bounded pasted UTF-8 text and Rust-owned native TXT/SRT/VTT import. Transcripts are distinct from reference documents: they use a lightweight local parser, do not require Docling, and persist normalized sources and ordered segments rather than copying the original transcript file indefinitely. Source paths never reach the WebView, and deleting a source does not change the user's original file. Platform-specific export adapters require sanitized real samples before support is claimed. Audio-file transcription is planned but not implemented; it will use a bounded one-shot local worker after the usable Summary loop. Video import remains deferred.

The Summary input workspace reads segments through stable ordinal pagination with a production page limit of 200 rather than loading up to 20,000 rows at once. Users may correct text or set a display name for matching source labels; revisions prevent silent overwrites, and source-level naming preserves the original label without inferring identity. The meeting model does not yet have an independent editable occurrence date, so the UI labels `created_at` only as creation time.

Transcript positions use optional relative millisecond offsets plus an optional source base time. Inputs without timestamps use a stable segment ordinal. Summary generation is eligible whenever the user has at least one valid input that can be frozen, including draft or in-progress meetings; it is not restricted to `ready`. Every run uses an immutable input snapshot so edits, source deletion, and regeneration cannot silently rewrite an existing or user-edited summary.

Schema v4 groups regenerations by `series_id` and a unique revision, constrains each child to the direct previous revision, and records provider provenance separately as `provider`, `provider_mode`, `model`, and `prompt_version`. Each run copies source/segment IDs, revisions, source/segment/global ordinals, speaker metadata, offsets, normalized text, language, confidence, finality, and snapshot time. A run accepts at most 20,000 final segments and 10 MiB of UTF-8 transcript text. The Rust Provider reads only this snapshot; every row contributes a run-local evidence ID, and unknown, duplicate, cross-run, or mismatched evidence fails before the structured result and cited evidence are atomically stored.

The current core exposes eight minimal commands through the centralized `summaryApi`: `get_summary_policy`, `create_summary_run`, `create_summary_revision`, `execute_summary_run`, `get_summary_run`, `list_summary_runs`, `cancel_summary_run`, and `list_summary_evidence`. Nested and top-level wire DTOs are camelCase; run/evidence lists use keyset pagination with a default of 50 and maximum of 100. The deterministic Mock does not read SecretStore, environment credentials, files, the Sidecar, or the network, and running cancellation reaches the active provider instance. Runs follow queued/running/completed/failed/cancelled; startup converts an abandoned running run to `failed` with `summary_interrupted` rather than replaying it.

The Summary Run Core Windows smoke is development-only and requires `VITE_SUMMARY_RUN_CORE_SMOKE=1`, a dedicated direct child of the operating-system temporary directory through `MEETINGLENS_SUMMARY_RUN_CORE_SMOKE_ROOT`, and `VITE_SUMMARY_RUN_CORE_SMOKE_ISOLATED=1`. Both launches must also merge a synthetic Tauri identifier named `com.gray.meetinglens.summarysmoke.<same-uuid>` so WebView2 storage is separate; Rust rejects a smoke root whose UUID does not match that identifier. This keeps the database and browser profile out of the normal MeetingLens locations. Release builds do not compile the Rust override, and production bundles exclude the browser harness.

Correcting a live segment marks dependent runs stale without changing their snapshots or results. Deleting a source used by a queued/running run is rejected; terminal-source deletion preserves only cited evidence, redacts full snapshot text and display metadata, and marks the result source-deleted/stale. Meeting deletion cancels active runs before the existing coordinator removes all summaries, snapshots, and evidence.

The planned initial templates are detailed meeting minutes, concise meeting summary, and actions/decisions, with Chinese, English, or bilingual output. Before the first DeepSeek transmission, the UI must explain that the selected transcript content will be sent to the external DeepSeek Provider and offer consent for this request, remember my choice, or cancel. A request without valid confirmation must fail closed. The remembered state is a minimal Personal Edition convenience, not an enterprise compliance or audit system. The complete result UI remains a later `feat/summary-mode-minutes` capability.

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
13. **Internationalization foundation — complete:** packaged zh-CN/en-US UI, locale preference, Help, and bilingual preview guides (`feat/i18n-user-guide-foundation`, PR #16).
14. **Direction alignment — complete:** freeze the three simple modes and shared pipeline (`docs/simplified-three-mode-spec`, PR #17).
15. **Transcript data core — complete:** schema v3, paste/TXT/SRT/VTT normalization, revisions, and stable Summary input (`feat/transcript-data-core`, PR #18).
16. **Transcript command boundary — complete:** secure commands, the Rust-native picker, and atomic meeting + transcript creation (`feat/transcript-command-api`, PR #19).
17. **Summary input UI — complete:** three-mode Home entry, paste/native import, paginated source review, and text/speaker corrections (`feat/summary-mode-input-ui`, PR #20).
18. **Summary run core — complete:** schema v4, run/series/revision, immutable transcript snapshots, evidence allowlist, stable recovery, and Rust zero-network Mock (`feat/summary-run-core`, PR #21).
19. **Status alignment — complete:** Summary Run Core status and acceptance documentation (`docs/summary-run-core-status`, PR #22).
20. **Current documentation re-baseline:** define the Personal Edition scope, shortest route, frozen capabilities, and risk-tiered acceptance (`docs/personal-edition-descope`, PR #23).
21. **Next — real Provider:** add the Rust DeepSeek Provider, consume the key only through SecretStore, strictly parse responses, preserve the evidence allowlist, and provide minimal first-outbound confirmation with an optional remembered choice (`feat/summary-provider-deepseek`; expected PR #24).
22. **Daily-use milestone:** add templates, Chinese/English/bilingual output, the formal results page, Markdown view/copy/save, body editing, and whole-result regeneration without overwriting earlier series revisions (`feat/summary-mode-minutes`; expected PR #25).
23. **Batch audio transcription:** import a meeting recording and transcribe it locally through a one-shot, CPU-int8 faster-whisper worker into the existing transcript model (`feat/audio-file-transcription`; expected PR #26). This is not realtime capture, WASAPI, realtime ASR, or realtime diarization.
24. **Simplified reference enrichment:** connect the completed Docling Worker, produce document chunks/evidence, retry one failed parse once, and otherwise ask the user to re-import (`feat/document-parsing-orchestration`; expected PR #27).
25. **Optional window context:** start only after the user confirms a repeated real need, and only for an explicitly selected window (`feat/window-context`; expected PR #28 or later).

Expected PR numbers are descriptive only; branch names and capability boundaries are authoritative. Realtime capture/ASR, the realtime Sidecar stack, realtime translation, speaker diarization, Translation/Meeting workspaces, checkpoints, structured visuals, installer/signing/SBOM/tray/auto-update work, and DOCX/PDF export are frozen/deferred, not deleted. When a single user capability has clear boundaries, one focused PR may include the necessary data layer, Rust commands, TypeScript API, and UI; unrelated capabilities and distinct security/network/device trust boundaries must not be bundled together.

## Pull request policy

Codex may develop, test, fix CI, commit, push, and open/update a PR. Auto-merge is disabled; the repository owner makes the final merge decision. Each PR delivers one clear user capability, but a well-bounded feature may include its necessary data, command/API, and UI layers together.

Acceptance is risk-tiered. Documentation PRs require Markdown/format/link/route checks and CI; ordinary features without schema changes require one local startup, their main user flow, relevant automation, and leak checks; schema, migration, security-boundary, or real-network changes retain migration/integrity/foreign-key, isolation or double-start when needed, failure-recovery, sensitive-data, and network-boundary validation. CI does not replace required Windows desktop acceptance.

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.

Direct dependencies are pinned to exact versions and lockfiles are committed. Docling is isolated in its own locked worker and is never installed in the foundation Sidecar. Other heavy AI components such as Diart, WhisperX, faster-whisper, and Screenpipe remain uninstalled until their assigned feature branches validate compatibility, licensing, model downloads, and Windows resource use.
