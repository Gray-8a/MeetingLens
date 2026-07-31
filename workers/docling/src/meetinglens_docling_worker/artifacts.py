from __future__ import annotations

import hashlib
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .constants import (
    DOCLING_VERSION,
    MAX_DOCUMENT_JSON_BYTES,
    MAX_MARKDOWN_BYTES,
    MAX_TEXT_BYTES,
    MAX_TOTAL_ARTIFACT_BYTES,
    PROTOCOL_VERSION,
    WORKER_NAME,
    ParserErrorCode,
)
from .errors import WorkerError


@dataclass(frozen=True, slots=True)
class ArtifactPayloads:
    document: dict[str, Any]
    markdown: str
    text: str
    page_count: int | None
    has_ocr: bool
    warnings: list[str]


def write_artifacts(
    *,
    output_root: Path,
    request_id: str,
    document_id: str,
    input_sha256: str,
    detected_media_type: str,
    duration_ms: int,
    payloads: ArtifactPayloads,
) -> dict[str, object]:
    document_bytes = _json_bytes(payloads.document)
    markdown_bytes = payloads.markdown.encode("utf-8")
    text_bytes = payloads.text.encode("utf-8")
    limits = (
        (document_bytes, MAX_DOCUMENT_JSON_BYTES),
        (markdown_bytes, MAX_MARKDOWN_BYTES),
        (text_bytes, MAX_TEXT_BYTES),
    )
    if any(len(data) > limit for data, limit in limits):
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
    if sum(len(data) for data, _ in limits) > MAX_TOTAL_ARTIFACT_BYTES:
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)

    artifact_specs = (
        ("document_json", "document.json", document_bytes),
        ("markdown", "content.md", markdown_bytes),
        ("text", "content.txt", text_bytes),
    )
    artifacts = [
        _artifact_entry(kind, relative_path, data) for kind, relative_path, data in artifact_specs
    ]

    manifest: dict[str, object] = {
        "schemaVersion": 1,
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "documentId": document_id,
        "parserName": WORKER_NAME,
        "parserVersion": DOCLING_VERSION,
        "inputSha256": input_sha256,
        "detectedMediaType": detected_media_type,
        "status": "success",
        "pageCount": payloads.page_count,
        "hasOcr": payloads.has_ocr,
        "warnings": payloads.warnings,
        "durationMs": duration_ms,
        "artifacts": artifacts,
    }
    manifest_bytes = _json_bytes(manifest)
    if len(manifest_bytes) + sum(len(data) for data, _ in limits) > MAX_TOTAL_ARTIFACT_BYTES:
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
    manifest_artifact = _artifact_entry("manifest", "manifest.json", manifest_bytes)

    try:
        for _, relative_path, data in artifact_specs:
            _write_atomic(output_root, relative_path, data)
        _write_atomic(output_root, "manifest.json", manifest_bytes)
    except WorkerError:
        _cleanup_fixed_artifacts(output_root)
        raise

    return {
        "manifest": manifest,
        "manifestArtifact": manifest_artifact,
    }


def _json_bytes(payload: object) -> bytes:
    try:
        return (
            json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise WorkerError(ParserErrorCode.FAILED) from error


def _write_atomic(output_root: Path, relative_path: str, data: bytes) -> None:
    target = output_root / relative_path
    temporary = output_root / f".{relative_path}.tmp"
    try:
        with temporary.open("xb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.replace(target)
    except OSError as error:
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass
        raise WorkerError(ParserErrorCode.FAILED) from error


def _artifact_entry(kind: str, relative_path: str, data: bytes) -> dict[str, object]:
    return {
        "kind": kind,
        "relativePath": relative_path,
        "sizeBytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def _cleanup_fixed_artifacts(output_root: Path) -> None:
    for name in (
        "document.json",
        "content.md",
        "content.txt",
        "manifest.json",
        ".document.json.tmp",
        ".content.md.tmp",
        ".content.txt.tmp",
        ".manifest.json.tmp",
    ):
        try:
            (output_root / name).unlink(missing_ok=True)
        except OSError:
            pass
