from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from datetime import UTC, datetime
from pathlib import Path

from .constants import (
    DOCLING_VERSION,
    LAYOUT_REPOSITORY,
    LAYOUT_REVISION,
    MODELS_MANIFEST_NAME,
    MODELS_MANIFEST_SCHEMA_VERSION,
    RAPIDOCR_UPSTREAM_RELEASE,
    TABLE_REPOSITORY,
    TABLE_REVISION,
    TABLE_UPSTREAM_TAG,
    ParserErrorCode,
)
from .errors import WorkerError


def models_status(models_root: Path) -> tuple[bool, bool, dict[str, object] | None]:
    manifest_path = models_root / MODELS_MANIFEST_NAME
    if not manifest_path.is_file():
        return False, False, None
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return True, False, None
    if not _manifest_metadata_is_compatible(manifest):
        return True, False, manifest if isinstance(manifest, dict) else None
    files = manifest.get("files")
    if not isinstance(files, list) or not files:
        return True, False, manifest
    for entry in files:
        if not isinstance(entry, dict):
            return True, False, manifest
        relative_path = entry.get("relativePath")
        expected_size = entry.get("sizeBytes")
        expected_sha = entry.get("sha256")
        if (
            not isinstance(relative_path, str)
            or not isinstance(expected_size, int)
            or not isinstance(expected_sha, str)
            or Path(relative_path).is_absolute()
            or ".." in Path(relative_path).parts
            or ":" in relative_path
        ):
            return True, False, manifest
        candidate = models_root / relative_path
        try:
            resolved = candidate.resolve(strict=True)
            root = models_root.resolve(strict=True)
            if not resolved.is_relative_to(root) or not resolved.is_file():
                return True, False, manifest
            if resolved.stat().st_size != expected_size or _sha256(resolved) != expected_sha:
                return True, False, manifest
        except OSError:
            return True, False, manifest
    return True, True, manifest


def require_models(models_root: Path) -> dict[str, object]:
    present, compatible, manifest = models_status(models_root)
    if not present:
        raise WorkerError(ParserErrorCode.MODELS_MISSING)
    if not compatible or manifest is None:
        raise WorkerError(ParserErrorCode.MODELS_INCOMPATIBLE)
    return manifest


def provision_models(models_root: Path) -> dict[str, object]:
    from docling.datamodel.pipeline_options import LayoutOptions
    from docling.models.stages.ocr.rapid_ocr_model import RapidOcrModel
    from huggingface_hub import snapshot_download

    models_root.mkdir(parents=True, exist_ok=True)
    layout_folder = LayoutOptions().model_spec.model_repo_folder
    snapshot_download(
        repo_id=LAYOUT_REPOSITORY,
        revision=LAYOUT_REVISION,
        local_dir=models_root / layout_folder,
    )
    snapshot_download(
        repo_id=TABLE_REPOSITORY,
        revision=TABLE_REVISION,
        local_dir=models_root / "docling-project--docling-models",
        allow_patterns=["model_artifacts/tableformer/**"],
    )
    RapidOcrModel.download_models(
        backend="onnxruntime",
        local_dir=models_root / "RapidOcr",
        force=False,
        progress=True,
        lang="chinese",
    )
    files = _inventory(models_root)
    manifest: dict[str, object] = {
        "schemaVersion": MODELS_MANIFEST_SCHEMA_VERSION,
        "doclingVersion": DOCLING_VERSION,
        "preparedAt": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "sources": [
            {
                "name": "layout",
                "repository": LAYOUT_REPOSITORY,
                "revision": LAYOUT_REVISION,
            },
            {
                "name": "tableformer",
                "repository": TABLE_REPOSITORY,
                "tag": TABLE_UPSTREAM_TAG,
                "revision": TABLE_REVISION,
            },
            {
                "name": "rapidocr-onnx",
                "repository": "RapidAI/RapidOCR",
                "release": RAPIDOCR_UPSTREAM_RELEASE,
            },
        ],
        "ocrBackend": "rapidocr-onnxruntime",
        "totalSizeBytes": sum(int(item["sizeBytes"]) for item in files),
        "files": files,
    }
    manifest_path = models_root / MODELS_MANIFEST_NAME
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return manifest


def _manifest_metadata_is_compatible(manifest: object) -> bool:
    if not isinstance(manifest, dict):
        return False
    if (
        manifest.get("schemaVersion") != MODELS_MANIFEST_SCHEMA_VERSION
        or manifest.get("doclingVersion") != DOCLING_VERSION
        or manifest.get("ocrBackend") != "rapidocr-onnxruntime"
    ):
        return False
    sources = manifest.get("sources")
    return sources == [
        {"name": "layout", "repository": LAYOUT_REPOSITORY, "revision": LAYOUT_REVISION},
        {
            "name": "tableformer",
            "repository": TABLE_REPOSITORY,
            "tag": TABLE_UPSTREAM_TAG,
            "revision": TABLE_REVISION,
        },
        {
            "name": "rapidocr-onnx",
            "repository": "RapidAI/RapidOCR",
            "release": RAPIDOCR_UPSTREAM_RELEASE,
        },
    ]


def _inventory(models_root: Path) -> list[dict[str, object]]:
    files: list[dict[str, object]] = []
    for path in sorted(models_root.rglob("*")):
        if not path.is_file() or path.name == MODELS_MANIFEST_NAME or ".cache" in path.parts:
            continue
        relative = path.relative_to(models_root).as_posix()
        files.append(
            {
                "relativePath": relative,
                "sizeBytes": path.stat().st_size,
                "sha256": _sha256(path),
            }
        )
    if not files:
        raise WorkerError(ParserErrorCode.MODELS_MISSING)
    return files


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Explicitly provision MeetingLens Docling models")
    parser.add_argument("--models-root", type=Path, required=True)
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args(argv)
    os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
    try:
        if args.verify_only:
            require_models(args.models_root)
            print("Docling model artifacts are compatible.")
        else:
            manifest = provision_models(args.models_root)
            print(
                "Docling model artifacts prepared: "
                f"{manifest['totalSizeBytes']} bytes, {len(manifest['files'])} files."
            )
    except WorkerError as error:
        print(error.safe_message, file=sys.stderr)
        return 2
    return 0
