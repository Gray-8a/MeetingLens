from __future__ import annotations

import json
import unittest
import uuid

from meetinglens_docling_worker.constants import MAX_PROTOCOL_MESSAGE_BYTES, ParserErrorCode
from meetinglens_docling_worker.errors import WorkerError
from meetinglens_docling_worker.protocol import (
    CapabilitiesRequest,
    HealthRequest,
    ParseRequest,
    decode_request,
    encode_response,
)


def _base(operation: str) -> dict[str, object]:
    return {
        "protocolVersion": "2",
        "requestId": str(uuid.uuid4()),
        "operation": operation,
    }


def _parse_payload() -> dict[str, object]:
    return {
        **_base("parse"),
        "documentId": str(uuid.uuid4()),
        "meetingId": str(uuid.uuid4()),
        "inputPath": "C:\\private\\input.txt",
        "allowedInputRoot": "C:\\private",
        "expectedSha256": "a" * 64,
        "detectedMediaType": "text/plain",
        "safeExtension": "txt",
        "outputRoot": "C:\\private\\parser-staging\\job",
        "parserOptions": {"ocr": False, "maxPages": 300, "documentTimeoutSeconds": 120.0},
    }


class ProtocolTests(unittest.TestCase):
    def test_decodes_health_request(self) -> None:
        request = decode_request(json.dumps(_base("health")).encode())
        self.assertIsInstance(request, HealthRequest)

    def test_decodes_capabilities_request(self) -> None:
        request = decode_request(json.dumps(_base("capabilities")).encode())
        self.assertIsInstance(request, CapabilitiesRequest)

    def test_decodes_parse_request(self) -> None:
        request = decode_request(json.dumps(_parse_payload()).encode())
        self.assertIsInstance(request, ParseRequest)
        self.assertEqual(request.safe_extension, "txt")

    def test_rejects_unknown_protocol_version(self) -> None:
        payload = _base("health")
        payload["protocolVersion"] = "1"
        self._assert_protocol_error(payload)

    def test_rejects_unknown_operation(self) -> None:
        self._assert_protocol_error(_base("debug"))

    def test_rejects_invalid_json(self) -> None:
        with self.assertRaises(WorkerError) as context:
            decode_request(b"{")
        self.assertEqual(context.exception.code, ParserErrorCode.PROTOCOL_ERROR)

    def test_rejects_oversized_request(self) -> None:
        with self.assertRaises(WorkerError):
            decode_request(b"x" * (MAX_PROTOCOL_MESSAGE_BYTES + 1))

    def test_rejects_unknown_fields(self) -> None:
        payload = _base("health")
        payload["apiKey"] = "synthetic-placeholder"
        self._assert_protocol_error(payload)

    def test_rejects_noncanonical_uuid(self) -> None:
        payload = _base("health")
        payload["requestId"] = str(uuid.uuid4()).upper()
        self._assert_protocol_error(payload)

    def test_rejects_invalid_hash(self) -> None:
        payload = _parse_payload()
        payload["expectedSha256"] = "not-a-hash"
        self._assert_protocol_error(payload)

    def test_rejects_limits_above_production_maximum(self) -> None:
        payload = _parse_payload()
        payload["parserOptions"] = {
            "ocr": False,
            "maxPages": 301,
            "documentTimeoutSeconds": 121.0,
        }
        self._assert_protocol_error(payload)

    def test_response_is_compact_json_line(self) -> None:
        encoded = encode_response({"ok": True, "value": "synthetic"})
        self.assertTrue(encoded.endswith(b"\n"))
        self.assertEqual(json.loads(encoded), {"ok": True, "value": "synthetic"})

    def _assert_protocol_error(self, payload: dict[str, object]) -> None:
        with self.assertRaises(WorkerError) as context:
            decode_request(json.dumps(payload).encode())
        self.assertEqual(context.exception.code, ParserErrorCode.PROTOCOL_ERROR)


if __name__ == "__main__":
    unittest.main()
