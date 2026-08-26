from __future__ import annotations

import hashlib
import json
from pathlib import Path

from meetinglens_transcription_worker.constants import (
    FASTER_WHISPER_VERSION,
    MODEL_MANIFEST_NAME,
    MODEL_MANIFEST_SCHEMA_VERSION,
    MODEL_REPOSITORY,
    MODEL_REQUIRED_FILES,
    MODEL_REVISION,
    TRANSCRIBE_OPTIONS,
)


def write_synthetic_model(models_root: Path) -> dict[str, object]:
    model_directory = models_root / "model"
    model_directory.mkdir(parents=True)
    files = []
    for name in MODEL_REQUIRED_FILES:
        data = f"synthetic-{name}".encode()
        path = model_directory / name
        path.write_bytes(data)
        files.append(
            {
                "relativePath": f"model/{name}",
                "sizeBytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        )
    files.sort(key=lambda item: str(item["relativePath"]))
    manifest: dict[str, object] = {
        "schemaVersion": MODEL_MANIFEST_SCHEMA_VERSION,
        "repository": MODEL_REPOSITORY,
        "revision": MODEL_REVISION,
        "fasterWhisperVersion": FASTER_WHISPER_VERSION,
        "device": "cpu",
        "computeType": "int8",
        "options": TRANSCRIBE_OPTIONS,
        "preparedAt": "2026-01-01T00:00:00Z",
        "totalSizeBytes": sum(int(item["sizeBytes"]) for item in files),
        "files": files,
    }
    (models_root / MODEL_MANIFEST_NAME).write_text(
        json.dumps(manifest, separators=(",", ":"), sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return manifest
