from __future__ import annotations

import importlib.metadata
import os
import time
from pathlib import Path
from typing import Any

from .artifacts import ArtifactPayloads, write_artifacts
from .constants import (
    DOCLING_VERSION,
    MAX_CPU_THREADS,
    MAX_INPUT_BYTES,
    MAX_PAGES,
    MODELS_ROOT_ENV,
    PROTOCOL_VERSION,
    WORKER_NAME,
    WORKER_VERSION,
    ParserErrorCode,
)
from .errors import WorkerError
from .models import models_status, require_models
from .protocol import ParseRequest
from .security import ValidatedPaths, validate_parse_paths

_MODEL_FORMATS = {"pdf", "png", "jpg", "jpeg"}
_SIMPLE_FORMATS = {"txt", "md", "csv", "docx", "pptx", "xlsx"}


def health() -> dict[str, object]:
    _verify_docling_version()
    return {
        "status": "ok",
        "service": WORKER_NAME,
        "workerVersion": WORKER_VERSION,
        "parserVersion": DOCLING_VERSION,
        "protocolVersion": PROTOCOL_VERSION,
        "pythonVersion": (
            f"{os.sys.version_info.major}.{os.sys.version_info.minor}.{os.sys.version_info.micro}"
        ),
        "transport": "stdio",
        "networkListener": False,
        "remoteServices": False,
        "externalPlugins": False,
        "device": "cpu",
    }


def capabilities() -> dict[str, object]:
    _verify_docling_version()
    models_root = _models_root(required=False)
    models_present, models_compatible, _ = (
        models_status(models_root) if models_root is not None else (False, False, None)
    )
    model_available = models_present and models_compatible
    rows = []
    for extension in ("pdf", "docx", "pptx", "xlsx", "txt", "md", "csv", "png", "jpg", "jpeg"):
        requires_models = extension in _MODEL_FORMATS
        is_image = extension in {"png", "jpg", "jpeg"}
        limitations = []
        if extension == "pdf":
            limitations.append("Digital PDF requires pinned local layout and table models.")
            limitations.append("Scanned PDF additionally requires local OCR artifacts.")
        elif is_image:
            limitations.append("Image parsing requires pinned local layout and OCR artifacts.")
        elif extension == "txt":
            limitations.append("Text input must be valid UTF-8.")
        else:
            limitations.append("Macros, external links, and embedded programs are not executed.")
        rows.append(
            {
                "format": extension,
                "supported": True,
                "available": model_available if requires_models else True,
                "requiresModels": requires_models,
                "ocrAvailable": (
                    model_available if extension in {"pdf", "png", "jpg", "jpeg"} else False
                ),
                "limitations": limitations,
            }
        )
    model_state = (
        "available" if model_available else "incompatible" if models_present else "missing"
    )
    return {
        "formats": rows,
        "models": model_state,
        "ocrBackend": "rapidocr-onnxruntime" if model_available else None,
        "limits": {
            "maxInputBytes": MAX_INPUT_BYTES,
            "maxPages": MAX_PAGES,
            "maxCpuThreads": MAX_CPU_THREADS,
        },
    }


def parse(request: ParseRequest) -> dict[str, object]:
    _verify_docling_version()
    paths = validate_parse_paths(request)
    models_root = _models_root(required=request.safe_extension in _MODEL_FORMATS)
    if request.safe_extension in _MODEL_FORMATS:
        if models_root is None:
            raise WorkerError(ParserErrorCode.MODELS_MISSING)
        require_models(models_root)
    if request.safe_extension in {"png", "jpg", "jpeg"} and not request.parser_options.ocr:
        raise WorkerError(ParserErrorCode.OCR_UNAVAILABLE)

    started = time.monotonic()
    payloads = _convert(request, paths, models_root)
    duration_ms = max(0, int((time.monotonic() - started) * 1000))
    return write_artifacts(
        output_root=paths.output_root,
        request_id=request.request_id,
        document_id=request.document_id,
        input_sha256=request.expected_sha256,
        detected_media_type=request.detected_media_type,
        duration_ms=duration_ms,
        payloads=payloads,
    )


