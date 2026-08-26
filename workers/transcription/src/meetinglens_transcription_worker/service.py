from __future__ import annotations

import importlib.metadata
import os
import sys
from collections.abc import Callable
from pathlib import Path

from .adapter import FasterWhisperEngine, TranscriptionEngine
from .artifacts import write_artifacts
from .constants import (
    DIRECT_DEPENDENCY_VERSIONS,
    FASTER_WHISPER_VERSION,
    MAX_ARTIFACT_BYTES,
    MAX_CPU_THREADS,
    MAX_DURATION_MS,
    MAX_INPUT_BYTES,
    MAX_SEGMENT_TEXT_BYTES,
    MAX_SEGMENTS,
    MAX_TRANSCRIPT_TEXT_BYTES,
    MODEL_REPOSITORY,
    MODEL_REVISION,
    MODELS_ROOT_ENV,
    PARSER_VERSION,
    PROTOCOL_VERSION,
    SUPPORTED_MEDIA_TYPES,
    WORKER_NAME,
    WORKER_VERSION,
    TranscriptionErrorCode,
)
from .errors import WorkerError
from .models import model_directory, models_status, require_models
from .protocol import CapabilitiesRequest, HealthRequest, TranscribeRequest
from .security import validate_existing_directory, validate_transcribe_paths

type EngineFactory = Callable[[Path], TranscriptionEngine]
type ProgressEmitter = Callable[[str, int | None, int | None, int | None], None]


def health() -> dict[str, object]:
    _verify_runtime()
    return {
        "status": "ok",
        "service": WORKER_NAME,
        "workerVersion": WORKER_VERSION,
        "parserVersion": PARSER_VERSION,
        "fasterWhisperVersion": FASTER_WHISPER_VERSION,
        "protocolVersion": PROTOCOL_VERSION,
        "pythonVersion": (
            f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"
        ),
        "transport": "stdio-jsonl",
        "networkListener": False,
        "remoteServices": False,
        "device": "cpu",
        "computeType": "int8",
        "systemFfmpegRequired": False,
    }


def capabilities() -> dict[str, object]:
    _verify_runtime()
    model_root = _models_root(required=False)
    present, compatible, _ = (
        models_status(model_root) if model_root is not None else (False, False, None)
    )
    model_state = "available" if compatible else "incompatible" if present else "missing"
    return {
        "formats": [
            {"safeExtension": extension, "detectedMediaType": media_type}
            for extension, media_type in SUPPORTED_MEDIA_TYPES.items()
        ],
        "model": {
            "repository": MODEL_REPOSITORY,
            "revision": MODEL_REVISION,
            "state": model_state,
        },
        "limits": {
            "maxInputBytes": MAX_INPUT_BYTES,
            "maxDurationMs": MAX_DURATION_MS,
            "maxSegments": MAX_SEGMENTS,
            "maxSegmentTextBytes": MAX_SEGMENT_TEXT_BYTES,
            "maxTranscriptTextBytes": MAX_TRANSCRIPT_TEXT_BYTES,
            "maxArtifactBytes": MAX_ARTIFACT_BYTES,
            "maxCpuThreads": MAX_CPU_THREADS,
        },
    }


def transcribe(
    request: TranscribeRequest,
    emit: ProgressEmitter,
    engine_factory: EngineFactory = FasterWhisperEngine,
) -> dict[str, object]:
    _verify_runtime()
    emit("validating_input", None, None, None)
    paths = validate_transcribe_paths(request)
    emit("verifying_model", None, None, None)
    models_root = _models_root(required=True)
    assert models_root is not None
    require_models(models_root)
    emit("loading_model", None, None, None)
    engine = engine_factory(model_directory(models_root))
    last_permille = -5

    def on_progress(processed_ms: int, total_ms: int) -> None:
        nonlocal last_permille
        if total_ms <= 0:
            return
        permille = min(1000, max(0, processed_ms * 1000 // total_ms))
        if permille < 1000 and permille < last_permille + 5:
            return
        last_permille = permille
        emit("transcribing", processed_ms, total_ms, permille)

    payload = engine.transcribe(paths.input_path, on_progress)
    emit("writing_artifacts", payload.duration_ms, payload.duration_ms, 1000)
    return write_artifacts(output_root=paths.output_root, request=request, payload=payload)


def dispatch(
    request: HealthRequest | CapabilitiesRequest | TranscribeRequest,
    emit: ProgressEmitter,
    engine_factory: EngineFactory = FasterWhisperEngine,
) -> dict[str, object]:
    if isinstance(request, HealthRequest):
        return health()
    if isinstance(request, CapabilitiesRequest):
        return capabilities()
    if isinstance(request, TranscribeRequest):
        return transcribe(request, emit, engine_factory)
    raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)


def _models_root(*, required: bool) -> Path | None:
    value = os.environ.get(MODELS_ROOT_ENV)
    if not value:
        if required:
            raise WorkerError(TranscriptionErrorCode.MODEL_MISSING)
        return None
    try:
        return validate_existing_directory(Path(value), TranscriptionErrorCode.MODEL_MISSING)
    except WorkerError:
        if required:
            raise
        return None


def _verify_runtime() -> None:
    if sys.version_info[:2] != (3, 12):
        raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE)
    for distribution, expected in DIRECT_DEPENDENCY_VERSIONS.items():
        try:
            version = importlib.metadata.version(distribution)
        except importlib.metadata.PackageNotFoundError as error:
            raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE) from error
        if version != expected:
            raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE)
