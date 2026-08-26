from __future__ import annotations

import json
import os
import subprocess
import sys
import uuid


def main() -> int:
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
    if completed.returncode != 0:
        return 1
    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return 1
    return 0 if response.get("ok") is True else 1


if __name__ == "__main__":
    raise SystemExit(main())
