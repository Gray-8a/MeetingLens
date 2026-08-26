from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from helpers import write_synthetic_model

from meetinglens_transcription_worker.constants import (
    MODEL_MANIFEST_NAME,
    MODEL_REQUIRED_FILES,
    MODEL_REVISION,
    TranscriptionErrorCode,
)
from meetinglens_transcription_worker.errors import WorkerError
from meetinglens_transcription_worker.models import (
    models_status,
    provision_models,
    require_models,
)


class ModelTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.models = self.root / "models"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_manifest_pins_revision_and_every_file_hash(self) -> None:
        manifest = write_synthetic_model(self.models)
        present, compatible, loaded = models_status(self.models)
        self.assertTrue(present)
        self.assertTrue(compatible)
        self.assertEqual(loaded, manifest)
        self.assertEqual(loaded["revision"], MODEL_REVISION)

    def test_rejects_tampered_missing_or_extra_model_file(self) -> None:
        for action in ("tamper", "missing", "extra"):
            with self.subTest(action=action):
                if self.models.exists():
                    for child in self.models.rglob("*"):
                        if child.is_file():
                            child.unlink()
                    for child in sorted(self.models.rglob("*"), reverse=True):
                        if child.is_dir():
                            child.rmdir()
                    self.models.rmdir()
                write_synthetic_model(self.models)
                if action == "tamper":
                    (self.models / "model" / MODEL_REQUIRED_FILES[0]).write_bytes(b"tampered")
                elif action == "missing":
                    (self.models / "model" / MODEL_REQUIRED_FILES[0]).unlink()
                else:
                    (self.models / "model" / "extra.bin").write_bytes(b"extra")
                self.assertEqual(models_status(self.models)[:2], (True, False))

    def test_rejects_manifest_extra_field_and_revision_change(self) -> None:
        write_synthetic_model(self.models)
        path = self.models / MODEL_MANIFEST_NAME
        for change in ({"unexpected": True}, {"revision": "0" * 40}):
            with self.subTest(change=change):
                manifest = json.loads(path.read_text(encoding="utf-8"))
                manifest.update(change)
                path.write_text(json.dumps(manifest), encoding="utf-8")
                with self.assertRaises(WorkerError) as context:
                    require_models(self.models)
                self.assertEqual(
                    context.exception.code,
                    TranscriptionErrorCode.MODEL_INCOMPATIBLE,
                )
                write_synthetic_model_after_cleanup(self.models)

    def test_provision_uses_exact_source_and_builds_inventory(self) -> None:
        observed: dict[str, object] = {}

        def fake_download(**kwargs: object) -> str:
            observed.update(kwargs)
            snapshot = Path(str(kwargs["cache_dir"])) / "snapshot"
            snapshot.mkdir(parents=True)
            for name in MODEL_REQUIRED_FILES:
                (snapshot / name).write_bytes(f"downloaded-{name}".encode())
            return str(snapshot)

        manifest = provision_models(self.models, downloader=fake_download)
        self.assertEqual(observed["revision"], MODEL_REVISION)
        self.assertFalse(observed["token"])
        self.assertEqual(len(manifest["files"]), len(MODEL_REQUIRED_FILES))
        self.assertTrue(models_status(self.models)[1])

    def test_isolated_module_verify_cli_executes(self) -> None:
        write_synthetic_model(self.models)
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-m",
                "meetinglens_transcription_worker.models",
                "--models-root",
                str(self.models),
                "--verify-only",
            ],
            capture_output=True,
            check=False,
            timeout=30,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr.decode(errors="replace"))
        self.assertIn(b"verified", completed.stdout)


def write_synthetic_model_after_cleanup(root: Path) -> None:
    for child in root.rglob("*"):
        if child.is_file():
            child.unlink()
    for child in sorted(root.rglob("*"), reverse=True):
        if child.is_dir():
            child.rmdir()
    root.rmdir()
    write_synthetic_model(root)


if __name__ == "__main__":
    unittest.main()
