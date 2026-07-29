# MeetingLens

MeetingLens is a privacy-first desktop workspace for meeting capture, transcription, translation, document context, speaker diarization, and verifiable AI summaries.

Phase 0 is complete: the repository contains the product specification, a Tauri 2 + React + TypeScript application shell, and a dependency-free Python runtime sidecar whose health contract is verified through a real React → Rust → Python call path. The next implementation PR is strictly `feat/meeting-data-core` (MVP 1A-1).

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
npm run sidecar         # Python sidecar on 127.0.0.1:8765
npm run lint            # ESLint
npm run format          # Prettier + Ruff (writes files)
npm run format:check    # Prettier + Ruff verification
npm run typecheck       # TypeScript
npm run test            # React unit tests
npm run test:sidecar    # Python sidecar tests
npm run build           # Frontend production build
npm run check           # All locally runnable frontend/Python checks
```

For the integrated health check, start the sidecar in one PowerShell terminal, then start Tauri in another:

```powershell
npm run sidecar
npm run tauri dev
```

Select **Check Python sidecar** in the desktop window. A browser-only Vite preview intentionally disables this button because it has no Tauri command bridge.

You can also verify the narrow HTTP contract directly:

```powershell
Invoke-RestMethod http://127.0.0.1:8765/health
```

The fixed `127.0.0.1:8765` address is a Phase 0 development contract. MVP 1A-3 defines the random loopback port and ephemeral session-token contract; MVP 1C wires it into the sidecar supervisor lifecycle.

## Project documents

- [Product specification](PRODUCT_SPEC.md)
- [Architecture](ARCHITECTURE.md)
- [Data model](DATA_MODEL.md)
- [Security](SECURITY.md)
- [Development plan](DEVELOPMENT_PLAN.md)
- [Agent workflow](AGENTS.md)

## Delivery roadmap

1. **Phase 0 — complete:** specification and runnable technical baseline.
2. **MVP 1A-1:** SQLite, migrations, meeting CRUD, deletion semantics, and persistence (`feat/meeting-data-core`).
3. **MVP 1A-2:** desktop UI and complete synthetic meeting flow (`feat/meeting-ui-mock`).
4. **MVP 1A-3:** Provider abstraction, session token, SecretStore, and security foundation (`feat/provider-security-foundation`).
5. **MVP 1B-1:** isolated document validation and Docling parsing (`feat/document-processing`).
6. **MVP 1B-2:** staged DeepSeek minutes, evidence links, and export (`feat/deepseek-summary`).
7. **MVP 1C:** sidecar supervisor, Windows audio, and realtime transcription in separate PRs.
8. **MVP 2:** realtime translation and speaker diarization.
9. **MVP 3:** meeting-window selection, OCR, and screen context.
10. **MVP 4:** Windows packaging and product hardening.

The authoritative branch mapping, exclusions, and acceptance criteria are in [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md). Docling and DeepSeek are separate PRs. Screenpipe, if adopted, is an optional provider rather than a core dependency.

## Pull request policy

Codex may develop, test, fix CI, commit, push, and open/update a PR. Auto-merge is disabled; the repository owner makes the final merge decision. Each large phase is split into independently testable PRs.

CI success is not sufficient for Windows desktop behavior. PRs that touch native windows, credentials, files, devices, permissions, sidecar lifecycle, or packaging must include user-local acceptance with `npm run tauri dev` and the stage-specific workflow.

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.

Direct dependencies are pinned to exact versions and lockfiles are committed. Heavy AI components such as Docling, Diart, WhisperX, faster-whisper, and Screenpipe are deliberately not installed in the foundation sidecar; their compatibility, licensing, model downloads, and Windows resource use must be validated in their assigned feature branches.
