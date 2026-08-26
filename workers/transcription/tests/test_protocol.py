from __future__ import annotations

import json
import unittest
import uuid

from pydantic import ValidationError

from meetinglens_transcription_worker.constants import TranscriptionErrorCode
from meetinglens_transcription_worker.errors import WorkerError
from meetinglens_transcription_worker.protocol import (
    TranscribeRequest,
    decode_request,
    encode_line,
)


def _payload() -> dict[str, object]:
    return {
        "protocolVersion": "1",
        "requestId": str(uuid.uuid4()),
        "operation": "transcribe",
        "inputPath": "C:\\private\\source.mp4",
        "allowedInputRoot": "C:\\private",
        "outputRoot": "C:\\staging\\job",
        "expectedSha256": "a" * 64,
        "expectedSizeBytes": 100,
        "detectedMediaType": "audio/mp4",
        "safeExtension": "mp4",
    }


class ProtocolTests(unittest.TestCase):
    def test_decodes_strict_transcribe_request(self) -> None:
        request = decode_request(json.dumps(_payload()).encode())
        self.assertIsInstance(request, TranscribeRequest)
        self.assertEqual(request.safe_extension, "mp4")

    def test_rejects_extension_media_type_mismatch(self) -> None:
        payload = _payload()
        payload["safeExtension"] = "mp3"
        with self.assertRaises(WorkerError) as context:
            decode_request(json.dumps(payload).encode())
        self.assertEqual(context.exception.code, TranscriptionErrorCode.PROTOCOL_ERROR)

    def test_rejects_unknown_fields_and_coercion(self) -> None:
        for field, value in (("unknown", True), ("expectedSizeBytes", "100")):
            payload = _payload()
            payload[field] = value
            with self.subTest(field=field):
                with self.assertRaises(WorkerError):
                    decode_request(json.dumps(payload).encode())

    def test_rejects_noncanonical_request_id(self) -> None:
        payload = _payload()
        payload["requestId"] = str(uuid.uuid4()).upper()
        with self.assertRaises(ValidationError):
            TranscribeRequest.model_validate(payload)

    def test_encoded_line_is_compact_jsonl(self) -> None:
        encoded = encode_line({"synthetic": True})
        self.assertEqual(encoded, b'{"synthetic":true}\n')


if __name__ == "__main__":
    unittest.main()
