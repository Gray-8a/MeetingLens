from __future__ import annotations

import json
import threading
import unittest
from http import HTTPStatus
from urllib.error import HTTPError
from urllib.request import urlopen

from sidecar.meetinglens_sidecar.server import create_server


class HealthEndpointTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = create_server(port=0)
        host, port = cls.server.server_address[:2]
        cls.base_url = f"http://{host}:{port}"
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def test_health_returns_stable_versioned_payload(self) -> None:
        with urlopen(f"{self.base_url}/health", timeout=1) as response:
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

    def test_unknown_path_returns_json_404(self) -> None:
        with self.assertRaises(HTTPError) as context:
            urlopen(f"{self.base_url}/private-meeting", timeout=1)

        error = context.exception
        self.assertEqual(error.code, HTTPStatus.NOT_FOUND)
        self.assertEqual(json.load(error), {"status": "error", "error": "not_found"})


if __name__ == "__main__":
    unittest.main()
