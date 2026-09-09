from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

import psutil
from docx import Document
from openpyxl import Workbook
from PIL import Image, ImageDraw, ImageFont
from pptx import Presentation

PROTOCOL_VERSION = "2"
SYNTHETIC_TEXT = "MeetingLens synthetic offline parser smoke 2026"
SECRET_SENTINEL = "MEETINGLENS_SMOKE_SECRET_SENTINEL"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _write_pdf(path: Path, lines: list[str]) -> None:
    objects: list[bytes] = []
    page_ids = [3 + index * 2 for index in range(len(lines))]
    font_id = 3 + len(lines) * 2
    objects.append(b"<< /Type /Catalog /Pages 2 0 R >>")
    kids = " ".join(f"{page_id} 0 R" for page_id in page_ids)
    objects.append(f"<< /Type /Pages /Kids [{kids}] /Count {len(lines)} >>".encode())
    for index, line in enumerate(lines):
        page_id = page_ids[index]
        content_id = page_id + 1
        objects.append(
            (
                f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                f"/Resources << /Font << /F1 {font_id} 0 R >> >> "
                f"/Contents {content_id} 0 R >>"
            ).encode()
        )
        escaped = line.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
        content = f"BT /F1 18 Tf 72 720 Td ({escaped}) Tj ET".encode()
        objects.append(
            f"<< /Length {len(content)} >>\nstream\n".encode() + content + b"\nendstream"
        )
    objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")

    output = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for object_id, payload in enumerate(objects, start=1):
        offsets.append(len(output))
        output.extend(f"{object_id} 0 obj\n".encode())
        output.extend(payload)
        output.extend(b"\nendobj\n")
    xref_offset = len(output)
    output.extend(f"xref\n0 {len(objects) + 1}\n".encode())
    output.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode())
    output.extend(
        (
            f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\n"
            f"startxref\n{xref_offset}\n%%EOF\n"
        ).encode()
    )
    path.write_bytes(output)


def _create_inputs(root: Path) -> dict[str, tuple[Path, str, str, bool]]:
    inputs: dict[str, tuple[Path, str, str, bool]] = {}
    text_path = root / "synthetic.txt"
    text_path.write_text(SYNTHETIC_TEXT, encoding="utf-8")
    inputs["txt"] = (text_path, "txt", "text/plain", False)

    markdown_path = root / "synthetic.md"
    markdown_path.write_text(f"# Smoke\n\n{SYNTHETIC_TEXT}\n", encoding="utf-8")
    inputs["md"] = (markdown_path, "md", "text/markdown", False)

    csv_path = root / "synthetic.csv"
    csv_path.write_text(f"kind,value\nsmoke,{SYNTHETIC_TEXT}\n", encoding="utf-8")
    inputs["csv"] = (csv_path, "csv", "text/csv", False)

    docx_path = root / "synthetic.docx"
    document = Document()
    document.add_heading("Smoke", 1)
    document.add_paragraph(SYNTHETIC_TEXT)
    document.save(docx_path)
    inputs["docx"] = (
        docx_path,
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        False,
    )

    pptx_path = root / "synthetic.pptx"
    presentation = Presentation()
    slide = presentation.slides.add_slide(presentation.slide_layouts[1])
    slide.shapes.title.text = "Smoke"
    slide.placeholders[1].text = SYNTHETIC_TEXT
    presentation.save(pptx_path)
    inputs["pptx"] = (
        pptx_path,
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        False,
    )

    xlsx_path = root / "synthetic.xlsx"
    workbook = Workbook()
    worksheet = workbook.active
    worksheet.append(["kind", "value"])
    worksheet.append(["smoke", SYNTHETIC_TEXT])
    workbook.save(xlsx_path)
    inputs["xlsx"] = (
        xlsx_path,
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        False,
    )

    pdf_path = root / "synthetic.pdf"
    _write_pdf(pdf_path, [SYNTHETIC_TEXT])
    inputs["digital-pdf"] = (pdf_path, "pdf", "application/pdf", False)

    image = Image.new("RGB", (1500, 400), "white")
    # Keep the synthetic scan legible after PDF rasterization; a stroked tiny
    # default font can produce no OCR text and correctly fail the chunk contract.
    ImageDraw.Draw(image).text(
        (80, 150), SYNTHETIC_TEXT, fill="black", font=ImageFont.load_default(size=48)
    )
    png_path = root / "synthetic.png"
    image.save(png_path, format="PNG")
    inputs["png-ocr"] = (png_path, "png", "image/png", True)
    jpeg_path = root / "synthetic.jpg"
    image.save(jpeg_path, format="JPEG", quality=95)
    inputs["jpeg-ocr"] = (jpeg_path, "jpg", "image/jpeg", True)
    jpeg_alias_path = root / "synthetic.jpeg"
    image.save(jpeg_alias_path, format="JPEG", quality=95)
    inputs["jpeg-extension-ocr"] = (jpeg_alias_path, "jpeg", "image/jpeg", True)
    scanned_pdf = root / "synthetic-scan.pdf"
    image.save(scanned_pdf, format="PDF", resolution=150)
    inputs["scanned-pdf-ocr"] = (scanned_pdf, "pdf", "application/pdf", True)
    return inputs


