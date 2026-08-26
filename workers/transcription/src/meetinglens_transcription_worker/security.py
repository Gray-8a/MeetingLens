from __future__ import annotations

import hashlib
import os
import stat
from dataclasses import dataclass
from pathlib import Path

from .constants import MAX_INPUT_BYTES, STAGING_ROOT_ENV, TranscriptionErrorCode
from .errors import WorkerError
from .protocol import TranscribeRequest

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
_REPARSE_POINT = getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)


@dataclass(frozen=True, slots=True)
class ValidatedPaths:
    input_path: Path
    allowed_input_root: Path
    output_root: Path
    staging_root: Path
    input_size: int


def secret_environment_names() -> list[str]:
    return sorted(
        name
        for name in os.environ
        if any(fragment in name.upper() for fragment in _FORBIDDEN_ENV_FRAGMENTS)
    )


def validate_environment() -> None:
    if secret_environment_names():
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)


def validate_transcribe_paths(request: TranscribeRequest) -> ValidatedPaths:
    staging_raw = os.environ.get(STAGING_ROOT_ENV)
    if not staging_raw:
        raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE)

    raw_paths = (
        request.input_path,
        request.allowed_input_root,
        request.output_root,
        staging_raw,
    )
    if any(not _is_safe_absolute_path(value) for value in raw_paths):
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)

    try:
        allowed_input_root = _validated_root(Path(request.allowed_input_root))
        staging_root = _validated_root(Path(staging_raw))
        input_path = _validated_descendant(
            allowed_input_root,
            Path(request.input_path),
            allow_root=False,
        )
        output_root = _validated_descendant(
            staging_root,
            Path(request.output_root),
            allow_root=False,
        )
    except WorkerError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT) from error

    if output_root.name != request.request_id:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
    try:
        if not output_root.is_dir() or any(output_root.iterdir()):
            raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
        input_stat = input_path.stat()
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT) from error

    if not stat.S_ISREG(input_stat.st_mode) or input_stat.st_size <= 0:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
    if input_stat.st_size > MAX_INPUT_BYTES:
        raise WorkerError(TranscriptionErrorCode.TOO_LARGE)
    if input_stat.st_size != request.expected_size_bytes:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED)
    if input_path.suffix.lower() != f".{request.safe_extension}":
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)

    before = _file_identity(input_stat)
    if hash_file(input_path) != request.expected_sha256:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED)
    try:
        after = _file_identity(input_path.stat())
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED) from error
    if before != after:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED)

    return ValidatedPaths(
        input_path=input_path,
        allowed_input_root=allowed_input_root,
        output_root=output_root,
        staging_root=staging_root,
        input_size=input_stat.st_size,
    )


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.INTEGRITY_FAILED) from error
    return digest.hexdigest()


def validate_existing_directory(path: Path, error_code: TranscriptionErrorCode) -> Path:
    try:
        raw = str(path)
        if not _is_safe_absolute_path(raw):
            raise WorkerError(error_code)
        resolved = _validated_root(path)
        if not resolved.is_dir():
            raise WorkerError(error_code)
        return resolved
    except WorkerError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise WorkerError(error_code) from error


def _validated_root(path: Path) -> Path:
    lexical = Path(os.path.abspath(path))
    _reject_reparse(lexical)
    resolved = lexical.resolve(strict=True)
    if not resolved.is_dir():
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
    return resolved


def _validated_descendant(root: Path, path: Path, *, allow_root: bool) -> Path:
    lexical_root = Path(os.path.abspath(root))
    lexical_path = Path(os.path.abspath(path))
    try:
        relative = lexical_path.relative_to(lexical_root)
    except ValueError as error:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT) from error
    current = lexical_root
    _reject_reparse(current)
    for part in relative.parts:
        current /= part
        _reject_reparse(current)
    resolved = lexical_path.resolve(strict=True)
    if not resolved.is_relative_to(root) or (resolved == root and not allow_root):
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)
    return resolved


def _reject_reparse(path: Path) -> None:
    try:
        details = path.lstat()
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT) from error
    attributes = getattr(details, "st_file_attributes", 0)
    if stat.S_ISLNK(details.st_mode) or attributes & _REPARSE_POINT:
        raise WorkerError(TranscriptionErrorCode.INVALID_INPUT)


def _is_safe_absolute_path(value: str) -> bool:
    normalized = value.strip().lower()
    if not normalized or "://" in normalized or normalized.startswith("//"):
        return False
    if normalized.startswith("\\\\") and not _is_verbatim_local_drive(normalized):
        return False
    path = Path(value)
    if not path.is_absolute():
        return False
    drive = path.drive
    remainder = str(path)[len(drive) :]
    return ":" not in remainder


def _is_verbatim_local_drive(value: str) -> bool:
    return (
        len(value) >= 7
        and value.startswith("\\\\?\\")
        and value[4].isalpha()
        and value[5:7] in {":\\", ":/"}
    )


def _file_identity(details: os.stat_result) -> tuple[int, int, int, int]:
    return (details.st_dev, details.st_ino, details.st_size, details.st_mtime_ns)
