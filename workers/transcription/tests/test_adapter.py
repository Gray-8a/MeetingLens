from __future__ import annotations

import ast
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from meetinglens_transcription_worker.adapter import _collect_segments, probe_audio
from meetinglens_transcription_worker.constants import TranscriptionErrorCode
from meetinglens_transcription_worker.errors import WorkerError


class _Container:
    def __init__(self, *, audio: bool, duration: int | None = 1_000_000) -> None:
        self.streams = [SimpleNamespace(type="audio" if audio else "video")]
        self.duration = duration

    def __enter__(self) -> _Container:
        return self

    def __exit__(self, *_args: object) -> None:
        return None


class AdapterTests(unittest.TestCase):
    def test_probe_rejects_video_only_container(self) -> None:
        with patch(
            "meetinglens_transcription_worker.adapter.av.open", return_value=_Container(audio=False)
        ):
            with self.assertRaises(WorkerError) as context:
                probe_audio(Path("synthetic.mp4"))
        self.assertEqual(context.exception.code, TranscriptionErrorCode.STREAM_MISSING)

    def test_segment_collection_normalizes_whitespace_and_overlaps(self) -> None:
        observed = []
        result = _collect_segments(
            [
                SimpleNamespace(start=0.0, end=1.0, text="  Synthetic\nopening. "),
                SimpleNamespace(start=0.9, end=2.0, text=" Synthetic outcome. "),
            ],
            2_000,
            lambda processed, total: observed.append((processed, total)),
        )
        self.assertEqual(result[0].text, "Synthetic opening.")
        self.assertEqual(result[1].start_offset_ms, result[0].end_offset_ms)
        self.assertEqual(observed[-1], (2_000, 2_000))

    def test_production_source_never_launches_system_media_tools(self) -> None:
        package = Path(__file__).parents[1] / "src" / "meetinglens_transcription_worker"
        forbidden_imports = {"subprocess", "ffmpeg", "ffmpeg_python"}
        for path in package.glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            imported = {
                alias.name.split(".")[0]
                for node in ast.walk(tree)
                if isinstance(node, (ast.Import, ast.ImportFrom))
                for alias in (
                    node.names
                    if isinstance(node, ast.Import)
                    else [SimpleNamespace(name=node.module or "")]
                )
            }
            self.assertTrue(imported.isdisjoint(forbidden_imports), path.name)


if __name__ == "__main__":
    unittest.main()
