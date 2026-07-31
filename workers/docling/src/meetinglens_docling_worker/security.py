from __future__ import annotations

import hashlib
import os
import stat
from dataclasses import dataclass
from pathlib import Path

from .constants import MAX_INPUT_BYTES, PARSER_STAGING_ROOT_ENV, ParserErrorCode
from .errors import WorkerError
from .protocol import ParseRequest

_FORBIDDEN_ENV_FRAGMENTS = (
    "API_KEY",
    "AUTHORIZATION",
    "AWS_",
    "AZURE_",
    "CREDENTIAL",
    "DEEPSEEK",
    "GITHUB_TOKEN",
    "GOOGLE_",
    "HF_TOKEN",
    "HUGGING_FACE_HUB_TOKEN",
    "MEETINGLENS_SESSION_TOKEN",
    "PASSWORD",
    "PROXY",
    "SECRET",
    "TOKEN",
)


@dataclass(frozen=True, slots=True)
class ValidatedPaths:
    input_path: Path
    allowed_input_root: Path
    output_root: Path
    parser_staging_root: Path
    input_size: int


def secret_environment_names() -> list[str]:
    return sorted(
        name
        for name in os.environ
        if any(fragment in name.upper() for fragment in _FORBIDDEN_ENV_FRAGMENTS)
    )


def validate_environment() -> None:
    if secret_environment_names():
        raise WorkerError(ParserErrorCode.INVALID_INPUT)


def validate_parse_paths(request: ParseRequest) -> ValidatedPaths:
    for raw in (request.input_path, request.allowed_input_root, request.output_root):
        if _looks_like_url_or_unc(raw) or _contains_alternate_stream(raw):
            raise WorkerError(ParserErrorCode.INVALID_INPUT)

    staging_raw = os.environ.get(PARSER_STAGING_ROOT_ENV)
    if not staging_raw or _looks_like_url_or_unc(staging_raw):
        raise WorkerError(ParserErrorCode.ENVIRONMENT_MISSING)

    try:
        allowed_input_root = Path(request.allowed_input_root).resolve(strict=True)
        input_path = Path(request.input_path).resolve(strict=True)
        parser_staging_root = Path(staging_raw).resolve(strict=True)
        output_root = Path(request.output_root).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise WorkerError(ParserErrorCode.INVALID_INPUT) from error

    if not allowed_input_root.is_dir() or not parser_staging_root.is_dir():
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    if not input_path.is_relative_to(allowed_input_root):
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    if not output_root.is_relative_to(parser_staging_root) or output_root == parser_staging_root:
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    if not output_root.is_dir() or any(output_root.iterdir()):
        raise WorkerError(ParserErrorCode.INVALID_INPUT)

    try:
        input_stat = input_path.stat()
    except OSError as error:
        raise WorkerError(ParserErrorCode.INVALID_INPUT) from error
    if not stat.S_ISREG(input_stat.st_mode) or input_stat.st_size <= 0:
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    if input_stat.st_size > MAX_INPUT_BYTES:
        raise WorkerError(ParserErrorCode.RESOURCE_LIMIT)
    if input_path.suffix.lower() != f".{request.safe_extension}":
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    if hash_file(input_path) != request.expected_sha256:
        raise WorkerError(ParserErrorCode.INTEGRITY_FAILED)

    return ValidatedPaths(
        input_path=input_path,
        allowed_input_root=allowed_input_root,
        output_root=output_root,
        parser_staging_root=parser_staging_root,
        input_size=input_stat.st_size,
    )


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise WorkerError(ParserErrorCode.INTEGRITY_FAILED) from error
    return digest.hexdigest()


def _looks_like_url_or_unc(value: str) -> bool:
    normalized = value.strip().lower()
    return "://" in normalized or normalized.startswith(("\\\\", "//"))


def _contains_alternate_stream(value: str) -> bool:
    path = Path(value)
    drive = path.drive
    remainder = str(path)[len(drive) :]
    return ":" in remainder
