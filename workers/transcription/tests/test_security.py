from __future__ import annotations

import hashlib
import os
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from meetinglens_transcription_worker.constants import STAGING_ROOT_ENV, TranscriptionErrorCode
from meetinglens_transcription_worker.errors import WorkerError
from meetinglens_transcription_worker.protocol import TranscribeRequest
from meetinglens_transcription_worker.security import (
    _is_safe_absolute_path,
    secret_environment_names,
    validate_transcribe_paths,
)


class SecurityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.inputs = self.root / "inputs"
        self.staging = self.root / "staging"
        self.request_id = str(uuid.uuid4())
        self.output = self.staging / self.request_id
        self.inputs.mkdir()
        self.output.mkdir(parents=True)
        self.input_path = self.inputs / "synthetic.mp4"
        self.input_path.write_bytes(b"synthetic media container")
        self.environment = patch.dict(
            os.environ,
            {STAGING_ROOT_ENV: str(self.staging)},
            clear=True,
        )
        self.environment.start()

    def tearDown(self) -> None:
        self.environment.stop()
        self.temporary.cleanup()

    def test_accepts_hashed_regular_file_in_private_roots(self) -> None:
        paths = validate_transcribe_paths(self._request())
        self.assertEqual(paths.input_path, self.input_path.resolve())

    def test_rejects_wrong_size_or_hash(self) -> None:
        for changes in ({"expectedSizeBytes": 1}, {"expectedSha256": "0" * 64}):
            with self.subTest(changes=changes):
                self._assert_error(
                    self._request(**changes), TranscriptionErrorCode.INTEGRITY_FAILED
                )

    def test_rejects_outside_and_nonempty_output(self) -> None:
        outside = self.root / "outside.mp4"
        outside.write_bytes(self.input_path.read_bytes())
        self._assert_error(self._request(inputPath=str(outside)))
        (self.output / "existing").write_text("synthetic", encoding="utf-8")
        self._assert_error(self._request())

    def test_rejects_output_not_named_for_request(self) -> None:
        other = self.staging / str(uuid.uuid4())
        other.mkdir()
        self._assert_error(self._request(outputRoot=str(other)))

    def test_rejects_url_unc_ads_and_relative_paths(self) -> None:
        for value in (
            "https://example.invalid/source.mp4",
            "\\\\server\\share\\source.mp4",
            f"{self.input_path}:stream",
            "relative/source.mp4",
        ):
            with self.subTest(value=value):
                self._assert_error(self._request(inputPath=value))

    @unittest.skipUnless(os.name == "nt", "Windows verbatim path contract")
    def test_allows_only_local_drive_verbatim_paths(self) -> None:
        self.assertTrue(_is_safe_absolute_path(r"\\?\C:\synthetic\source.mp4"))
        self.assertFalse(_is_safe_absolute_path(r"\\?\UNC\server\share\source.mp4"))
        self.assertFalse(_is_safe_absolute_path(r"\\.\C:\synthetic\source.mp4"))

    def test_rejects_symlink_even_when_target_is_inside_root(self) -> None:
        link = self.inputs / "linked.mp4"
        try:
            link.symlink_to(self.input_path)
        except OSError as error:
            self.skipTest(f"file symlink creation is unavailable: {error}")
        self._assert_error(self._request(inputPath=str(link)))

    def test_secret_name_detection_never_returns_values(self) -> None:
        with patch.dict(os.environ, {"SYNTHETIC_API_KEY": "do-not-return"}, clear=True):
            self.assertEqual(secret_environment_names(), ["SYNTHETIC_API_KEY"])

    def _request(self, **changes: object) -> TranscribeRequest:
        data: dict[str, object] = {
            "protocolVersion": "1",
            "requestId": self.request_id,
            "operation": "transcribe",
            "inputPath": str(self.input_path),
            "allowedInputRoot": str(self.inputs),
            "outputRoot": str(self.output),
            "expectedSha256": hashlib.sha256(self.input_path.read_bytes()).hexdigest(),
            "expectedSizeBytes": self.input_path.stat().st_size,
            "detectedMediaType": "audio/mp4",
            "safeExtension": "mp4",
        }
        data.update(changes)
        return TranscribeRequest.model_validate(data)

    def _assert_error(
        self,
        request: TranscribeRequest,
        code: TranscriptionErrorCode = TranscriptionErrorCode.INVALID_INPUT,
    ) -> None:
        with self.assertRaises(WorkerError) as context:
            validate_transcribe_paths(request)
        self.assertEqual(context.exception.code, code)


if __name__ == "__main__":
    unittest.main()
