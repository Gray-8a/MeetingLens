from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import os
import shutil
import stat
import sys
import tempfile
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path

from .constants import (
    DIRECT_DEPENDENCY_VERSIONS,
    FASTER_WHISPER_VERSION,
    MODEL_MANIFEST_NAME,
    MODEL_MANIFEST_SCHEMA_VERSION,
    MODEL_REPOSITORY,
    MODEL_REQUIRED_FILES,
    MODEL_REVISION,
    TRANSCRIBE_OPTIONS,
    TranscriptionErrorCode,
)
from .errors import WorkerError

_MAX_MANIFEST_BYTES = 1024 * 1024
_REPARSE_POINT = getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)
type Downloader = Callable[..., str]


def models_status(models_root: Path) -> tuple[bool, bool, dict[str, object] | None]:
    manifest_path = models_root / MODEL_MANIFEST_NAME
    if not models_root.is_dir() or not manifest_path.is_file():
        return False, False, None
    try:
        if _is_redirected(manifest_path):
            return True, False, None
        if manifest_path.stat().st_size > _MAX_MANIFEST_BYTES:
            return True, False, None
        manifest = json.loads(manifest_path.read_text(encoding="utf-8", errors="strict"))
        if not isinstance(manifest, dict) or not _valid_manifest_header(manifest):
            return True, False, None
        actual = _inventory(models_root)
        expected = manifest.get("files")
        if actual != expected:
            return True, False, None
        return True, True, manifest
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, WorkerError):
        return True, False, None


def require_models(models_root: Path) -> dict[str, object]:
    present, compatible, manifest = models_status(models_root)
    if not present:
        raise WorkerError(TranscriptionErrorCode.MODEL_MISSING)
    if not compatible or manifest is None:
        raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
    return manifest


def provision_models(
    models_root: Path,
    *,
    downloader: Downloader | None = None,
) -> dict[str, object]:
    _verify_direct_dependencies()
    if models_root.exists():
        return require_models(models_root)
    try:
        models_root.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            prefix=f".{models_root.name}.provision-",
            dir=models_root.parent,
        ) as temporary_name:
            temporary = Path(temporary_name)
            download_cache = temporary / "download-cache"
            download_cache.mkdir()
            prepared = temporary / "prepared"
            model_dir = prepared / "model"
            model_dir.mkdir(parents=True)
            if downloader is None:
                from huggingface_hub import snapshot_download

                downloader = snapshot_download
            cache_root = download_cache.resolve(strict=True)
            snapshot = Path(
                downloader(
                    repo_id=MODEL_REPOSITORY,
                    revision=MODEL_REVISION,
                    allow_patterns=list(MODEL_REQUIRED_FILES),
                    cache_dir=download_cache,
                    token=False,
                )
            ).resolve(strict=True)
            if not snapshot.is_relative_to(cache_root):
                raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
            for name in MODEL_REQUIRED_FILES:
                source = (snapshot / name).resolve(strict=True)
                if not source.is_relative_to(cache_root) or not source.is_file():
                    raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
                shutil.copyfile(source, model_dir / name)

            files = _inventory(prepared)
            manifest: dict[str, object] = {
                "schemaVersion": MODEL_MANIFEST_SCHEMA_VERSION,
                "repository": MODEL_REPOSITORY,
                "revision": MODEL_REVISION,
                "fasterWhisperVersion": FASTER_WHISPER_VERSION,
                "device": "cpu",
                "computeType": "int8",
                "options": TRANSCRIBE_OPTIONS,
                "preparedAt": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
                "totalSizeBytes": sum(int(item["sizeBytes"]) for item in files),
                "files": files,
            }
            (prepared / MODEL_MANIFEST_NAME).write_text(
                json.dumps(manifest, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
                + "\n",
                encoding="utf-8",
            )
            prepared.rename(models_root)
    except WorkerError:
        raise
    except (OSError, RuntimeError, ValueError) as error:
        raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE) from error
    return require_models(models_root)


def model_directory(models_root: Path) -> Path:
    require_models(models_root)
    return models_root / "model"


