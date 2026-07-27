"""Dependency-free health server for the MeetingLens processing sidecar."""

from __future__ import annotations

import argparse
import json
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import urlsplit

from . import __version__

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8765
API_VERSION = "v1"
SERVICE_NAME = "meetinglens-sidecar"


class SidecarHttpServer(ThreadingHTTPServer):
    """Threaded server that does not keep the process alive for open requests."""

    daemon_threads = True


class SidecarRequestHandler(BaseHTTPRequestHandler):
    """Serve the intentionally narrow unauthenticated health contract."""

    protocol_version = "HTTP/1.1"
    server_version = "MeetingLensSidecar"
    sys_version = ""

    def do_GET(self) -> None:  # noqa: N802 - required by BaseHTTPRequestHandler
        path = urlsplit(self.path).path
        if path == "/health":
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
                "error": "not_found",
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
        """Disable request logging so paths cannot leak private job identifiers later."""


def create_server(host: str = DEFAULT_HOST, port: int = DEFAULT_PORT) -> SidecarHttpServer:
    """Create a server without starting its event loop."""

    return SidecarHttpServer((host, port), SidecarRequestHandler)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the MeetingLens local sidecar")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    server = create_server(args.host, args.port)
    try:
        server.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
