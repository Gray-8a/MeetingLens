from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from meetinglens_docling_worker.constants import (
    MODELS_ROOT_ENV,
    PARSER_STAGING_ROOT_ENV,
    ParserErrorCode,
)
from meetinglens_docling_worker.docling_adapter import capabilities, health, parse
from meetinglens_docling_worker.errors import WorkerError
from meetinglens_docling_worker.protocol import ParseRequest


class OfflineIntegrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.inputs = self.root / "inputs"
        self.staging = self.root / "parser-staging"
        self.models = self.root / "models"
        self.inputs.mkdir()
        self.staging.mkdir()
        self.models.mkdir()
        self.home = self.root / "home"
        self.home.mkdir()
        self.environment = patch.dict(
            os.environ,
            {
                PARSER_STAGING_ROOT_ENV: str(self.staging),
                MODELS_ROOT_ENV: str(self.models),
                "HF_HUB_OFFLINE": "1",
                "TRANSFORMERS_OFFLINE": "1",
                "HF_DATASETS_OFFLINE": "1",
                "HOME": str(self.home),
                "USERPROFILE": str(self.home),
                "LOCALAPPDATA": str(self.home / "local"),
            },
            clear=True,
        )
        self.environment.start()

    def tearDown(self) -> None:
        self.environment.stop()
        self.temporary.cleanup()

    def test_health_imports_locked_docling(self) -> None:
        result = health()
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["parserVersion"], "2.117.0")
        self.assertFalse(result["networkListener"])

    def test_capabilities_distinguish_available_and_model_formats(self) -> None:
        rows = {row["format"]: row for row in capabilities()["formats"]}
        self.assertTrue(rows["docx"]["available"])
        self.assertFalse(rows["pdf"]["available"])
        self.assertFalse(rows["png"]["ocrAvailable"])

    def test_parses_utf8_text_into_official_docling_schema(self) -> None:
        result, output = self._parse_file("synthetic.txt", "Synthetic UTF-8 文本。")
        self.assertEqual(result["manifest"]["status"], "success")
        document = json.loads((output / "document.json").read_text(encoding="utf-8"))
        self.assertIn("texts", document)
        self.assertIn("Synthetic UTF-8", (output / "content.txt").read_text(encoding="utf-8"))

    def test_parses_markdown_offline(self) -> None:
        _, output = self._parse_file("synthetic.md", "# Synthetic\n\nOffline content.")
        self.assertIn("Synthetic", (output / "content.md").read_text(encoding="utf-8"))

    def test_parses_csv_offline(self) -> None:
        _, output = self._parse_file("synthetic.csv", "item,value\nalpha,1\nbeta,2\n")
        self.assertIn("alpha", (output / "content.txt").read_text(encoding="utf-8"))

    def test_parses_programmatic_docx_offline(self) -> None:
        from docx import Document

        source = self.inputs / "synthetic.docx"
        document = Document()
        document.add_heading("Synthetic heading", 1)
        document.add_paragraph("Programmatic offline DOCX content.")
        document.save(source)
        _, output = self._parse_existing(source)
        text = (output / "content.txt").read_text(encoding="utf-8")
        self.assertIn("Programmatic offline DOCX", text)

    def test_pdf_fails_closed_without_models(self) -> None:
        source = self.inputs / "synthetic.pdf"
        source.write_bytes(b"%PDF-1.4\n%%EOF\n")
        with self.assertRaises(WorkerError) as context:
            self._parse_existing(source)
        self.assertEqual(context.exception.code, ParserErrorCode.MODELS_MISSING)

    def test_output_contains_no_absolute_input_or_output_path(self) -> None:
        _, output = self._parse_file("private-name.txt", "Synthetic path test.")
        all_output = "\n".join(
            path.read_text(encoding="utf-8") for path in output.iterdir() if path.is_file()
        )
        self.assertNotIn(str(self.root), all_output)
        self.assertNotIn("private-name.txt", all_output)

    def _parse_file(self, name: str, content: str) -> tuple[dict[str, object], Path]:
        source = self.inputs / name
        source.write_text(content, encoding="utf-8")
        return self._parse_existing(source)

    def _parse_existing(self, source: Path) -> tuple[dict[str, object], Path]:
        output = self.staging / str(uuid.uuid4())
        output.mkdir()
        request = ParseRequest.model_validate(
            {
                "protocolVersion": "1",
                "requestId": str(uuid.uuid4()),
                "operation": "parse",
                "documentId": str(uuid.uuid4()),
                "meetingId": str(uuid.uuid4()),
                "inputPath": str(source),
                "allowedInputRoot": str(self.inputs),
                "expectedSha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "detectedMediaType": _media_type(source.suffix),
                "safeExtension": source.suffix.removeprefix("."),
                "outputRoot": str(output),
                "parserOptions": {
                    "ocr": False,
                    "maxPages": 300,
                    "documentTimeoutSeconds": 120.0,
                },
            }
        )
        return parse(request), output


def _media_type(suffix: str) -> str:
    return {
        ".txt": "text/plain",
        ".md": "text/markdown",
        ".csv": "text/csv",
        ".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ".pdf": "application/pdf",
    }[suffix]


if __name__ == "__main__":
    unittest.main()
