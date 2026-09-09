from __future__ import annotations

import io
import json
import os
import unittest
import uuid
from contextlib import nullcontext
from unittest.mock import patch

from meetinglens_docling_worker.constants import ParserErrorCode
from meetinglens_docling_worker.errors import WorkerError
from meetinglens_docling_worker.worker import BoundedDiscardSink, run_once


def _request(operation: str = "health") -> dict[str, object]:
    return {
        "protocolVersion": "2",
        "requestId": str(uuid.uuid4()),
        "operation": operation,
    }


def _streams(payload: bytes) -> tuple[io.TextIOWrapper, io.TextIOWrapper, io.BytesIO]:
    stdin = io.TextIOWrapper(io.BytesIO(payload), encoding="utf-8")
    output_bytes = io.BytesIO()
    stdout = io.TextIOWrapper(output_bytes, encoding="utf-8")
    return stdin, stdout, output_bytes


class WorkerTests(unittest.TestCase):
    def test_success_response_preserves_request_id(self) -> None:
        payload = _request()
        response = self._run(payload)
        self.assertTrue(response["ok"])
        self.assertEqual(response["requestId"], payload["requestId"])

    def test_handled_error_uses_stable_safe_payload(self) -> None:
        with patch(
            "meetinglens_docling_worker.worker.dispatch",
            side_effect=WorkerError(ParserErrorCode.MODELS_MISSING),
        ):
            response = self._run(_request())
        self.assertFalse(response["ok"])
        self.assertEqual(response["error"]["code"], "parser_models_missing")
        self.assertNotIn("traceback", json.dumps(response).lower())

    def test_unknown_exception_maps_to_internal_error(self) -> None:
        with patch(
            "meetinglens_docling_worker.worker.dispatch", side_effect=RuntimeError("private")
        ):
            response = self._run(_request())
        self.assertEqual(response["error"]["code"], "internal_error")
        self.assertNotIn("private", json.dumps(response))

    def test_invalid_json_uses_zero_request_id(self) -> None:
        response = self._run_raw(b"{\n")
        self.assertEqual(response["requestId"], "00000000-0000-0000-0000-000000000000")
        self.assertEqual(response["error"]["code"], "parser_protocol_error")

    def test_rejects_multiple_requests_in_one_process(self) -> None:
        first = json.dumps(_request()).encode()
        second = json.dumps(_request()).encode()
        response = self._run_raw(first + b"\n" + second + b"\n")
        self.assertEqual(response["error"]["code"], "parser_protocol_error")

    def test_secret_environment_fails_closed(self) -> None:
        with patch.dict(os.environ, {"SYNTHETIC_API_KEY": "placeholder"}, clear=True):
            response = self._run(_request(), clean_environment=False)
        self.assertEqual(response["error"]["code"], "parser_invalid_input")

    def test_third_party_stdout_is_discarded(self) -> None:
        def noisy_dispatch(_request: object) -> dict[str, object]:
            print("untrusted-noise")
            return {"status": "ok"}

        with patch("meetinglens_docling_worker.worker.dispatch", side_effect=noisy_dispatch):
            raw, response = self._run_with_raw(_request())
        self.assertEqual(raw.count(b"\n"), 1)
        self.assertNotIn(b"untrusted-noise", raw)
        self.assertTrue(response["ok"])

    def test_bounded_sink_never_retains_content(self) -> None:
        sink = BoundedDiscardSink(limit=4)
        self.assertEqual(sink.write("private-value"), len("private-value"))
        self.assertEqual(sink.observed, 5)
        self.assertFalse(hasattr(sink, "value"))

    def _run(
        self,
        payload: dict[str, object],
        *,
        clean_environment: bool = True,
    ) -> dict[str, object]:
        return self._run_with_raw(payload, clean_environment=clean_environment)[1]

    def _run_with_raw(
        self,
        payload: dict[str, object],
        *,
        clean_environment: bool = True,
    ) -> tuple[bytes, dict[str, object]]:
        raw = json.dumps(payload).encode() + b"\n"
        output = self._execute(raw, clean_environment=clean_environment)
        return output, json.loads(output)

    def _run_raw(self, raw: bytes) -> dict[str, object]:
        return json.loads(self._execute(raw))

    def _execute(self, raw: bytes, *, clean_environment: bool = True) -> bytes:
        stdin, stdout, output_bytes = _streams(raw)
        context = patch.dict(os.environ, {}, clear=True) if clean_environment else nullcontext()
        with context:
            self.assertEqual(run_once(stdin, stdout), 0)
        stdout.flush()
        return output_bytes.getvalue()


if __name__ == "__main__":
    unittest.main()
