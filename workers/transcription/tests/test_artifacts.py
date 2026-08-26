from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from meetinglens_transcription_worker.adapter import TranscriptPayload, TranscriptSegment
from meetinglens_transcription_worker.artifacts import write_artifacts
from meetinglens_transcription_worker.constants import MODEL_REVISION, TranscriptionErrorCode
from meetinglens_transcription_worker.errors import WorkerError
from meetinglens_transcription_worker.protocol import TranscribeRequest


class ArtifactTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.output = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_writes_strict_jsonl_and_manifest_with_hashes(self) -> None:
        result = self._write()
        self.assertEqual(
            sorted(path.name for path in self.output.iterdir()),
            ["manifest.json", "segments.jsonl"],
        )
        lines = (self.output / "segments.jsonl").read_text(encoding="utf-8").splitlines()
        self.assertEqual(json.loads(lines[0])["text"], "Synthetic opening.")
        manifest = json.loads((self.output / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["modelRevision"], MODEL_REVISION)
        self.assertEqual(manifest["segmentsArtifact"], result["segmentsArtifact"])
        expected_keys = {
            "schemaVersion",
            "protocolVersion",
            "requestId",
            "status",
            "parserName",
            "parserVersion",
            "inputSha256",
            "model",
            "modelRevision",
            "fasterWhisperVersion",
            "device",
            "computeType",
            "language",
            "durationMs",
            "segmentCount",
            "beamSize",
            "task",
            "segmentsArtifact",
        }
        self.assertEqual(set(manifest), expected_keys)
        for artifact in (result["segmentsArtifact"], result["manifestArtifact"]):
            data = (self.output / artifact["relativePath"]).read_bytes()
            self.assertEqual(artifact["sizeBytes"], len(data))
            self.assertEqual(artifact["sha256"], hashlib.sha256(data).hexdigest())

    def test_manifest_contains_no_absolute_paths(self) -> None:
        self._write()
        data = (self.output / "manifest.json").read_text(encoding="utf-8")
        self.assertNotIn(str(self.output), data)

    def test_rejects_segment_line_and_total_artifact_limits(self) -> None:
        with patch(
            "meetinglens_transcription_worker.artifacts.MAX_PROTOCOL_LINE_BYTES",
            32,
        ):
            self._assert_error(TranscriptionErrorCode.OUTPUT_TOO_LARGE)
        with patch("meetinglens_transcription_worker.artifacts.MAX_ARTIFACT_BYTES", 16):
            self._assert_error(TranscriptionErrorCode.OUTPUT_TOO_LARGE)

    def _write(self) -> dict[str, object]:
        return write_artifacts(
            output_root=self.output,
            request=self._request(),
            payload=TranscriptPayload(
                language="en",
                language_probability=0.99,
                duration_ms=2_000,
                segments=(
                    TranscriptSegment(0, 1_000, "Synthetic opening."),
                    TranscriptSegment(1_000, 2_000, "Synthetic decision."),
                ),
            ),
        )

    def _request(self) -> TranscribeRequest:
        return TranscribeRequest.model_validate(
            {
                "protocolVersion": "1",
                "requestId": str(uuid.uuid4()),
                "operation": "transcribe",
                "inputPath": "C:\\private\\synthetic.wav",
                "allowedInputRoot": "C:\\private",
                "outputRoot": "C:\\staging\\job",
                "expectedSha256": "a" * 64,
                "expectedSizeBytes": 20,
                "detectedMediaType": "audio/wav",
                "safeExtension": "wav",
            }
        )

    def _assert_error(self, code: TranscriptionErrorCode) -> None:
        with self.assertRaises(WorkerError) as context:
            self._write()
        self.assertEqual(context.exception.code, code)


if __name__ == "__main__":
    unittest.main()
