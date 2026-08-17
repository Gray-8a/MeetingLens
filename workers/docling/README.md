# MeetingLens Docling Worker

This directory contains the isolated, offline document-parser process delivered in PR #14. It is not the existing authenticated HTTP Sidecar and it exposes no network listener. Rust starts one Python process per request, sends one bounded JSON Lines request on standard input, reads one bounded response from standard output, and terminates the complete Windows Job Object on success, cancellation, timeout, crash, or resource-limit failure.

## Scope

This PR provides only the parser foundation:

- an exact Python 3.12 dependency lock;
- health, capability, and parse protocol operations;
- stable safe error codes;
- offline CPU-only Docling conversion;
- bounded artifacts with hashes and relative paths;
- explicit local-model provisioning and compatibility verification;
- Rust process control, timeout, cancellation, memory limit, and output revalidation.

It does **not** add Tauri commands, WebView access, database migrations, parsing jobs, document state transitions, product UI, background supervision, packaging, DeepSeek calls, or remote model access. The foundation was merged in PR #14 and is retained as an optional reference-document enhancement. Under the Personal Edition roadmap, product wiring is planned for the single simplified `feat/document-parsing-orchestration` stage after `feat/audio-file-transcription`. That stage may add document chunks/evidence and one automatic retry; a second failure asks the user to re-import rather than introducing a general scheduler, long-lived checkpoints, multi-level recovery, or an enterprise monitoring surface. It is not a prerequisite for pasted/TXT/SRT/VTT transcripts or the first usable transcript-only minutes. Realtime capture work is Frozen/Deferred, and future PR numbers are descriptive rather than authoritative.

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

Model downloads are never implicit. The explicit provisioning command defaults to a versioned directory under `%LOCALAPPDATA%\MeetingLens`, outside the repository and outside the system temporary directory:

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

The validated model set contains 14 inventoried files (564,981,412 bytes). The Windows CPU smoke parses programmatically generated TXT, Markdown, CSV, DOCX, PPTX, XLSX, digital PDF, PNG/JPEG OCR, and scanned-PDF OCR with networking forced offline. It also checks missing models, URL/root/hash rejection, page/output limits, artifact integrity/path hygiene, network listeners, and residual processes. The final acceptance run took about 78 seconds and observed a peak process-tree RSS of 1,665,912,832 bytes, below the fixed 4 GiB Job Object cap. Results vary by CPU and available memory.

## Security contract

- Inputs must be regular files within a Rust-approved staging root and must match the registered size and SHA-256.
- URLs, UNC paths, alternate data streams, traversal, and arbitrary absolute output paths are rejected.
- The child receives a cleared environment plus a small allowlist; credentials, proxy settings, and session tokens are not inherited.
- Network-backed Docling services, plugins, and remote models are disabled.
- Limits are 300 pages, 100 MiB input, 128 MiB total artifacts, 1 MiB protocol messages, 150 seconds, and 4 GiB Job Object memory in production.
- Responses and errors never contain source text, absolute paths, SQL, raw exceptions, environment values, or credentials.
- Output is limited to `document.json`, `content.md`, `content.txt`, and `manifest.json`, written atomically beneath the approved parser-staging root.

The current adapter handles TXT, Markdown, CSV, DOCX, PPTX, and XLSX without model weights. PDF, PNG, JPEG, and OCR require the compatible local model set. The later simplified orchestration PR must persist only Rust-revalidated artifacts and stable status/error data, keep paths hidden from React, retry a failed parse at most once automatically, and never send source files externally without an explicit future scope change.
