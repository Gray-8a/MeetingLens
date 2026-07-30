from __future__ import annotations

import contextlib
import hmac
import io
import json
import threading
import unittest
from http import HTTPStatus
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from sidecar.meetinglens_sidecar.server import (
    DEFAULT_HOST,
    SESSION_TOKEN_ENV,
    SidecarConfigurationError,
    create_server,
    load_session_token,
)

TEST_TOKEN = "synthetic-sidecar-session-token-for-tests-0001"
WRONG_TOKEN = "synthetic-sidecar-session-token-for-tests-0002"


class HealthEndpointTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = create_server(port=0, session_token=TEST_TOKEN)
        host, port = cls.server.server_address[:2]
        cls.base_url = f"http://{host}:{port}"
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def request(
        self,
        path: str = "/health",
        token: str | None = TEST_TOKEN,
        scheme: str = "Bearer",
        headers: dict[str, str] | None = None,
    ) -> Request:
        request_headers = {} if headers is None else dict(headers)
        if token is not None:
            request_headers["Authorization"] = f"{scheme} {token}"
        return Request(f"{self.base_url}{path}", headers=request_headers)

    def assert_unauthorized(self, request: Request) -> dict[str, object]:
        with self.assertRaises(HTTPError) as context:
            urlopen(request, timeout=1)
        error = context.exception
        self.assertEqual(error.code, HTTPStatus.UNAUTHORIZED)
        payload = json.load(error)
        self.assertEqual(payload["status"], "error")
        self.assertEqual(payload["error"]["code"], "sidecar_unauthorized")
        self.assertNotIn(TEST_TOKEN, json.dumps(payload))
        self.assertNotIn(WRONG_TOKEN, json.dumps(payload))
        return payload

    def test_health_returns_stable_versioned_payload_with_correct_token(self) -> None:
        with urlopen(self.request(), timeout=1) as response:
            payload = json.load(response)

        self.assertEqual(response.status, HTTPStatus.OK)
        self.assertEqual(response.headers.get_content_type(), "application/json")
        self.assertEqual(response.headers["Cache-Control"], "no-store")
        self.assertEqual(
            payload,
            {
                "status": "ok",
                "service": "meetinglens-sidecar",
                "version": "0.1.0",
                "api_version": "v1",
            },
        )

    def test_missing_authorization_header_returns_safe_401(self) -> None:
        self.assert_unauthorized(self.request(token=None))

    def test_wrong_scheme_returns_the_same_safe_401(self) -> None:
        self.assert_unauthorized(self.request(token=TEST_TOKEN, scheme="Basic"))

    def test_wrong_token_returns_the_same_safe_401(self) -> None:
        self.assert_unauthorized(self.request(token=WRONG_TOKEN))

    def test_wrong_token_does_not_execute_health_route(self) -> None:
        before = self.server.authenticated_health_requests
        self.assert_unauthorized(self.request(token=WRONG_TOKEN))
        self.assertEqual(self.server.authenticated_health_requests, before)

    def test_query_cookie_and_body_are_not_authentication_channels(self) -> None:
        request = self.request(
            path=f"/health?token={TEST_TOKEN}",
            token=None,
            headers={"Cookie": f"token={TEST_TOKEN}"},
        )
        self.assert_unauthorized(request)

    def test_unknown_path_requires_authentication_before_404(self) -> None:
        self.assert_unauthorized(self.request(path="/private-meeting", token=None))
        with self.assertRaises(HTTPError) as context:
            urlopen(self.request(path="/private-meeting"), timeout=1)
        self.assertEqual(context.exception.code, HTTPStatus.NOT_FOUND)
        payload = json.load(context.exception)
        self.assertEqual(payload["error"]["code"], "not_found")

    def test_constant_time_compare_is_used_for_authentication(self) -> None:
        with patch(
            "sidecar.meetinglens_sidecar.server.hmac.compare_digest",
            wraps=hmac.compare_digest,
        ) as compare:
            with urlopen(self.request(), timeout=1):
                pass
        compare.assert_called_once_with(TEST_TOKEN, TEST_TOKEN)

    def test_health_response_does_not_disclose_environment_or_paths(self) -> None:
        with urlopen(self.request(), timeout=1) as response:
            serialized = json.dumps(json.load(response)).lower()
        for forbidden in ["environment", "executable", "username", "path", "token", "header"]:
            self.assertNotIn(forbidden, serialized)

    def test_authentication_failure_does_not_log_or_echo_header(self) -> None:
        captured = io.StringIO()
        with contextlib.redirect_stderr(captured):
            payload = self.assert_unauthorized(self.request(token=WRONG_TOKEN))
        combined = captured.getvalue() + json.dumps(payload)
        self.assertNotIn(TEST_TOKEN, combined)
        self.assertNotIn(WRONG_TOKEN, combined)
        self.assertNotIn("Authorization", combined)


class SidecarConfigurationTests(unittest.TestCase):
    def test_missing_session_token_fails_closed(self) -> None:
        with self.assertRaisesRegex(SidecarConfigurationError, "sidecar_session_missing"):
            load_session_token({})

    def test_empty_or_short_session_token_fails_closed(self) -> None:
        for value in ["", "short", " " * 32]:
            with self.subTest(value_length=len(value)):
                with self.assertRaisesRegex(SidecarConfigurationError, "sidecar_session_missing"):
                    load_session_token({SESSION_TOKEN_ENV: value})

    def test_server_binds_only_to_ipv4_loopback(self) -> None:
        server = create_server(port=0, session_token=TEST_TOKEN)
        try:
            self.assertEqual(server.server_address[0], DEFAULT_HOST)
        finally:
            server.server_close()

    def test_wildcard_lan_and_ipv6_hosts_are_rejected(self) -> None:
        for host in ["0.0.0.0", "192.168.1.20", "::", "::1", "localhost"]:
            with self.subTest(host=host):
                with self.assertRaisesRegex(SidecarConfigurationError, "sidecar_host_not_allowed"):
                    create_server(host=host, port=0, session_token=TEST_TOKEN)

    def test_new_session_rejects_the_previous_token(self) -> None:
        first = create_server(port=0, session_token=TEST_TOKEN)
        first.server_close()

        second = create_server(port=0, session_token=WRONG_TOKEN)
        host, port = second.server_address[:2]
        thread = threading.Thread(target=second.serve_forever, daemon=True)
        thread.start()
        try:
            request = Request(
                f"http://{host}:{port}/health",
                headers={"Authorization": f"Bearer {TEST_TOKEN}"},
            )
            with self.assertRaises(HTTPError) as context:
                urlopen(request, timeout=1)
            self.assertEqual(context.exception.code, HTTPStatus.UNAUTHORIZED)
        finally:
            second.shutdown()
            second.server_close()
            thread.join(timeout=2)


if __name__ == "__main__":
    unittest.main()
