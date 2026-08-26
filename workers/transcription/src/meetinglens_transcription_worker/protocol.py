from __future__ import annotations

import json
import re
import uuid
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator, model_validator

from .constants import (
    MAX_INPUT_BYTES,
    MAX_PROTOCOL_LINE_BYTES,
    PROTOCOL_VERSION,
    SUPPORTED_MEDIA_TYPES,
    TranscriptionErrorCode,
)
from .errors import WorkerError

_SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
type SafeExtension = Literal["wav", "mp3", "m4a", "mp4"]
type DetectedMediaType = Literal["audio/wav", "audio/mpeg", "audio/mp4"]


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True, populate_by_name=True)


class BaseRequest(StrictModel):
    protocol_version: str = Field(alias="protocolVersion")
    request_id: str = Field(alias="requestId")

    @field_validator("protocol_version")
    @classmethod
    def validate_protocol_version(cls, value: str) -> str:
        if value != PROTOCOL_VERSION:
            raise ValueError("unsupported protocol")
        return value

    @field_validator("request_id")
    @classmethod
    def validate_request_id(cls, value: str) -> str:
        try:
            parsed = uuid.UUID(value)
        except ValueError as error:
            raise ValueError("invalid request id") from error
        if str(parsed) != value:
            raise ValueError("request id must be canonical")
        return value


class HealthRequest(BaseRequest):
    operation: Literal["health"]


class CapabilitiesRequest(BaseRequest):
    operation: Literal["capabilities"]


class TranscribeRequest(BaseRequest):
    operation: Literal["transcribe"]
    input_path: str = Field(alias="inputPath", min_length=1, max_length=4096)
    allowed_input_root: str = Field(alias="allowedInputRoot", min_length=1, max_length=4096)
    output_root: str = Field(alias="outputRoot", min_length=1, max_length=4096)
    expected_sha256: str = Field(alias="expectedSha256")
    expected_size_bytes: Annotated[int, Field(alias="expectedSizeBytes", ge=1, le=MAX_INPUT_BYTES)]
    detected_media_type: DetectedMediaType = Field(alias="detectedMediaType")
    safe_extension: SafeExtension = Field(alias="safeExtension")

    @field_validator("expected_sha256")
    @classmethod
    def validate_sha256(cls, value: str) -> str:
        if _SHA256_PATTERN.fullmatch(value) is None:
            raise ValueError("invalid sha256")
        return value

    @model_validator(mode="after")
    def validate_media_pair(self) -> TranscribeRequest:
        if SUPPORTED_MEDIA_TYPES[self.safe_extension] != self.detected_media_type:
            raise ValueError("media type does not match extension")
        return self


type WorkerRequest = HealthRequest | CapabilitiesRequest | TranscribeRequest


def decode_request(raw: bytes) -> WorkerRequest:
    if not raw or len(raw) > MAX_PROTOCOL_LINE_BYTES:
        raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR) from error
    if not isinstance(payload, dict):
        raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
    request_type: type[WorkerRequest]
    match payload.get("operation"):
        case "health":
            request_type = HealthRequest
        case "capabilities":
            request_type = CapabilitiesRequest
        case "transcribe":
            request_type = TranscribeRequest
        case _:
            raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
    try:
        return request_type.model_validate(payload)
    except ValidationError as error:
        raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR) from error


def encode_line(payload: dict[str, object]) -> bytes:
    try:
        raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise WorkerError(TranscriptionErrorCode.INTERNAL_ERROR) from error
    if len(raw) + 1 > MAX_PROTOCOL_LINE_BYTES:
        raise WorkerError(TranscriptionErrorCode.PROTOCOL_ERROR)
    return raw + b"\n"


def success_response(request_id: str, result: dict[str, object]) -> dict[str, object]:
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "type": "result",
        "ok": True,
        "result": result,
    }


def error_response(request_id: str, error: WorkerError) -> dict[str, object]:
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "type": "result",
        "ok": False,
        "error": error.response(),
    }