def _convert(
    request: ParseRequest,
    paths: ValidatedPaths,
    models_root: Path | None,
) -> ArtifactPayloads:
    if request.safe_extension not in _MODEL_FORMATS | _SIMPLE_FORMATS:
        raise WorkerError(ParserErrorCode.UNSUPPORTED_TYPE)
    _preflight_page_limit(
        paths.input_path, request.safe_extension, request.parser_options.max_pages
    )
    if request.safe_extension == "txt":
        return _convert_text(paths.input_path, request.document_id)

    try:
        from docling.datamodel.accelerator_options import AcceleratorDevice, AcceleratorOptions
        from docling.datamodel.base_models import ConversionStatus, InputFormat
        from docling.datamodel.pipeline_options import (
            ConvertPipelineOptions,
            PdfPipelineOptions,
            RapidOcrOptions,
        )
        from docling.document_converter import (
            CsvFormatOption,
            DocumentConverter,
            ExcelFormatOption,
            ImageFormatOption,
            MarkdownFormatOption,
            PdfFormatOption,
            PowerpointFormatOption,
            WordFormatOption,
        )

        accelerator = AcceleratorOptions(
            num_threads=MAX_CPU_THREADS,
            device=AcceleratorDevice.CPU,
            _env_file=None,
        )
        base_options = ConvertPipelineOptions(
            document_timeout=request.parser_options.document_timeout_seconds,
            accelerator_options=accelerator,
            enable_remote_services=False,
            allow_external_plugins=False,
            artifacts_path=models_root,
            do_picture_classification=False,
            do_picture_description=False,
            do_chart_extraction=False,
        )
        format_map: dict[str, tuple[Any, Any]] = {
            "md": (InputFormat.MD, MarkdownFormatOption(pipeline_options=base_options)),
            "csv": (InputFormat.CSV, CsvFormatOption(pipeline_options=base_options)),
            "docx": (InputFormat.DOCX, WordFormatOption(pipeline_options=base_options)),
            "pptx": (InputFormat.PPTX, PowerpointFormatOption(pipeline_options=base_options)),
            "xlsx": (InputFormat.XLSX, ExcelFormatOption(pipeline_options=base_options)),
        }
        if request.safe_extension in _MODEL_FORMATS:
            assert models_root is not None
            pdf_options = PdfPipelineOptions(
                document_timeout=request.parser_options.document_timeout_seconds,
                accelerator_options=accelerator,
                enable_remote_services=False,
                allow_external_plugins=False,
                artifacts_path=models_root,
                do_picture_classification=False,
                do_picture_description=False,
                do_chart_extraction=False,
                generate_page_images=False,
                generate_picture_images=False,
                generate_table_images=False,
                generate_parsed_pages=False,
                do_table_structure=True,
                do_ocr=request.parser_options.ocr,
                ocr_options=RapidOcrOptions(
                    backend="onnxruntime",
                    lang=["chinese"],
                    print_verbose=False,
                ),
                do_code_enrichment=False,
                do_formula_enrichment=False,
            )
            if request.safe_extension == "pdf":
                selected_format = InputFormat.PDF
                selected_option = PdfFormatOption(pipeline_options=pdf_options)
            else:
                selected_format = InputFormat.IMAGE
                selected_option = ImageFormatOption(pipeline_options=pdf_options)
        else:
            selected_format, selected_option = format_map[request.safe_extension]

        converter = DocumentConverter(
            allowed_formats=[selected_format],
            format_options={selected_format: selected_option},
        )
        result = converter.convert(
            paths.input_path,
            raises_on_error=False,
            max_num_pages=request.parser_options.max_pages,
            max_file_size=MAX_INPUT_BYTES,
        )
        if result.status == ConversionStatus.PARTIAL_SUCCESS or result.has_timeout_errors():
            raise WorkerError(ParserErrorCode.PARTIAL_RESULT)
        if result.status != ConversionStatus.SUCCESS:
            if _has_page_limit_error(result):
                raise WorkerError(ParserErrorCode.PAGE_LIMIT_EXCEEDED)
            raise WorkerError(ParserErrorCode.FAILED)
        document = result.document
        page_count = len(document.pages) if document.pages else None
        if page_count is not None and page_count > request.parser_options.max_pages:
            raise WorkerError(ParserErrorCode.PAGE_LIMIT_EXCEEDED)
        document_dict = _remove_unsafe_origin_fields(document.export_to_dict())
        markdown = document.export_to_markdown(
            escape_html=True,
            image_placeholder="<!-- image -->",
        )
        text = document.export_to_text()
        if not text.strip() and not markdown.strip():
            raise WorkerError(ParserErrorCode.FAILED)
        return ArtifactPayloads(
            document=document_dict,
            markdown=markdown,
            text=text,
            page_count=page_count,
            has_ocr=request.parser_options.ocr,
            warnings=[],
        )
    except WorkerError:
        raise
    except MemoryError as error:
        raise WorkerError(ParserErrorCode.RESOURCE_LIMIT) from error
    except Exception as error:
        raise WorkerError(ParserErrorCode.FAILED) from error


