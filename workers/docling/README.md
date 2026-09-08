# MeetingLens Docling Worker

This directory contains the isolated, offline document-parser process established in PR #14 and wired into the product by the current `feat/document-parsing-orchestration` delivery. It is not the authenticated HTTP Sidecar and exposes no network listener. Rust starts one Python process per attempt, sends one bounded JSON Lines request on standard input, reads one bounded response from standard output, and terminates the complete Windows Job Object on success, cancellation, timeout, crash, or resource-limit failure.

## Scope

The Worker and current orchestration provide:

- an exact Python 3.12 dependency lock;
- health, capability, and parse protocol operations;
- stable safe error codes;
- offline CPU-only Docling conversion;
- bounded artifacts with hashes and relative paths;
- explicit local-model provisioning and compatibility verification;
- Rust process control, timeout, cancellation, memory limit, and output revalidation.
- protocol-v2 normalized `chunks.jsonl` for append-only schema-v7 `document_chunks`;
- product commands/status, private per-attempt staging, atomic persistence, and restart-safe recovery;
- immutable optional document snapshots and typed document evidence in transcript-backed Summary runs.

It does **not** create a general parsing-job scheduler, long-lived supervisor, checkpoint system, enterprise monitoring surface, release bundle, DeepSeek client, or remote-model path. One parse has at most two attempts: only a retryable first failure receives one automatic retry; the next/non-retryable failure asks the user to delete and re-import. Cancellation kills the process tree and cleans staging. On restart, interrupted parsing is marked failed safely and is never replayed. Pasted/TXT/SRT/VTT transcripts continue through the bounded Rust transcript parser and are never reparsed here.

Documents are optional references and remain semantically separate from transcripts. A Summary run still requires at least one transcript; combined ordinals are transcript first, and every decision/action item needs transcript evidence. Local parsing never calls DeepSeek. Only an explicit user Generate action plus Consent v2 may send normalized text/structure from selected immutable document snapshots; original files, paths, unselected documents, and Worker artifacts never leave the device.

## Locked environment

The worker pins `docling-slim==2.117.0` with only conversion core, PDF, Office, Markdown, local-model, and RapidOCR ONNX extras. It deliberately excludes ASR, VLM, service-client, HTML-renderer, remote-service, and all-extras bundles. `torch==2.13.0` and `torchvision==0.28.0` come from the official CPU wheel index. `uv==0.11.21` is required to reproduce `uv.lock`.

Prepare or verify the environment on Windows:

```powershell
.\scripts\setup-docling-worker.ps1
npm run check:docling-worker
```

CI installs the exact lock and runs offline protocol, security, artifact, real TXT/Markdown/CSV/DOCX, crash, timeout, cancellation, and Rust process-boundary tests. CI intentionally does not download model weights and therefore does not claim PDF, image, or OCR acceptance.

The locked Windows environment currently occupies approximately 1.21 GB after installation. On the validated Windows 11 x64 machine, a warm environment synchronization took under one second; the initial audited installation took about 23 seconds and the first Docling import about 35 seconds. These are development measurements, not product startup guarantees.

The 92 installed distributions were audited from locked metadata. The main parsing packages declare permissive licenses: Docling components and ONNX Runtime use MIT, RapidOCR uses Apache-2.0, Torch declares its bundled Apache/BSD/MIT-family notices, TorchVision uses BSD, and NumPy declares its bundled BSD/MIT/Zlib/CC0 notices. The existing Rust `windows` crate remains version 0.61.3 under MIT OR Apache-2.0; this PR only enables Job Object and threading features. Model licensing is tracked separately: the pinned layout artifacts are Apache-2.0, while the pinned TableFormer source documents CDLA-Permissive-2.0/Apache-2.0 terms. Distribution and model notices must be carried into the packaging/SBOM stage.

## Local models

Model downloads are never implicit. The explicit provisioning command uses `%LOCALAPPDATA%\MeetingLens\docling\models\2.117.0`, outside the repository, outside ordinary MeetingLens app-data, and outside the system temporary directory:

```powershell
.\scripts\provision-docling-models.ps1
.\scripts\provision-docling-models.ps1 -VerifyOnly
```

The generated manifest pins and inventories:

- Docling layout model revision `8f39ad3c0b4c58e9c2d2c84a38465abf757272d8`;
- TableFormer tag `v2.3.0`, revision `fc0f2d45e2218ea24bce5045f58a389aed16dc23`;
- RapidOCR release `v3.9.0` with the ONNX Runtime backend;
- every local artifact's relative path, size, and SHA-256.

Missing, incompatible, or modified models fail closed. Model files, parser staging, input documents, and generated artifacts are ignored and must never be committed.

Run the native acceptance smoke after model provisioning:

```powershell
uv run --project workers/docling --frozen python workers/docling/windows_smoke.py `
  --models-root "$env:LOCALAPPDATA\MeetingLens\docling\models\2.117.0"
```

The validated model set contains 14 inventoried files (564,981,412 bytes). The Windows CPU smoke parses programmatically generated TXT, Markdown, CSV, DOCX, PPTX, XLSX, digital PDF, PNG/JPEG OCR, and scanned-PDF OCR with networking forced offline. It also checks missing models, URL/root/hash rejection, page/output limits, artifact integrity/path hygiene, network listeners, and residual processes. The final acceptance run took about 82 seconds and observed a peak process-tree RSS of 1,340,301,312 bytes, below the fixed 4 GiB Job Object cap. The OCR fixtures use an explicit legible font, and every format must recover synthetic keywords into normalized chunks; both `.jpg` and `.jpeg` are exercised. No network listeners or residual Workers remained, and temporary data was removed. Results vary by CPU and available memory.

## Security contract

- Inputs must be regular files within a Rust-approved staging root and must match the registered size and SHA-256.
- URLs, UNC paths, alternate data streams, traversal, and arbitrary absolute output paths are rejected.
- The child receives a cleared environment plus a small allowlist; credentials, proxy settings, and session tokens are not inherited.
- Network-backed Docling services, plugins, and remote models are disabled.
- Limits are 300 pages, 100 MiB input, 128 MiB total artifacts, 1 MiB protocol messages, 150 seconds, and 4 GiB Job Object memory in production.
- Responses and errors never contain source text, absolute paths, SQL, raw exceptions, environment values, or credentials.
- Output is limited to `document.json`, `content.md`, `content.txt`, `chunks.jsonl`, and `manifest.json`, written atomically beneath the approved parser-staging root.

The adapter handles TXT, Markdown, CSV, DOCX, PPTX, and XLSX without model weights. PDF, PNG, JPG/JPEG, and OCR require the compatible local model set. Current product parsing is available only from a source checkout under debug/`tauri dev`; release Worker/model bundling remains Frozen. Rust persists only revalidated normalized chunks and stable status/error data, keeps paths and Worker stderr hidden from React, and never treats Docling output as transcript data.

The current PR also upgrades new DeepSeek Summary provenance to `summary-deepseek-v2`. A historical terminal v1 run with the same provider/mode/model may still parent a new revision; the old revision retains v1 provenance, while the child freezes the current v2 contract and current selected scope.
