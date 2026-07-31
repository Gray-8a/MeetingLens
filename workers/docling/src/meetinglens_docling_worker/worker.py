from __future__ import annotations

import io
import logging
import sys
from contextlib import redirect_stderr, redirect_stdout
from typing import TextIO

from .constants import MAX_PROTOCOL_MESSAGE_BYTES, ParserErrorCode
from .docling_adapter import capabilities, health, parse
from .errors import WorkerError
from .protocol import (
    CapabilitiesRequest,
    HealthRequest,
    ParseRequest,
    decode_request,
    encode_response,
    error_response,
    success_response,
)
from .security import validate_environment


class BoundedDiscardSink(io.TextIOBase):
    def __init__(self, limit: int = 64 * 1024) -> None:
        self.limit = limit
        self.observed = 0

    def writable(self) -> bool:
        return True

    def write(self, value: str) -> int:
        encoded = value.encode("utf-8", errors="replace")
        self.observed = min(self.limit + 1, self.observed + len(encoded))
        return len(value)


def dispatch(request: HealthRequest | CapabilitiesRequest | ParseRequest) -> dict[str, object]:
    if isinstance(request, HealthRequest):
        return health()
    if isinstance(request, CapabilitiesRequest):
        return capabilities()
    if isinstance(request, ParseRequest):
        return parse(request)
    raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)


def run_once(stdin: TextIO, protocol_stdout: TextIO) -> int:
    request_id = "00000000-0000-0000-0000-000000000000"
    try:
        raw = stdin.buffer.readline(MAX_PROTOCOL_MESSAGE_BYTES + 1)
        if len(raw) > MAX_PROTOCOL_MESSAGE_BYTES:
            raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
        remainder = stdin.buffer.read(MAX_PROTOCOL_MESSAGE_BYTES + 1)
        if remainder.strip():
            raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
        request = decode_request(raw.rstrip(b"\r\n"))
        request_id = request.request_id
        validate_environment()
        sink = BoundedDiscardSink()
        logging.disable(logging.CRITICAL)
        with redirect_stdout(sink), redirect_stderr(sink):
            result = dispatch(request)
        payload = success_response(request_id, result)
    except WorkerError as error:
        payload = error_response(request_id, error)
    except Exception:
        payload = error_response(request_id, WorkerError(ParserErrorCode.INTERNAL_ERROR))
    protocol_stdout.buffer.write(encode_response(payload))
    protocol_stdout.buffer.flush()
    return 0


def main() -> int:
    return run_once(sys.stdin, sys.stdout)
