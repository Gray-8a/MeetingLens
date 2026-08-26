# MeetingLens transcription worker

This directory contains the isolated, one-request-per-process audio transcription worker. It
supports user-selected WAV, MP3, M4A, and MP4 files and uses `faster-whisper==1.2.1` with the
pinned `Systran/faster-whisper-small` revision on CPU `int8`.

The controlled entry point used by the current source-checkout development flow is:

```powershell
uv run --project workers/transcription --frozen python -I -m meetinglens_transcription_worker
```

It accepts exactly one bounded JSON request on stdin. It writes bounded JSONL progress followed
by exactly one terminal result. Under the application protocol and implementation, transcript
text is exchanged only through the Rust-owned output staging directory as `segments.jsonl`;
stdout contains only progress and artifact metadata. This path boundary is not an AppContainer,
restricted-token, or ACL filesystem sandbox: the worker still has the current Windows user's OS
permissions. The worker
does not listen on a network port, receive credentials, call a cloud service, or launch system
`ffmpeg`. Audio decoding is provided by the locked PyAV wheel used by faster-whisper.

## Setup and tests

Python 3.12 and `uv 0.11.21` are required.

Rust currently resolves this checkout Worker only in debug builds. Release builds deliberately
report it as unavailable until the frozen installer/distribution work defines and audits a
packaged Worker and model layout.

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup-transcription-worker.ps1
npm run check:transcription-worker
```

Tests inject a deterministic fake engine and create synthetic bytes only in temporary folders.
They do not download the model, decode a real meeting recording, or access the network.

## Explicit model provisioning

Runtime model loading is always offline and verifies the complete local file inventory and every
SHA-256 digest. Provisioning is a separate, explicit action:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/provision-transcription-model.ps1
```

The default location is outside the repository:

```text
%LOCALAPPDATA%\MeetingLens\transcription\models\small-536b0662742c02347bc0e980a01041f333bce120
```

That directory contains `manifest.json` and a `model` directory. Use `-VerifyOnly` to re-check
the pinned repository, revision, runtime options, exact inventory, sizes, and hashes without any
download.

## Dependency and model license checkpoint

The material runtime licenses for this development-only Worker are recorded before any future
packaging work: `faster-whisper==1.2.1` and its locked CTranslate2 runtime are MIT; PyAV 16.0.1 is
BSD-3-Clause; the PyAV Windows wheel used in acceptance reports its bundled FFmpeg libraries as
LGPL version 3 or later; `huggingface-hub` is Apache-2.0; and the pinned
`Systran/faster-whisper-small` model revision is marked MIT by its model card. Commercial
distribution remains frozen. If packaging is later unfrozen, re-audit the complete lockfile and
model notices and satisfy all binary-redistribution obligations rather than treating this
checkpoint as a release SBOM or legal opinion.
