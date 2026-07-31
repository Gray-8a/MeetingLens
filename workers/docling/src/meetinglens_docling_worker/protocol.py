from __future__ import annotations

import json
import re
import uuid
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator

from .constants import (
    DOCLING_TIMEOUT_SECONDS,
    MAX_PAGES,
    MAX_PROTOCOL_MESSAGE_BYTES,
    PROTOCOL_VERSION,
    ParserErrorCode,
)
from .errors import WorkerError

_SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
type SafeExtension = Literal[
    "pdf", "docx", "pptx", "xlsx", "txt", "md", "csv", "png", "jpg", "jpeg"
]


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


class ParserOptions(StrictModel):
    ocr: bool = False
    max_pages: Annotated[int, Field(alias="maxPages", ge=1, le=MAX_PAGES)] = MAX_PAGES
    document_timeout_seconds: Annotated[
        float,
        Field(alias="documentTimeoutSeconds", gt=0, le=DOCLING_TIMEOUT_SECONDS),
    ] = DOCLING_TIMEOUT_SECONDS


class ParseRequest(BaseRequest):
    operation: Literal["parse"]
    document_id: str = Field(alias="documentId")
    meeting_id: str = Field(alias="meetingId")
    input_path: str = Field(alias="inputPath", min_length=1, max_length=4096)
    allowed_input_root: str = Field(alias="allowedInputRoot", min_length=1, max_length=4096)
    expected_sha256: str = Field(alias="expectedSha256")
    detected_media_type: str = Field(alias="detectedMediaType", min_length=1, max_length=255)
    safe_extension: SafeExtension = Field(alias="safeExtension")
    output_root: str = Field(alias="outputRoot", min_length=1, max_length=4096)
    parser_options: ParserOptions = Field(alias="parserOptions")

    @field_validator("document_id", "meeting_id")
    @classmethod
    def validate_entity_id(cls, value: str) -> str:
        try:
            parsed = uuid.UUID(value)
        except ValueError as error:
            raise ValueError("invalid entity id") from error
        if str(parsed) != value:
            raise ValueError("entity id must be canonical")
        return value

    @field_validator("expected_sha256")
    @classmethod
    def validate_sha256(cls, value: str) -> str:
        if _SHA256_PATTERN.fullmatch(value) is None:
            raise ValueError("invalid sha256")
        return value


type WorkerRequest = HealthRequest | CapabilitiesRequest | ParseRequest


def decode_request(raw: bytes) -> WorkerRequest:
    if not raw or len(raw) > MAX_PROTOCOL_MESSAGE_BYTES:
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR) from error
    if not isinstance(payload, dict):
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
    operation = payload.get("operation")
    request_type: type[WorkerRequest]
    if operation == "health":
        request_type = HealthRequest
    elif operation == "capabilities":
        request_type = CapabilitiesRequest
    elif operation == "parse":
        request_type = ParseRequest
    else:
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
    try:
        return request_type.model_validate(payload)
    except ValidationError as error:
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR) from error


def encode_response(payload: dict[str, object]) -> bytes:
    try:
        raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise WorkerError(ParserErrorCode.INTERNAL_ERROR) from error
    if len(raw) + 1 > MAX_PROTOCOL_MESSAGE_BYTES:
        raise WorkerError(ParserErrorCode.PROTOCOL_ERROR)
    return raw + b"\n"


def success_response(request_id: str, result: dict[str, object]) -> dict[str, object]:
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "ok": True,
        "result": result,
    }


def error_response(request_id: str, error: WorkerError) -> dict[str, object]:
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": request_id,
        "ok": False,
        "error": error.response(),
    }