def _safe_environment(working_root: Path, models_root: Path, staging_root: Path) -> dict[str, str]:
    environment = {
        "PYTHONUTF8": "1",
        "PYTHONIOENCODING": "utf-8",
        "PYTHONNOUSERSITE": "1",
        "OMP_NUM_THREADS": "4",
        "MKL_NUM_THREADS": "4",
        "OPENBLAS_NUM_THREADS": "4",
        "NUMEXPR_NUM_THREADS": "4",
        "HF_HUB_OFFLINE": "1",
        "HF_DATASETS_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
        "HF_HUB_DISABLE_TELEMETRY": "1",
        "DO_NOT_TRACK": "1",
        "HOME": str(working_root),
        "USERPROFILE": str(working_root),
        "LOCALAPPDATA": str(working_root),
        "TEMP": str(working_root),
        "TMP": str(working_root),
        "XDG_CACHE_HOME": str(working_root),
        "MEETINGLENS_DOCLING_MODELS_ROOT": str(models_root),
        "MEETINGLENS_PARSER_STAGING_ROOT": str(staging_root),
    }
    for name in ("SystemRoot", "WINDIR", "ComSpec"):
        if value := os.environ.get(name):
            environment[name] = value
    return environment


def _run_worker(
    request: dict[str, Any],
    environment: dict[str, str],
    working_root: Path,
) -> tuple[dict[str, Any], int, set[int]]:
    process = subprocess.Popen(
        [sys.executable, "-I", "-m", "meetinglens_docling_worker"],
        cwd=working_root,
        env=environment,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        creationflags=subprocess.CREATE_NO_WINDOW,
    )
    command_line = " ".join(psutil.Process(process.pid).cmdline())
    if SECRET_SENTINEL in command_line:
        raise AssertionError("secret sentinel reached the child command line")

    encoded = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    peak_rss = 0
    listening_pids: set[int] = set()
    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(process.communicate, encoded, 150)
        while not future.done():
            try:
                parent = psutil.Process(process.pid)
                descendants = parent.children(recursive=True)
                peak_rss = max(
                    peak_rss,
                    sum(item.memory_info().rss for item in [parent, *descendants]),
                )
                child_pids = {item.pid for item in descendants} | {parent.pid}
                listening_pids.update(
                    item.pid
                    for item in psutil.net_connections(kind="inet")
                    if item.pid in child_pids and item.status == psutil.CONN_LISTEN
                )
            except (psutil.Error, OSError):
                pass
            time.sleep(0.05)
        stdout, stderr = future.result()
    if process.returncode != 0 or stderr or len(stdout) > 1024 * 1024:
        raise AssertionError("worker process boundary failed")
    if SECRET_SENTINEL.encode() in stdout or SECRET_SENTINEL.encode() in stderr:
        raise AssertionError("secret sentinel reached worker output")
    response = json.loads(stdout)
    if response.get("protocolVersion") != PROTOCOL_VERSION:
        raise AssertionError("unexpected protocol version")
    return response, peak_rss, listening_pids


def _base_request(operation: str) -> dict[str, Any]:
    return {
        "protocolVersion": PROTOCOL_VERSION,
        "requestId": str(uuid.uuid4()),
        "operation": operation,
    }


def _parse_request(
    input_path: Path,
    input_root: Path,
    output_root: Path,
    extension: str,
    media_type: str,
    *,
    ocr: bool,
    sha256: str | None = None,
    max_pages: int = 300,
) -> dict[str, Any]:
    return {
        **_base_request("parse"),
        "documentId": str(uuid.uuid4()),
        "meetingId": str(uuid.uuid4()),
        "inputPath": str(input_path.resolve()),
        "allowedInputRoot": str(input_root.resolve()),
        "expectedSha256": sha256 or _sha256(input_path),
        "detectedMediaType": media_type,
        "safeExtension": extension,
        "outputRoot": str(output_root.resolve()),
        "parserOptions": {
            "ocr": ocr,
            "maxPages": max_pages,
            "documentTimeoutSeconds": 120.0,
        },
    }


