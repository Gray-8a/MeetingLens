from __future__ import annotations

import hashlib
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from meetinglens_docling_worker import artifacts
from meetinglens_docling_worker.artifacts import ArtifactPayloads, write_artifacts
from meetinglens_docling_worker.constants import ParserErrorCode
from meetinglens_docling_worker.errors import WorkerError


class ArtifactTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.output = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_writes_fixed_artifact_names_and_manifest(self) -> None:
        result = self._write()
        self.assertEqual(
            sorted(path.name for path in self.output.iterdir()),
            ["content.md", "content.txt", "document.json", "manifest.json"],
        )
        self.assertEqual(result["manifest"]["status"], "success")

    def test_records_size_and_hash_for_each_content_artifact(self) -> None:
        manifest = self._write()["manifest"]
        for artifact in manifest["artifacts"]:
            data = (self.output / artifact["relativePath"]).read_bytes()
            self.assertEqual(artifact["sizeBytes"], len(data))
            self.assertEqual(artifact["sha256"], hashlib.sha256(data).hexdigest())

    def test_write_failure_removes_only_fixed_partial_artifacts(self) -> None:
        unknown = self.output / "unknown.keep"
        unknown.write_text("keep", encoding="utf-8")
        real_write = artifacts._write_atomic
        calls = 0

        def fail_second_write(output_root: Path, relative_path: str, data: bytes) -> None:
            nonlocal calls
            calls += 1
            if calls == 2:
                raise WorkerError(ParserErrorCode.FAILED)
            real_write(output_root, relative_path, data)

        with patch.object(artifacts, "_write_atomic", side_effect=fail_second_write):
            with self.assertRaises(WorkerError):
                self._write()

        self.assertEqual([path.name for path in self.output.iterdir()], ["unknown.keep"])

    def test_manifest_contains_no_absolute_paths(self) -> None:
        manifest_text = (
            (self.output / "manifest.json").read_text(encoding="utf-8")
            if (self.output / "manifest.json").exists()
            else ""
        )
        if not manifest_text:
            self._write()
            manifest_text = (self.output / "manifest.json").read_text(encoding="utf-8")
        self.assertNotIn(str(self.output), manifest_text)

    def test_rejects_document_json_over_limit(self) -> None:
        with patch("meetinglens_docling_worker.artifacts.MAX_DOCUMENT_JSON_BYTES", 8):
            self._assert_output_too_large()

    def test_rejects_total_output_over_limit(self) -> None:
        with patch("meetinglens_docling_worker.artifacts.MAX_TOTAL_ARTIFACT_BYTES", 8):
            self._assert_output_too_large()

    def test_rejects_non_json_document_payload(self) -> None:
        payloads = ArtifactPayloads(
            document={"invalid": {"set"}},
            markdown="synthetic",
            text="synthetic",
            page_count=1,
            has_ocr=False,
            warnings=[],
        )
        with self.assertRaises(WorkerError) as context:
            self._write(payloads)
        self.assertEqual(context.exception.code, ParserErrorCode.FAILED)

    def _write(self, payloads: ArtifactPayloads | None = None) -> dict[str, object]:
        return write_artifacts(
            output_root=self.output,
            request_id=str(uuid.uuid4()),
            document_id=str(uuid.uuid4()),
            input_sha256="a" * 64,
            detected_media_type="text/plain",
            duration_ms=1,
            payloads=payloads
            or ArtifactPayloads(
                document={"name": "synthetic"},
                markdown="# Synthetic",
                text="Synthetic",
                page_count=1,
                has_ocr=False,
                warnings=[],
            ),
        )

    def _assert_output_too_large(self) -> None:
        with self.assertRaises(WorkerError) as context:
            self._write()
        self.assertEqual(context.exception.code, ParserErrorCode.OUTPUT_TOO_LARGE)


if __name__ == "__main__":
    unittest.main()
