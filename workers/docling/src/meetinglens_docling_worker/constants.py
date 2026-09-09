from __future__ import annotations

from enum import StrEnum

PROTOCOL_VERSION = "2"
WORKER_NAME = "meetinglens-docling-worker"
WORKER_VERSION = "0.2.0"
DOCLING_VERSION = "2.117.0"
CHUNK_SCHEMA_VERSION = 1

MAX_PROTOCOL_MESSAGE_BYTES = 1024 * 1024
MAX_INPUT_BYTES = 100 * 1024 * 1024
MAX_PAGES = 300
DOCLING_TIMEOUT_SECONDS = 120.0
RUST_HARD_TIMEOUT_SECONDS = 150.0
MAX_CPU_THREADS = 4
MAX_JOB_MEMORY_BYTES = 4 * 1024 * 1024 * 1024
MAX_DOCUMENT_JSON_BYTES = 64 * 1024 * 1024
MAX_MARKDOWN_BYTES = 32 * 1024 * 1024
MAX_TEXT_BYTES = 32 * 1024 * 1024
MAX_CHUNKS_JSONL_BYTES = 32 * 1024 * 1024
MAX_TOTAL_ARTIFACT_BYTES = 128 * 1024 * 1024
MAX_CHUNKS = 20_000
MAX_CHUNK_TEXT_BYTES = 65_536
MAX_TOTAL_CHUNK_TEXT_BYTES = 10 * 1024 * 1024
MAX_SECTION_DEPTH = 16
MAX_SECTION_ITEM_BYTES = 255
MAX_SECTION_PATH_JSON_BYTES = 4096
MAX_TABLE_CELLS = 100_000

MODELS_MANIFEST_NAME = "meetinglens-docling-models.json"
MODELS_MANIFEST_SCHEMA_VERSION = 1
LAYOUT_REPOSITORY = "docling-project/docling-layout-heron"
LAYOUT_REVISION = "8f39ad3c0b4c58e9c2d2c84a38465abf757272d8"
TABLE_REPOSITORY = "docling-project/docling-models"
TABLE_REVISION = "fc0f2d45e2218ea24bce5045f58a389aed16dc23"
TABLE_UPSTREAM_TAG = "v2.3.0"
RAPIDOCR_UPSTREAM_RELEASE = "v3.9.0"

MODELS_ROOT_ENV = "MEETINGLENS_DOCLING_MODELS_ROOT"
PARSER_STAGING_ROOT_ENV = "MEETINGLENS_PARSER_STAGING_ROOT"


class ParserErrorCode(StrEnum):
    UNAVAILABLE = "parser_unavailable"
    ENVIRONMENT_MISSING = "parser_environment_missing"
    DEPENDENCY_INCOMPATIBLE = "parser_dependency_incompatible"
    MODELS_MISSING = "parser_models_missing"
    MODELS_INCOMPATIBLE = "parser_models_incompatible"
    INVALID_INPUT = "parser_invalid_input"
    UNSUPPORTED_TYPE = "parser_unsupported_type"
    OCR_UNAVAILABLE = "parser_ocr_unavailable"
    PROTOCOL_ERROR = "parser_protocol_error"
    WORKER_START_FAILED = "parser_worker_start_failed"
    WORKER_CRASHED = "parser_worker_crashed"
    TIMEOUT = "parser_timeout"
    CANCELLED = "parser_cancelled"
    RESOURCE_LIMIT = "parser_resource_limit"
    PAGE_LIMIT_EXCEEDED = "parser_page_limit_exceeded"
    OUTPUT_TOO_LARGE = "parser_output_too_large"
    PARTIAL_RESULT = "parser_partial_result"
    INTEGRITY_FAILED = "parser_integrity_failed"
    FAILED = "parser_failed"
    INTERNAL_ERROR = "internal_error"


SAFE_ERROR_DETAILS: dict[ParserErrorCode, tuple[str, bool]] = {
    ParserErrorCode.UNAVAILABLE: ("The local document parser is unavailable.", True),
    ParserErrorCode.ENVIRONMENT_MISSING: ("The document parser environment is missing.", False),
    ParserErrorCode.DEPENDENCY_INCOMPATIBLE: (
        "The document parser dependencies are incompatible.",
        False,
    ),
    ParserErrorCode.MODELS_MISSING: ("Required local parser models are missing.", False),
    ParserErrorCode.MODELS_INCOMPATIBLE: (
        "The local parser models are incompatible.",
        False,
    ),
    ParserErrorCode.INVALID_INPUT: ("The document parser input is invalid.", False),
    ParserErrorCode.UNSUPPORTED_TYPE: ("The document type is not supported.", False),
    ParserErrorCode.OCR_UNAVAILABLE: ("Local OCR is unavailable.", False),
    ParserErrorCode.PROTOCOL_ERROR: ("The document parser protocol is invalid.", False),
    ParserErrorCode.WORKER_START_FAILED: ("The document parser could not start.", True),
    ParserErrorCode.WORKER_CRASHED: ("The document parser stopped unexpectedly.", True),
    ParserErrorCode.TIMEOUT: ("The document parser timed out.", True),
    ParserErrorCode.CANCELLED: ("Document parsing was cancelled.", False),
    ParserErrorCode.RESOURCE_LIMIT: ("The document exceeded a parser resource limit.", False),
    ParserErrorCode.PAGE_LIMIT_EXCEEDED: ("The document exceeds the page limit.", False),
    ParserErrorCode.OUTPUT_TOO_LARGE: ("The parser output exceeds the size limit.", False),
    ParserErrorCode.PARTIAL_RESULT: ("The parser produced only a partial result.", True),
    ParserErrorCode.INTEGRITY_FAILED: ("A parser integrity check failed.", False),
    ParserErrorCode.FAILED: ("The document could not be parsed.", True),
    ParserErrorCode.INTERNAL_ERROR: ("An internal parser error occurred.", False),
}
