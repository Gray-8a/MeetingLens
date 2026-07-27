# MeetingLens

MeetingLens is a privacy-first desktop workspace for meeting capture, transcription, translation, document context, speaker diarization, and verifiable AI summaries.

The current branch establishes the product specification, Tauri 2 + React + TypeScript application shell, and a dependency-free Python sidecar health endpoint.

## Prerequisites

- Node.js 22 or newer
- npm 10 or newer
- Python 3.10 or newer
- Rust stable and the platform-specific [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for desktop builds

## Setup

```bash
npm ci
```

Keep development credentials in `.env`. The file is ignored by Git and must never be committed.

## Commands

```bash
npm run dev             # React/Vite development server
npm run tauri dev       # Tauri desktop development mode (requires Rust)
npm run sidecar         # Python sidecar on 127.0.0.1:8765
npm run lint            # ESLint
npm run typecheck       # TypeScript
npm run test            # React unit tests
npm run test:sidecar    # Python sidecar tests
npm run build           # Frontend production build
npm run check           # All locally runnable frontend/Python checks
```

Verify the sidecar in another terminal:

```bash
curl http://127.0.0.1:8765/health
```

## Project documents

- [Product specification](PRODUCT_SPEC.md)
- [Architecture](ARCHITECTURE.md)
- [Data model](DATA_MODEL.md)
- [Security](SECURITY.md)
- [Development plan](DEVELOPMENT_PLAN.md)
- [Agent workflow](AGENTS.md)

## Privacy

Do not place real recordings, transcripts, screenshots, uploaded meeting documents, local databases, logs, or credentials in this repository. Tests and examples use synthetic data only.
