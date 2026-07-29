# MeetingLens

MeetingLens is a privacy-first desktop workspace for meeting capture, transcription, translation, document context, speaker diarization, and verifiable AI summaries.

The current branch establishes the full product specification, a Tauri 2 + React + TypeScript application shell, and a dependency-free Python runtime sidecar whose health contract is verified through a real React → Rust → Python call path.

## Prerequisites

- Node.js 22 or newer
- npm 10 or newer
- Python 3.12.x
- Rust stable and the platform-specific [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for desktop builds

On Windows 11, install Microsoft C++ Build Tools with the Desktop development with C++ workload and the WebView2 runtime as described by Tauri before running the desktop shell.

## Setup

```bash
npm ci
python -m pip install --requirement requirements-dev.txt
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

The fixed `127.0.0.1:8765` address is a first-round development contract. The product implementation will use a Rust-selected random loopback port and an ephemeral session token.

## Project documents

- [Product specification](PRODUCT_SPEC.md)
- [Architecture](ARCHITECTURE.md)
- [Data model](DATA_MODEL.md)
- [Security](SECURITY.md)
- [Development plan](DEVELOPMENT_PLAN.md)
- [Agent workflow](AGENTS.md)

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.

Direct dependencies are pinned to exact versions and lockfiles are committed. Heavy AI components such as Docling, Diart, WhisperX, faster-whisper, and Screenpipe are deliberately not installed in the foundation sidecar; their compatibility, licensing, model downloads, and Windows resource use must be validated in their feature branches.
