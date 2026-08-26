from __future__ import annotations

import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from helpers import write_synthetic_model

from meetinglens_transcription_worker.adapter import TranscriptPayload, TranscriptSegment
from meetinglens_transcription_worker.constants import (
    MODELS_ROOT_ENV,
    SAFE_ERROR_DETAILS,
    STAGING_ROOT_ENV,
    TranscriptionErrorCode,
)
from meetinglens_transcription_worker.worker import BoundedDiscardSink, run_once


class FakeEngine:
    def __init__(self, _model_directory: Path) -> None:
        pass

    def transcribe(self, _input_path: Path, progress: object) -> TranscriptPayload:
        progress(1_000, 2_000)
        progress(2_000, 2_000)
        return TranscriptPayload(
            language="en",
            language_probability=0.9,
            duration_ms=2_000,
            segments=(
                TranscriptSegment(0, 1_000, "Synthetic private phrase."),
                TranscriptSegment(1_000, 2_000, "Synthetic public outcome."),
            ),
        )


class WorkerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.inputs = self.root / "inputs"
        self.staging = self.root / "staging"
        self.models = self.root / "models"
        self.request_id = str(uuid.uuid4())
        self.output = self.staging / self.request_id
        self.inputs.mkdir()
        self.output.mkdir(parents=True)
        write_synthetic_model(self.models)
        self.input_path = self.inputs / "synthetic.wav"
        self.input_path.write_bytes(b"synthetic test media")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_fake_engine_end_to_end_keeps_transcript_out_of_stdout(self) -> None:
        raw = self._execute(self._request(), engine_factory=FakeEngine)
        lines = [json.loads(line) for line in raw.splitlines()]
        self.assertGreaterEqual(len(lines), 5)
        self.assertTrue(lines[-1]["ok"])
        self.assertEqual(lines[-1]["type"], "result")
        self.assertEqual(lines[-1]["result"]["segmentCount"], 2)
        self.assertTrue(all(line["requestId"] == self.request_id for line in lines))
        self.assertNotIn(b"Synthetic private phrase", raw)
        self.assertNotIn(str(self.input_path).encode(), raw)
        self.assertEqual(
            [line["sequence"] for line in lines if line["type"] == "progress"],
            list(range(1, len(lines))),
        )

    def test_rejects_second_request_and_secret_environment(self) -> None:
        request = json.dumps(self._request()).encode() + b"\n"
        response = self._execute_raw(request + request)
        self.assertEqual(json.loads(response)["error"]["code"], "audio_protocol_error")
        response = self._execute_raw(
            request,
            extra_environment={"SYNTHETIC_API_KEY": "placeholder"},
        )
        self.assertEqual(json.loads(response)["error"]["code"], "audio_invalid_input")

    def test_invalid_json_is_one_safe_terminal_line(self) -> None:
        response = self._execute_raw(b"{\n")
        parsed = json.loads(response)
        self.assertEqual(response.count(b"\n"), 1)
        self.assertEqual(parsed["requestId"], "00000000-0000-0000-0000-000000000000")
        self.assertNotIn("traceback", response.decode().lower())

    def test_bounded_sink_never_retains_content(self) -> None:
        sink = BoundedDiscardSink(limit=4)
        self.assertEqual(sink.write("private-value"), len("private-value"))
        self.assertEqual(sink.observed, 5)
        self.assertFalse(hasattr(sink, "value"))

    def test_remote_error_contract_matches_rust_codes_messages_and_retryability(self) -> None:
        expected = {
            "audio_invalid_input": ("The selected media file is invalid.", False),
            "audio_unsupported_type": ("This media type is not supported.", False),
            "audio_too_large": ("The selected media file exceeds the size limit.", False),
            "audio_too_long": ("The selected recording exceeds the duration limit.", False),
            "audio_stream_missing": (
                "The selected media does not contain a supported audio stream.",
                False,
            ),
            "audio_model_missing": (
                "The required local transcription model is missing.",
                False,
            ),
            "audio_model_incompatible": (
                "The local transcription model is incompatible.",
                False,
            ),
            "audio_worker_unavailable": (
                "The local transcription worker is unavailable.",
                True,
            ),
            "audio_worker_start_failed": (
                "The local transcription worker could not start.",
                True,
            ),
            "audio_worker_crashed": (
                "The local transcription worker stopped unexpectedly.",
                True,
            ),
            "audio_protocol_error": (
                "The local transcription worker returned an invalid response.",
                False,
            ),
            "audio_output_too_large": (
                "The transcription output exceeds the safety limit.",
                False,
            ),
            "audio_integrity_failed": ("A transcription integrity check failed.", False),
            "audio_timeout": ("Local transcription timed out.", True),
            "audio_cancelled": ("Local transcription was cancelled.", False),
            "internal_error": ("An internal error occurred.", False),
        }
        self.assertEqual(
            {code.value: SAFE_ERROR_DETAILS[code] for code in TranscriptionErrorCode},
            expected,
        )

    def test_isolated_module_health_is_one_shot_and_offline(self) -> None:
        request = {
            "protocolVersion": "1",
            "requestId": str(uuid.uuid4()),
            "operation": "health",
        }
        environment = {
            "PATH": os.environ.get("PATH", ""),
            "SYSTEMROOT": os.environ.get("SYSTEMROOT", ""),
        }
        completed = subprocess.run(
            [sys.executable, "-I", "-m", "meetinglens_transcription_worker"],
            input=json.dumps(request).encode() + b"\n",
            capture_output=True,
            check=False,
            env=environment,
            timeout=30,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr.decode(errors="replace"))
        response = json.loads(completed.stdout)
        self.assertTrue(response["ok"])
        self.assertFalse(response["result"]["systemFfmpegRequired"])

    def _request(self) -> dict[str, object]:
        import hashlib

        return {
            "protocolVersion": "1",
            "requestId": self.request_id,
            "operation": "transcribe",
            "inputPath": str(self.input_path),
            "allowedInputRoot": str(self.inputs),
            "outputRoot": str(self.output),
            "expectedSha256": hashlib.sha256(self.input_path.read_bytes()).hexdigest(),
            "expectedSizeBytes": self.input_path.stat().st_size,
            "detectedMediaType": "audio/wav",
            "safeExtension": "wav",
        }

    def _execute(
        self,
        payload: dict[str, object],
        *,
        engine_factory: object,
    ) -> bytes:
        return self._execute_raw(
            json.dumps(payload).encode() + b"\n",
            engine_factory=engine_factory,
        )

    def _execute_raw(
        self,
        raw: bytes,
        *,
        engine_factory: object = FakeEngine,
        extra_environment: dict[str, str] | None = None,
    ) -> bytes:
        stdin = io.TextIOWrapper(io.BytesIO(raw), encoding="utf-8")
        output_bytes = io.BytesIO()
        stdout = io.TextIOWrapper(output_bytes, encoding="utf-8")
        environment = {
            MODELS_ROOT_ENV: str(self.models),
            STAGING_ROOT_ENV: str(self.staging),
            **(extra_environment or {}),
        }
        with patch.dict(os.environ, environment, clear=True):
            self.assertEqual(
                run_once(stdin, stdout, engine_factory=engine_factory),
                0,
            )
        stdout.flush()
        return output_bytes.getvalue()


if __name__ == "__main__":
    unittest.main()
