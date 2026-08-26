from __future__ import annotations

from enum import StrEnum

PROTOCOL_VERSION = "1"
WORKER_NAME = "meetinglens-transcription-worker"
WORKER_VERSION = "0.1.0"
PARSER_VERSION = "faster-whisper/1.2.1"
FASTER_WHISPER_VERSION = "1.2.1"
DIRECT_DEPENDENCY_VERSIONS = {
    "av": "16.0.1",
    "faster-whisper": FASTER_WHISPER_VERSION,
    "huggingface-hub": "1.26.0",
    "pydantic": "2.13.4",
}

MODEL_REPOSITORY = "Systran/faster-whisper-small"
MODEL_REVISION = "536b0662742c02347bc0e980a01041f333bce120"
MODEL_MANIFEST_NAME = "manifest.json"
MODEL_MANIFEST_SCHEMA_VERSION = 1
MODEL_REQUIRED_FILES = (
    "config.json",
    "model.bin",
    "tokenizer.json",
    "vocabulary.txt",
)

MODELS_ROOT_ENV = "MEETINGLENS_TRANSCRIPTION_MODELS_ROOT"
STAGING_ROOT_ENV = "MEETINGLENS_TRANSCRIPTION_STAGING_ROOT"

MAX_PROTOCOL_LINE_BYTES = 64 * 1024
MAX_STDOUT_BYTES = 4 * 1024 * 1024
MAX_STDERR_BYTES = 64 * 1024
MAX_INPUT_BYTES = 2 * 1024 * 1024 * 1024
MAX_DURATION_MS = 4 * 60 * 60 * 1000
MAX_SEGMENTS = 20_000
MAX_SEGMENT_TEXT_BYTES = 64 * 1024
MAX_TRANSCRIPT_TEXT_BYTES = 10 * 1024 * 1024
MAX_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_CPU_THREADS = 4

SUPPORTED_MEDIA_TYPES = {
    "wav": "audio/wav",
    "mp3": "audio/mpeg",
    "m4a": "audio/mp4",
    "mp4": "audio/mp4",
}

TRANSCRIBE_OPTIONS: dict[str, object] = {
    "beamSize": 5,
    "conditionOnPreviousText": True,
    "task": "transcribe",
    "vadFilter": True,
    "wordTimestamps": False,
}


class TranscriptionErrorCode(StrEnum):
    INVALID_INPUT = "audio_invalid_input"
    UNSUPPORTED_TYPE = "audio_unsupported_type"
    TOO_LARGE = "audio_too_large"
    TOO_LONG = "audio_too_long"
    STREAM_MISSING = "audio_stream_missing"
    MODEL_MISSING = "audio_model_missing"
    MODEL_INCOMPATIBLE = "audio_model_incompatible"
    WORKER_UNAVAILABLE = "audio_worker_unavailable"
    WORKER_START_FAILED = "audio_worker_start_failed"
    WORKER_CRASHED = "audio_worker_crashed"
    PROTOCOL_ERROR = "audio_protocol_error"
    OUTPUT_TOO_LARGE = "audio_output_too_large"
    INTEGRITY_FAILED = "audio_integrity_failed"
    TIMEOUT = "audio_timeout"
    CANCELLED = "audio_cancelled"
    INTERNAL_ERROR = "internal_error"


SAFE_ERROR_DETAILS: dict[TranscriptionErrorCode, tuple[str, bool]] = {
    TranscriptionErrorCode.INVALID_INPUT: ("The selected media file is invalid.", False),
    TranscriptionErrorCode.UNSUPPORTED_TYPE: ("This media type is not supported.", False),
    TranscriptionErrorCode.TOO_LARGE: (
        "The selected media file exceeds the size limit.",
        False,
    ),
    TranscriptionErrorCode.TOO_LONG: (
        "The selected recording exceeds the duration limit.",
        False,
    ),
    TranscriptionErrorCode.STREAM_MISSING: (
        "The selected media does not contain a supported audio stream.",
        False,
    ),
    TranscriptionErrorCode.MODEL_MISSING: (
        "The required local transcription model is missing.",
        False,
    ),
    TranscriptionErrorCode.MODEL_INCOMPATIBLE: (
        "The local transcription model is incompatible.",
        False,
    ),
    TranscriptionErrorCode.WORKER_UNAVAILABLE: (
        "The local transcription worker is unavailable.",
        True,
    ),
    TranscriptionErrorCode.WORKER_START_FAILED: (
        "The local transcription worker could not start.",
        True,
    ),
    TranscriptionErrorCode.WORKER_CRASHED: (
        "The local transcription worker stopped unexpectedly.",
        True,
    ),
    TranscriptionErrorCode.PROTOCOL_ERROR: (
        "The local transcription worker returned an invalid response.",
        False,
    ),
    TranscriptionErrorCode.OUTPUT_TOO_LARGE: (
        "The transcription output exceeds the safety limit.",
        False,
    ),
    TranscriptionErrorCode.INTEGRITY_FAILED: (
        "A transcription integrity check failed.",
        False,
    ),
    TranscriptionErrorCode.TIMEOUT: ("Local transcription timed out.", True),
    TranscriptionErrorCode.CANCELLED: ("Local transcription was cancelled.", False),
    TranscriptionErrorCode.INTERNAL_ERROR: ("An internal error occurred.", False),
}