def _valid_manifest_header(manifest: dict[str, object]) -> bool:
    expected_keys = {
        "schemaVersion",
        "repository",
        "revision",
        "fasterWhisperVersion",
        "device",
        "computeType",
        "options",
        "preparedAt",
        "totalSizeBytes",
        "files",
    }
    if set(manifest) != expected_keys:
        return False
    if (
        type(manifest.get("schemaVersion")) is not int
        or manifest.get("schemaVersion") != MODEL_MANIFEST_SCHEMA_VERSION
        or manifest.get("repository") != MODEL_REPOSITORY
        or manifest.get("revision") != MODEL_REVISION
        or manifest.get("fasterWhisperVersion") != FASTER_WHISPER_VERSION
        or manifest.get("device") != "cpu"
        or manifest.get("computeType") != "int8"
        or manifest.get("options") != TRANSCRIBE_OPTIONS
        or not isinstance(manifest.get("preparedAt"), str)
        or not isinstance(manifest.get("totalSizeBytes"), int)
        or isinstance(manifest.get("totalSizeBytes"), bool)
    ):
        return False
    files = manifest.get("files")
    if not isinstance(files, list) or len(files) != len(MODEL_REQUIRED_FILES):
        return False
    required_paths = {f"model/{name}" for name in MODEL_REQUIRED_FILES}
    found_paths: set[str] = set()
    total = 0
    for entry in files:
        if not isinstance(entry, dict) or set(entry) != {"relativePath", "sizeBytes", "sha256"}:
            return False
        relative = entry.get("relativePath")
        size = entry.get("sizeBytes")
        sha256 = entry.get("sha256")
        if (
            not isinstance(relative, str)
            or relative not in required_paths
            or relative in found_paths
            or not isinstance(size, int)
            or isinstance(size, bool)
            or size <= 0
            or not isinstance(sha256, str)
            or len(sha256) != 64
            or any(character not in "0123456789abcdef" for character in sha256)
        ):
            return False
        found_paths.add(relative)
        total += size
    return found_paths == required_paths and manifest.get("totalSizeBytes") == total


def _inventory(models_root: Path) -> list[dict[str, object]]:
    files: list[dict[str, object]] = []
    for path in sorted(models_root.rglob("*")):
        if _is_redirected(path):
            raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
        if not path.is_file() or path.name == MODEL_MANIFEST_NAME:
            continue
        relative = path.relative_to(models_root).as_posix()
        if relative not in {f"model/{name}" for name in MODEL_REQUIRED_FILES}:
            raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
        details = path.stat()
        if details.st_size <= 0:
            raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE)
        files.append(
            {
                "relativePath": relative,
                "sizeBytes": details.st_size,
                "sha256": _sha256(path),
            }
        )
    if len(files) != len(MODEL_REQUIRED_FILES):
        raise WorkerError(TranscriptionErrorCode.MODEL_MISSING)
    return files


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE) from error
    return digest.hexdigest()


def _is_redirected(path: Path) -> bool:
    try:
        details = path.lstat()
    except OSError as error:
        raise WorkerError(TranscriptionErrorCode.MODEL_INCOMPATIBLE) from error
    attributes = getattr(details, "st_file_attributes", 0)
    return stat.S_ISLNK(details.st_mode) or bool(attributes & _REPARSE_POINT)


def _verify_direct_dependencies() -> None:
    for distribution, expected in DIRECT_DEPENDENCY_VERSIONS.items():
        try:
            version = importlib.metadata.version(distribution)
        except importlib.metadata.PackageNotFoundError as error:
            raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE) from error
        if version != expected:
            raise WorkerError(TranscriptionErrorCode.WORKER_UNAVAILABLE)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Explicitly provision the pinned MeetingLens transcription model"
    )
    parser.add_argument("--models-root", type=Path, required=True)
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args(argv)
    os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
    try:
        manifest = (
            require_models(args.models_root)
            if args.verify_only
            else provision_models(args.models_root)
        )
        action = "verified" if args.verify_only else "prepared"
        print(
            f"Transcription model artifacts {action}: "
            f"{manifest['totalSizeBytes']} bytes, {len(manifest['files'])} files."
        )
    except WorkerError as error:
        print(error.safe_message, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