def _convert_text(path: Path, document_id: str) -> ArtifactPayloads:
    try:
        content = path.read_text(encoding="utf-8", errors="strict")
    except (OSError, UnicodeDecodeError) as error:
        raise WorkerError(ParserErrorCode.INVALID_INPUT) from error
    if not content.strip():
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    try:
        from docling_core.types.doc import DocItemLabel, DoclingDocument

        document = DoclingDocument(name=document_id)
        document.add_text(label=DocItemLabel.TEXT, text=content)
        return ArtifactPayloads(
            document=document.export_to_dict(),
            markdown=document.export_to_markdown(escape_html=True),
            text=document.export_to_text(),
            page_count=None,
            has_ocr=False,
            warnings=["Plain UTF-8 text was normalized into the Docling document schema."],
        )
    except Exception as error:
        raise WorkerError(ParserErrorCode.FAILED) from error


def _remove_unsafe_origin_fields(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _remove_unsafe_origin_fields(item)
            for key, item in value.items()
            if key.lower() not in {"filename", "uri", "path"}
        }
    if isinstance(value, list):
        return [_remove_unsafe_origin_fields(item) for item in value]
    return value


def _has_page_limit_error(result: Any) -> bool:
    return any(
        "page" in str(getattr(error, "error_message", "")).lower() for error in result.errors
    )


def _preflight_page_limit(path: Path, extension: str, max_pages: int) -> None:
    if extension != "pdf":
        return
    try:
        from pypdfium2 import PdfDocument

        with PdfDocument(path) as document:
            if len(document) > max_pages:
                raise WorkerError(ParserErrorCode.PAGE_LIMIT_EXCEEDED)
    except WorkerError:
        raise
    except Exception as error:
        raise WorkerError(ParserErrorCode.INVALID_INPUT) from error


def _verify_docling_version() -> None:
    try:
        version = importlib.metadata.version("docling-slim")
    except importlib.metadata.PackageNotFoundError as error:
        raise WorkerError(ParserErrorCode.ENVIRONMENT_MISSING) from error
    if version != DOCLING_VERSION:
        raise WorkerError(ParserErrorCode.DEPENDENCY_INCOMPATIBLE)


def _models_root(*, required: bool) -> Path | None:
    value = os.environ.get(MODELS_ROOT_ENV)
    if not value:
        if required:
            raise WorkerError(ParserErrorCode.MODELS_MISSING)
        return None
    try:
        root = Path(value).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        if required:
            raise WorkerError(ParserErrorCode.MODELS_MISSING) from error
        return None
    if not root.is_dir():
        if required:
            raise WorkerError(ParserErrorCode.MODELS_MISSING)
        return None
    return root
