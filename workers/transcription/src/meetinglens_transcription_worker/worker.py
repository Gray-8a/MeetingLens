from __future__ import annotations

import io
import logging
import os
import sys
from contextlib import redirect_stderr, redirect_stdout
from typing import TextIO

from .adapter import FasterWhisperEngine
from .constants import (
    MAX_PROTOCOL_LINE_BYTES,
    MAX_STDERR_BYTES,
    MAX_STDOUT_BYTES,
    PROTOCOL_VERSION,
    TranscriptionErrorCode,
)
from .errors import WorkerError
from .protocol import decode_request, encode_line, error_response, success_response
from .security import validate_environment
from .service import EngineFactory, dispatch

_ZERO_REQUEST_ID = "00000000-0000-0000-0000-000000000000"


class BoundedDiscardSink(io.TextIOBase):
    def __init__(self, limit: int = MAX_STDERR_BYTES) -> None:
        self.limit = limit
        self.observed = 0

    def writable(self) -> bool:
        return True

    def write(self, value: str) -> int:
        encoded = value.encode("utf-8", errors="replace")
        self.observed = min(self.limit + 1, self.observed + len(encoded))
        return len(value)


class JsonlEmitter:
    def __init__(self, protocol_stdout: TextIO, request_id: str) -> None:
        self.protocol_stdout = protocol_stdout
        self.request_id = request_id
        self.sequence = 0
        self.written = 0

    def progress(
        self,
        stage: str,
        processed_ms: int | None,
        total_ms: int | None,
        progress_permille: int | None,
    ) -> None:
        self.sequence += 1
        self.write(
            {
                "protocolVersion": PROTOCOL_VERSION,
                "requestId": self.request_id,
                "type": "progress",
                "sequence": self.sequence,
                "stage": stage,
                "processedMs": processed_ms,
                "totalMs": total_ms,
                "progressPermille": progress_permille,
            }
        )

    def write(self, payload: dict[str, object]) -> None:
        raw = encode_line(payload)
        if self.written + len(raw) > MAX_STDOUT_BYTES:
            raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
        self.protocol_stdout.buffer.write(raw)
        self.protocol_stdout.buffer.flush()
        self.written += len(raw)


def run_once(
    stdin: TextIO,
    protocol_stdout: TextIO,
    *,
    engine_factory: EngineFactory = FasterWhisperEngine,
) -> int:
    request_id = _ZERO_REQUEST_ID
    emitter = JsonlEmitter(protocol_stdout, request_id)
    try:
        raw = stdin.buffer.readline(MAX_PROTOCOL_LINE_BYTES + 1)
        if len(raw) > MAX_PROTOCOL_LINE_BYTES:
            raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
        remainder = stdin.buffer.read(MAX_PROTOCOL_LINE_BYTES + 1)
        if remainder.strip():
            raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
        request = decode_request(raw.rstrip(b"\r\n"))
        request_id = request.request_id
        emitter.request_id = request_id
        validate_environment()
        os.environ["HF_HUB_OFFLINE"] = "1"
        os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
        sink = BoundedDiscardSink()
        logging.disable(logging.CRITICAL)
        with redirect_stdout(sink), redirect_stderr(sink):
            result = dispatch(request, emitter.progress, engine_factory)
        payload = success_response(request_id, result)
    except WorkerError as error:
        payload = error_response(request_id, error)
    except Exception:
        payload = error_response(
            request_id,
            WorkerError(TranscriptionErrorCode.INTERNAL_ERROR),
        )
    emitter.write(payload)
    return 0


def main() -> int:
    return run_once(sys.stdin, sys.stdout)
