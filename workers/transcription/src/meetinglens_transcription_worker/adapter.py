from __future__ import annotations

import importlib.metadata
import math
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

import av

from .constants import (
    FASTER_WHISPER_VERSION,
    MAX_CPU_THREADS,
    MAX_DURATION_MS,
    MAX_SEGMENT_TEXT_BYTES,
    MAX_SEGMENTS,
    MAX_TRANSCRIPT_TEXT_BYTES,
    TranscriptionErrorCode,
)
from .errors import WorkerError


@dataclass(frozen=True, slots=True)
class TranscriptSegment:
    start_offset_ms: int
    end_offset_ms: int
    text: str


@dataclass(frozen=True, slots=True)
class TranscriptPayload:
    language: str
    language_probability: float | None
    duration_ms: int
    segments: tuple[TranscriptSegment, ...]


type ProgressCallback = Callable[[int, int], None]


class TranscriptionEngine(Protocol):
    def transcribe(self, input_path: Path, progress: ProgressCallback) -> TranscriptPayload: ...


class FasterWhisperEngine:
    def __init__(self, model_directory: Path) -> None:
        self.model_directory = model_directory

    def transcribe(self, input_path: Path, progress: ProgressCallback) -> TranscriptPayload:
        _verify_dependency()
        probed_duration_ms = probe_audio(input_path)
        try:
            from faster_whisper import WhisperModel

            model = WhisperModel(
                str(self.model_directory),
                device="cpu",
                compute_type="int8",
                cpu_threads=MAX_CPU_THREADS,
                num_workers=1,
                local_files_only=True,
            )
            raw_segments, info = model.transcribe(
                str(input_path),
                beam_size=5,
                condition_on_previous_text=True,
                task="transcribe",
                vad_filter=True,
                word_timestamps=False,
            )
            duration_ms = _duration_ms(getattr(info, "duration", None), probed_duration_ms)
            language = _language(getattr(info, "language", None))
            language_probability = _probability(getattr(info, "language_probability", None))
            segments = _collect_segments(raw_segments, duration_ms, progress)
            return TranscriptPayload(
                language=language,
                language_probability=language_probability,
                duration_ms=duration_ms,
                segments=segments,
            )
        except WorkerError:
            raise
        except MemoryError as error:
            raise WorkerError(TranscriptionErrorCode.TOO_LARGE) from error
        except Exception as error:
            raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED) from error


def probe_audio(input_path: Path) -> int:
    try:
        with av.open(str(input_path), mode="r", metadata_errors="ignore") as container:
            if not any(stream.type == "audio" for stream in container.streams):
                raise WorkerError(TranscriptionErrorCode.STREAM_MISSING)
            duration = container.duration
            if duration is None or duration <= 0:
                return 0
            duration_ms = int(round(float(duration) / float(av.time_base) * 1000.0))
            _validate_duration(duration_ms)
            return duration_ms
    except WorkerError:
        raise
    except (av.FFmpegError, OSError, ValueError, OverflowError) as error:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT) from error


def _collect_segments(
    raw_segments: Iterable[Any],
    duration_ms: int,
    progress: ProgressCallback,
) -> tuple[TranscriptSegment, ...]:
    segments: list[TranscriptSegment] = []
    total_text_bytes = 0
    previous_end = 0
    for raw in raw_segments:
        if len(segments) >= MAX_SEGMENTS:
            raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        text = " ".join(str(getattr(raw, "text", "")).split())
        if any(ord(character) < 32 or ord(character) == 127 for character in text):
            raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED)
        if not text:
            continue
        text_bytes = len(text.encode("utf-8"))
        if text_bytes > MAX_SEGMENT_TEXT_BYTES:
            raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        total_text_bytes += text_bytes
        if total_text_bytes > MAX_TRANSCRIPT_TEXT_BYTES:
            raise WorkerError(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        try:
            raw_start = float(getattr(raw, "start"))
            raw_end = float(getattr(raw, "end"))
        except (TypeError, ValueError, OverflowError) as error:
            raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED) from error
        if not math.isfinite(raw_start) or not math.isfinite(raw_end) or raw_end <= raw_start:
            raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED)
        start_ms = max(previous_end, int(round(raw_start * 1000.0)))
        end_ms = max(start_ms + 1, int(round(raw_end * 1000.0)))
        if end_ms > MAX_DURATION_MS or (duration_ms and end_ms > duration_ms + 1000):
            raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED)
        segment = TranscriptSegment(start_ms, end_ms, text)
        segments.append(segment)
        previous_end = end_ms
        progress(min(end_ms, duration_ms), duration_ms)
    if not segments:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
    progress(duration_ms, duration_ms)
    return tuple(segments)


def _duration_ms(raw_duration: object, fallback: int) -> int:
    try:
        seconds = float(raw_duration)
    except (TypeError, ValueError, OverflowError):
        seconds = 0.0
    if math.isfinite(seconds) and seconds > 0:
        duration_ms = int(round(seconds * 1000.0))
    else:
        duration_ms = fallback
    if duration_ms <= 0:
        raise WorkerError(TranscriptionErrorCode.WORKER_CRASHED)
    _validate_duration(duration_ms)
    return duration_ms


def _validate_duration(duration_ms: int) -> None:
    if duration_ms > MAX_DURATION_MS:
        raise WorkerError(TranscriptionErrorCode.TOO_LONG)


def _language(value: object) -> str:
    if not isinstance(value, str):
        return "und"
    language = value.strip().lower()
    if 2 <= len(language) <= 35 and all(
        character.isascii() and (character.isalnum() or character == "-") for character in language
    ):
        return language
    return "und"


def _probability(value: object) -> float | None:
    try:
        probability = float(value)
    except (TypeError, ValueError, OverflowError):
        return None
    if not math.isfinite(probability) or not 0.0 <= probability <= 1.0:
        return None
    return probability


def _verify_dependency() -> None:
    try:
        version = importlib.metadata.version("faster-whisper")
    except importlib.metadata.PackageNotFoundError as error:
        raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE) from error
    if version != FASTER_WHISPER_VERSION:
        raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE)
