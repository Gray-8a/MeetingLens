from __future__ import annotations

import hashlib
import os
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from meetinglens_docling_worker.constants import PARSER_STAGING_ROOT_ENV, ParserErrorCode
from meetinglens_docling_worker.errors import WorkerError
from meetinglens_docling_worker.protocol import ParseRequest
from meetinglens_docling_worker.security import (
    secret_environment_names,
    validate_parse_paths,
)


class SecurityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.inputs = self.root / "inputs"
        self.staging = self.root / "parser-staging"
        self.output = self.staging / str(uuid.uuid4())
        self.inputs.mkdir()
        self.output.mkdir(parents=True)
        self.input_path = self.inputs / "synthetic.txt"
        self.input_path.write_text("Synthetic document.", encoding="utf-8")
        self.environment = patch.dict(
            os.environ,
            {PARSER_STAGING_ROOT_ENV: str(self.staging)},
            clear=True,
        )
        self.environment.start()

    def tearDown(self) -> None:
        self.environment.stop()
        self.temporary.cleanup()

    def test_accepts_regular_file_inside_roots(self) -> None:
        result = validate_parse_paths(self._request())
        self.assertEqual(result.input_path, self.input_path.resolve())

    def test_rejects_input_outside_allowed_root(self) -> None:
        request = self._request(input_path=self.root / "outside.txt")
        Path(request.input_path).write_text("outside", encoding="utf-8")
        request.expected_sha256 = hashlib.sha256(b"outside").hexdigest()
        self._assert_error(request, ParserErrorCode.INVALID_INPUT)

    def test_rejects_symlink_that_resolves_outside_allowed_root(self) -> None:
        outside = self.root / "outside-target.txt"
        outside.write_text("outside", encoding="utf-8")
        link = self.inputs / "linked.txt"
        try:
            link.symlink_to(outside)
        except OSError as error:
            self.skipTest(f"file symlink creation is unavailable: {error.winerror}")
        request = self._request(input_path=link)
        request.expected_sha256 = hashlib.sha256(b"outside").hexdigest()
        self._assert_error(request, ParserErrorCode.INVALID_INPUT)

    def test_rejects_url_input(self) -> None:
        self._assert_error(self._request(input_path="https://example.invalid/file.txt"))

    def test_rejects_unc_input(self) -> None:
        self._assert_error(self._request(input_path="\\\\server\\share\\file.txt"))

    def test_rejects_alternate_data_stream(self) -> None:
        self._assert_error(self._request(input_path=f"{self.input_path}:stream"))

    def test_rejects_directory_input(self) -> None:
        request = self._request(input_path=self.inputs)
        self._assert_error(request)

    def test_rejects_hash_mismatch(self) -> None:
        request = self._request()
        request.expected_sha256 = "0" * 64
        self._assert_error(request, ParserErrorCode.INTEGRITY_FAILED)

    def test_rejects_output_outside_staging_root(self) -> None:
        outside = self.root / "outside-output"
        outside.mkdir()
        self._assert_error(self._request(output_root=outside))

    def test_rejects_nonempty_output_root(self) -> None:
        (self.output / "existing.txt").write_text("existing", encoding="utf-8")
        self._assert_error(self._request())

    def test_rejects_oversized_input(self) -> None:
        with patch("meetinglens_docling_worker.security.MAX_INPUT_BYTES", 4):
            self._assert_error(self._request(), ParserErrorCode.RESOURCE_LIMIT)

    def test_detects_secret_environment_names_without_values(self) -> None:
        with patch.dict(os.environ, {"SYNTHETIC_API_KEY": "placeholder"}, clear=True):
            self.assertEqual(secret_environment_names(), ["SYNTHETIC_API_KEY"])

    def _request(
        self,
        *,
        input_path: str | Path | None = None,
        output_root: str | Path | None = None,
    ) -> ParseRequest:
        return ParseRequest.model_validate(
            {
                "protocolVersion": "2",
                "requestId": str(uuid.uuid4()),
                "operation": "parse",
                "documentId": str(uuid.uuid4()),
                "meetingId": str(uuid.uuid4()),
                "inputPath": str(input_path or self.input_path),
                "allowedInputRoot": str(self.inputs),
                "expectedSha256": hashlib.sha256(self.input_path.read_bytes()).hexdigest(),
                "detectedMediaType": "text/plain",
                "safeExtension": "txt",
                "outputRoot": str(output_root or self.output),
                "parserOptions": {
                    "ocr": False,
                    "maxPages": 300,
                    "documentTimeoutSeconds": 120.0,
                },
            }
        )

    def _assert_error(
        self,
        request: ParseRequest,
        code: ParserErrorCode = ParserErrorCode.INVALID_INPUT,
    ) -> None:
        with self.assertRaises(WorkerError) as context:
            validate_parse_paths(request)
        self.assertEqual(context.exception.code, code)


if __name__ == "__main__":
    unittest.main()
