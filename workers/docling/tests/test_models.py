from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from meetinglens_docling_worker.constants import (
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
from meetinglens_docling_worker.errors import WorkerError
from meetinglens_docling_worker.models import models_status, require_models


class ModelManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.model = self.root / "model.bin"
        self.model.write_bytes(b"synthetic model bytes")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_missing_manifest_is_reported_as_missing(self) -> None:
        self.assertEqual(models_status(self.root), (False, False, None))

    def test_compatible_manifest_verifies_file_hashes(self) -> None:
        manifest = self._write_manifest()
        present, compatible, loaded = models_status(self.root)
        self.assertTrue(present)
        self.assertTrue(compatible)
        self.assertEqual(loaded, manifest)

    def test_version_mismatch_is_incompatible(self) -> None:
        manifest = self._manifest()
        manifest["doclingVersion"] = "0.0.0"
        self._write(manifest)
        self.assertEqual(models_status(self.root)[:2], (True, False))

    def test_tampered_model_is_incompatible(self) -> None:
        self._write_manifest()
        self.model.write_bytes(b"tampered")
        self.assertEqual(models_status(self.root)[:2], (True, False))

    def test_absolute_manifest_path_is_rejected(self) -> None:
        manifest = self._manifest()
        manifest["files"][0]["relativePath"] = str(self.model.resolve())
        self._write(manifest)
        self.assertEqual(models_status(self.root)[:2], (True, False))

    def test_require_models_uses_stable_missing_and_incompatible_errors(self) -> None:
        with self.assertRaises(WorkerError) as missing:
            require_models(self.root / "missing")
        self.assertEqual(missing.exception.code, ParserErrorCode.MODELS_MISSING)
        self._write({"schemaVersion": -1})
        with self.assertRaises(WorkerError) as incompatible:
            require_models(self.root)
        self.assertEqual(incompatible.exception.code, ParserErrorCode.MODELS_INCOMPATIBLE)

    def _write_manifest(self) -> dict[str, object]:
        manifest = self._manifest()
        self._write(manifest)
        return manifest

    def _write(self, manifest: dict[str, object]) -> None:
        (self.root / MODELS_MANIFEST_NAME).write_text(
            json.dumps(manifest),
            encoding="utf-8",
        )

    def _manifest(self) -> dict[str, object]:
        data = self.model.read_bytes()
        return {
            "schemaVersion": MODELS_MANIFEST_SCHEMA_VERSION,
            "doclingVersion": DOCLING_VERSION,
            "preparedAt": "2026-01-01T00:00:00Z",
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
            "totalSizeBytes": len(data),
            "files": [
                {
                    "relativePath": self.model.name,
                    "sizeBytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest(),
                }
            ],
        }


if __name__ == "__main__":
    unittest.main()
