"""Authenticated loopback server for the MeetingLens processing sidecar."""

from __future__ import annotations

import argparse
import hmac
import json
import os
from collections.abc import Mapping
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import urlsplit

from . import __version__

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8765
API_VERSION = "v1"
SERVICE_NAME = "meetinglens-sidecar"
SESSION_TOKEN_ENV = "MEETINGLENS_SESSION_TOKEN"
MINIMUM_TOKEN_LENGTH = 32


class SidecarConfigurationError(RuntimeError):
    """Represent a safe, non-secret startup configuration failure."""


class SidecarHttpServer(ThreadingHTTPServer):
    """Threaded loopback server with one in-memory application session token."""

    daemon_threads = True

    def __init__(
        self,
        server_address: tuple[str, int],
        handler: type[BaseHTTPRequestHandler],
        session_token: str,
    ) -> None:
        super().__init__(server_address, handler)
        self._session_token = session_token
        self.authenticated_health_requests = 0

    def authenticates(self, authorization: str | None) -> bool:
        if authorization is None:
            return False
        scheme, separator, supplied_token = authorization.partition(" ")
        if separator != " " or scheme.lower() != "bearer" or not supplied_token:
            return False
        return hmac.compare_digest(supplied_token, self._session_token)


class SidecarRequestHandler(BaseHTTPRequestHandler):
    """Authenticate every request before dispatching a narrow route contract."""

    protocol_version = "HTTP/1.1"
    server_version = "MeetingLensSidecar"
    sys_version = ""

    @property
    def sidecar_server(self) -> SidecarHttpServer:
        server = self.server
        if not isinstance(server, SidecarHttpServer):
            raise SidecarConfigurationError("sidecar_server_invalid")
        return server

    def do_GET(self) -> None:  # noqa: N802 - required by BaseHTTPRequestHandler
        authorization_values = self.headers.get_all("Authorization", failobj=[])
        authorization = authorization_values[0] if len(authorization_values) == 1 else None
        if not self.sidecar_server.authenticates(authorization):
            self._send_json(
                HTTPStatus.UNAUTHORIZED,
                {
                    "status": "error",
                    "error": {
                        "code": "sidecar_unauthorized",
                        "message": "Sidecar authentication failed.",
                        "retryable": False,
                    },
                },
            )
            return

        path = urlsplit(self.path).path
        if path == "/health":
            self.sidecar_server.authenticated_health_requests += 1
            self._send_json(
                HTTPStatus.OK,
                {
                    "status": "ok",
                    "service": SERVICE_NAME,
                    "version": __version__,
                    "api_version": API_VERSION,
                },
            )
            return

        self._send_json(
            HTTPStatus.NOT_FOUND,
            {
                "status": "error",
                "error": {
                    "code": "not_found",
                    "message": "The requested route was not found.",
                    "retryable": False,
                },
            },
        )

    def _send_json(self, status: HTTPStatus, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        """Disable request logging so headers, paths, and query values cannot leak."""


def load_session_token(environment: Mapping[str, str] | None = None) -> str:
    """Read the parent-provided token without providing a development fallback."""

    values = os.environ if environment is None else environment
    token = values.get(SESSION_TOKEN_ENV, "")
    if (
        len(token) < MINIMUM_TOKEN_LENGTH
        or token.strip() != token
        or any(character.isspace() for character in token)
    ):
        raise SidecarConfigurationError("sidecar_session_missing")
    return token


def validate_host(host: str) -> str:
    """Reject every non-IPv4-loopback binding, including wildcard addresses."""

    if host != DEFAULT_HOST:
        raise SidecarConfigurationError("sidecar_host_not_allowed")
    return host


def create_server(
    host: str = DEFAULT_HOST,
    port: int = DEFAULT_PORT,
    session_token: str | None = None,
) -> SidecarHttpServer:
    """Create an authenticated loopback server without starting its event loop."""

    safe_host = validate_host(host)
    token = (
        load_session_token()
        if session_token is None
        else load_session_token({SESSION_TOKEN_ENV: session_token})
    )
    return SidecarHttpServer((safe_host, port), SidecarRequestHandler, token)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the MeetingLens local sidecar")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    server = create_server(port=args.port)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