def _validate_success(
    response: dict[str, Any], output_root: Path, forbidden_paths: list[Path]
) -> None:
    if response.get("ok") is not True:
        raise AssertionError(
            f"expected successful parse, received {response.get('error', {}).get('code')}"
        )
    expected = {"document.json", "content.md", "content.txt", "chunks.jsonl", "manifest.json"}
    actual = {item.name for item in output_root.iterdir() if item.is_file()}
    if actual != expected:
        raise AssertionError("unexpected artifact set")
    manifest = json.loads((output_root / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("chunkSchemaVersion") != 1:
        raise AssertionError("chunk schema mismatch")
    for artifact in manifest["artifacts"]:
        relative = Path(artifact["relativePath"])
        if relative.is_absolute() or ".." in relative.parts:
            raise AssertionError("unsafe artifact path")
        candidate = output_root / relative
        if (
            candidate.stat().st_size != artifact["sizeBytes"]
            or _sha256(candidate) != artifact["sha256"]
        ):
            raise AssertionError("artifact integrity mismatch")
    chunks = [
        json.loads(line)
        for line in (output_root / "chunks.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    if not chunks:
        raise AssertionError("normalized chunks are missing")
    normalized_text = " ".join(chunk["text"] for chunk in chunks).casefold()
    if "synthetic" not in normalized_text or "smoke" not in normalized_text:
        raise AssertionError("synthetic text was not recovered into chunks")
    for ordinal, chunk in enumerate(chunks):
        expected_keys = {
            "schemaVersion",
            "ordinal",
            "kind",
            "text",
            "contentSha256",
            "locator",
        }
        if set(chunk) != expected_keys or chunk["schemaVersion"] != 1:
            raise AssertionError("invalid chunk record")
        if chunk["ordinal"] != ordinal or chunk["kind"] not in {
            "heading",
            "paragraph",
            "list_item",
            "table",
        }:
            raise AssertionError("invalid chunk ordering or kind")
        if hashlib.sha256(chunk["text"].encode("utf-8")).hexdigest() != chunk["contentSha256"]:
            raise AssertionError("chunk content hash mismatch")
    content = b"".join(path.read_bytes() for path in output_root.iterdir() if path.is_file())
    if SECRET_SENTINEL.encode() in content or b"data:image" in content or b"base64," in content:
        raise AssertionError("forbidden content in artifacts")
    for path in forbidden_paths:
        if str(path.resolve()).encode() in content:
            raise AssertionError("absolute path leaked into artifacts")


def run(models_root: Path) -> dict[str, Any]:
    if sys.platform != "win32":
        raise RuntimeError("This acceptance smoke is Windows-only.")
    os.environ["MEETINGLENS_SMOKE_API_KEY"] = SECRET_SENTINEL
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="MeetingLensDoclingSmoke-") as temporary:
        root = Path(temporary)
        input_root = root / "inputs"
        staging_root = root / "parser-staging"
        working_root = root / "worker-home"
        for path in (input_root, staging_root, working_root):
            path.mkdir()
        environment = _safe_environment(
            working_root, models_root.resolve(strict=True), staging_root
        )

        health, health_rss, health_ports = _run_worker(
            _base_request("health"), environment, working_root
        )
        if not health.get("ok") or health["result"] != {
            "status": "ok",
            "service": "meetinglens-docling-worker",
            "workerVersion": "0.2.0",
            "parserVersion": "2.117.0",
            "protocolVersion": "2",
            "pythonVersion": sys.version.split()[0],
            "transport": "stdio",
            "networkListener": False,
            "remoteServices": False,
            "externalPlugins": False,
            "device": "cpu",
        }:
            raise AssertionError("health contract mismatch")
        capabilities, capability_rss, capability_ports = _run_worker(
            _base_request("capabilities"), environment, working_root
        )
        if not capabilities.get("ok") or capabilities["result"]["models"] != "available":
            raise AssertionError("model-backed capabilities are unavailable")
        if not all(row["available"] for row in capabilities["result"]["formats"]):
            raise AssertionError("one or more required formats are unavailable")

        inputs = _create_inputs(input_root)
        peak_rss = max(health_rss, capability_rss)
        listening_pids = health_ports | capability_ports
        durations: dict[str, int] = {}
        for name, (path, extension, media_type, ocr) in inputs.items():
            output_root = staging_root / str(uuid.uuid4())
            output_root.mkdir()
            request = _parse_request(path, input_root, output_root, extension, media_type, ocr=ocr)
            parse_started = time.monotonic()
            response, rss, ports = _run_worker(request, environment, working_root)
            durations[name] = int((time.monotonic() - parse_started) * 1000)
            peak_rss = max(peak_rss, rss)
            listening_pids.update(ports)
            try:
                _validate_success(
                    response, output_root, [path, input_root, output_root, models_root]
                )
            except AssertionError as error:
                raise AssertionError(f"{name}: {error}") from None

        outside_path = root / "outside.txt"
        outside_path.write_text(SYNTHETIC_TEXT, encoding="utf-8")
        negative_cases: list[tuple[dict[str, Any], str]] = []
        for label in ("url", "outside", "hash"):
            output_root = staging_root / str(uuid.uuid4())
            output_root.mkdir()
            if label == "url":
                request = _parse_request(
                    inputs["txt"][0], input_root, output_root, "txt", "text/plain", ocr=False
                )
                request["inputPath"] = "https://example.invalid/private.txt"
                expected = "parser_invalid_input"
            elif label == "outside":
                request = _parse_request(
                    outside_path, input_root, output_root, "txt", "text/plain", ocr=False
                )
                expected = "parser_invalid_input"
            else:
                request = _parse_request(
                    inputs["txt"][0],
                    input_root,
                    output_root,
                    "txt",
                    "text/plain",
                    ocr=False,
                    sha256="0" * 64,
                )
                expected = "parser_integrity_failed"
            negative_cases.append((request, expected))

        missing_models = root / "missing-models"
        missing_models.mkdir()
        missing_environment = _safe_environment(working_root, missing_models, staging_root)
        output_root = staging_root / str(uuid.uuid4())
        output_root.mkdir()
        negative_cases.append(
            (
                _parse_request(
                    inputs["digital-pdf"][0],
                    input_root,
                    output_root,
                    "pdf",
                    "application/pdf",
                    ocr=False,
                ),
                "parser_models_missing",
            )
        )

        two_page_pdf = input_root / "two-pages.pdf"
        _write_pdf(two_page_pdf, [SYNTHETIC_TEXT, "Second synthetic page"])
        output_root = staging_root / str(uuid.uuid4())
        output_root.mkdir()
        negative_cases.append(
            (
                _parse_request(
                    two_page_pdf,
                    input_root,
                    output_root,
                    "pdf",
                    "application/pdf",
                    ocr=False,
                    max_pages=1,
                ),
                "parser_page_limit_exceeded",
            )
        )

        oversized_output = input_root / "oversized.txt"
        oversized_output.write_text("X" * (32 * 1024 * 1024 + 1), encoding="utf-8")
        output_root = staging_root / str(uuid.uuid4())
        output_root.mkdir()
        negative_cases.append(
            (
                _parse_request(
                    oversized_output,
                    input_root,
                    output_root,
                    "txt",
                    "text/plain",
                    ocr=False,
                ),
                "parser_output_too_large",
            )
        )

        for request, expected_code in negative_cases:
            selected_environment = (
                missing_environment if expected_code == "parser_models_missing" else environment
            )
            response, rss, ports = _run_worker(request, selected_environment, working_root)
            peak_rss = max(peak_rss, rss)
            listening_pids.update(ports)
            if response.get("ok") is not False or response["error"]["code"] != expected_code:
                raise AssertionError(
                    f"expected {expected_code}, received {response.get('error', {}).get('code')}"
                )

        if listening_pids:
            raise AssertionError("worker opened a network listener")
        worker_processes = [
            process
            for process in psutil.process_iter(["name", "cmdline"])
            if "meetinglens_docling_worker" in " ".join(process.info.get("cmdline") or [])
        ]
        if worker_processes:
            raise AssertionError("worker process remained after smoke")
        if any(staging_root.iterdir()):
            for child in staging_root.iterdir():
                if not child.is_dir():
                    raise AssertionError("unexpected parser staging entry")

        return {
            "formats": sorted(inputs),
            "ocrBackend": capabilities["result"]["ocrBackend"],
            "peakProcessTreeRssBytes": peak_rss,
            "durationMs": int((time.monotonic() - started) * 1000),
            "perFormatDurationMs": durations,
            "networkListeners": 0,
            "residualWorkers": 0,
            "temporaryDataCleanedOnExit": True,
        }


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the Windows-only Docling acceptance smoke")
    parser.add_argument("--models-root", type=Path, required=True)
    result = run(parser.parse_args().models_root)
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
