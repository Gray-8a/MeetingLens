from __future__ import annotations

import hashlib
import json
import os
import re
from pathlib import Path

from .adapter import TranscriptPayload
from .constants import (
    FASTER_WHISPER_VERSION,
    MAX_ARTIFACT_BYTES,
    MAX_DURATION_MS,
    MAX_PROTOCOL_LINE_BYTES,
    MAX_SEGMENT_TEXT_BYTES,
    MAX_SEGMENTS,
    MAX_TRANSCRIPT_TEXT_BYTES,
    MODEL_REPOSITORY,
    MODEL_REVISION,
    PARSER_VERSION,
    PROTOCOL_VERSION,
    WORKER_NAME,
    TranscriptionErrorCode,
)
from .errors import WorkerError
from .protocol import TranscribeRequest

_LANGUAGE_PATTERN = re.compile(r"^(?:und|[a-z]{2,3}(?:-[a-z0-9]{2,8})*)$")


def write_artifacts(
    *,
    output_root: Path,
    request: TranscribeRequest,
    payload: TranscriptPayload,
) -> dict[str, object]:
    segment_lines: list[bytes] = []
    total_text_bytes = 0
    previous_end = 0
    if not 1 <= len(payload.segments) <= MAX_SEGMENTS:
        raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
    if not 1 <= payload.duration_ms <= MAX_DURATION_MS:
        raise WorkerError(TranscriptionErrorCode.TOO_LONG)
    if _LANGUAGE_PATTERN.fullmatch(payload.language) is None:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED)

    for segment in payload.segments:
        text_bytes = segment.text.encode("utf-8")
        if (
            not segment.text.strip()
            or len(text_bytes) > MAX_SEGMENT_TEXT_BYTES
            or segment.start_offset_ms < previous_end
            or segment.end_offset_ms <= segment.start_offset_ms
            or segment.end_offset_ms > payload.duration_ms + 1000
        ):
            raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED)
        total_text_bytes += len(text_bytes)
        if total_text_bytes > MAX_TRANSCRIPT_TEXT_BYTES:
            raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        line = _json_line(
            {
                "startOffsetMs": segment.start_offset_ms,
                "endOffsetMs": segment.end_offset_ms,
                "text": segment.text,
            }
        )
        if len(line) > MAX_PROTOCOL_LINE_BYTES:
            raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        segment_lines.append(line)
        previous_end = segment.end_offset_ms

    segments_bytes = b"".join(segment_lines)
    segments_artifact = _artifact_entry("segments", "segments.jsonl", segments_bytes)
    manifest: dict[str, object] = {
        "schemaVersion": 1,
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request.request_id,
        "parserName": WORKER_NAME,
        "parserVersion": PARSER_VERSION,
        "inputSha256": request.expected_sha256,
        "model": MODEL_REPOSITORY,
        "modelRevision": MODEL_REVISION,
        "fasterWhisperVersion": FASTER_WHISPER_VERSION,
        "device": "cpu",
        "computeType": "int8",
        "language": payload.language,
        "durationMs": payload.duration_ms,
        "segmentCount": len(payload.segments),
        "beamSize": 5,
        "task": "transcribe",
        "status": "success",
        "segmentsArtifact": segments_artifact,
    }
    manifest_bytes = _json_line(manifest)
    if len(segments_bytes) + len(manifest_bytes) > MAX_ARTIFACT_BYTES:
        raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
    manifest_artifact = _artifact_entry("manifest", "manifest.json", manifest_bytes)

    try:
        _write_atomic(output_root, "segments.jsonl", segments_bytes)
        _write_atomic(output_root, "manifest.json", manifest_bytes)
    except WorkerError:
        _cleanup_fixed_artifacts(output_root)
        raise
    return {
        "parserVersion": PARSER_VERSION,
        "language": payload.language,
        "durationMs": payload.duration_ms,
        "segmentCount": len(payload.segments),
        "segmentsArtifact": segments_artifact,
        "manifestArtifact": manifest_artifact,
    }


def _json_line(payload: object) -> bytes:
    try:
        return (
            json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise WorkerError(TranscriptionErrorCode.INTERNAL_ERROR) from error


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
        raise WorkerError(TranscriptionErrorCode.INTERNAL_ERROR) from error


def _artifact_entry(kind: str, relative_path: str, data: bytes) -> dict[str, object]:
    return {
        "kind": kind,
        "relativePath": relative_path,
        "sizeBytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def _cleanup_fixed_artifacts(output_root: Path) -> None:
    for name in (
        "segments.jsonl",
        "manifest.json",
        ".segments.jsonl.tmp",
        ".manifest.json.tmp",
    ):
        try:
            (output_root / name).unlink(missing_ok=True)
        except OSError:
            pass
