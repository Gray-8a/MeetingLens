from __future__ import annotations

import hashlib
import json
import unicodedata
from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

from .constants import (
    CHUNK_SCHEMA_VERSION,
    MAX_CHUNK_TEXT_BYTES,
    MAX_CHUNKS,
    MAX_CHUNKS_JSONL_BYTES,
    MAX_PAGES,
    MAX_SECTION_DEPTH,
    MAX_SECTION_ITEM_BYTES,
    MAX_SECTION_PATH_JSON_BYTES,
    MAX_TABLE_CELLS,
    MAX_TOTAL_CHUNK_TEXT_BYTES,
    ParserErrorCode,
)
from .errors import WorkerError

if TYPE_CHECKING:
    from docling_core.types.doc import DoclingDocument, TableItem

type ChunkKind = Literal["heading", "paragraph", "list_item", "table"]


@dataclass(frozen=True, repr=False, slots=True)
class ChunkLocator:
    page_start: int | None
    page_end: int | None
    section_path: tuple[str, ...]
    table_ordinal: int | None

    def payload(self) -> dict[str, object]:
        return {
            "pageStart": self.page_start,
            "pageEnd": self.page_end,
            "sectionPath": list(self.section_path),
            "tableOrdinal": self.table_ordinal,
        }


@dataclass(frozen=True, repr=False, slots=True)
class NormalizedChunk:
    ordinal: int
    kind: ChunkKind
    text: str
    content_sha256: str
    locator: ChunkLocator

    def payload(self) -> dict[str, object]:
        return {
            "schemaVersion": CHUNK_SCHEMA_VERSION,
            "ordinal": self.ordinal,
            "kind": self.kind,
            "text": self.text,
            "contentSha256": self.content_sha256,
            "locator": self.locator.payload(),
        }


def build_chunks(document: DoclingDocument) -> list[NormalizedChunk]:
    from docling_core.types.doc import ListItem, SectionHeaderItem, TableItem, TextItem, TitleItem

    chunks: list[NormalizedChunk] = []
    section_stack: list[tuple[int, str]] = []
    table_ordinal = 0
    total_text_bytes = 0

    for item, _level in document.iterate_items():
        kind: ChunkKind
        text: str
        current_table_ordinal: int | None = None

        if isinstance(item, TableItem):
            text = _render_table(item)
            if not text:
                continue
            kind = "table"
            current_table_ordinal = table_ordinal
            table_ordinal += 1
        elif isinstance(item, (SectionHeaderItem, TitleItem)):
            text = _normalize_text(item.text)
            if not text:
                continue
            kind = "heading"
            raw_level = item.level if isinstance(item, SectionHeaderItem) else 1
            heading_level = min(max(raw_level, 1), MAX_SECTION_DEPTH)
            section_name = _bounded_section_name(text)
            while section_stack and section_stack[-1][0] >= heading_level:
                section_stack.pop()
            section_stack.append((heading_level, section_name))
            if len(section_stack) > MAX_SECTION_DEPTH:
                section_stack = section_stack[-MAX_SECTION_DEPTH:]
        elif isinstance(item, ListItem):
            text = _normalize_text(item.text)
            if not text:
                continue
            kind = "list_item"
        elif isinstance(item, TextItem):
            text = _normalize_text(item.text)
            if not text:
                continue
            kind = "paragraph"
        else:
            continue

        page_start, page_end = _page_range(item)
        section_path = tuple(name for _, name in section_stack)
        _validate_section_path(section_path)
        locator = ChunkLocator(
            page_start=page_start,
            page_end=page_end,
            section_path=section_path,
            table_ordinal=current_table_ordinal,
        )
        for piece in _split_utf8(text):
            piece_bytes = piece.encode("utf-8")
            total_text_bytes += len(piece_bytes)
            if total_text_bytes > MAX_TOTAL_CHUNK_TEXT_BYTES or len(chunks) >= MAX_CHUNKS:
                raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
            chunks.append(
                NormalizedChunk(
                    ordinal=len(chunks),
                    kind=kind,
                    text=piece,
                    content_sha256=hashlib.sha256(piece_bytes).hexdigest(),
                    locator=locator,
                )
            )

    if not chunks:
        raise WorkerError(ParserErrorCode.FAILED)
    return chunks


def encode_chunks_jsonl(chunks: list[NormalizedChunk]) -> bytes:
    if not chunks or len(chunks) > MAX_CHUNKS:
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
    encoded = bytearray()
    try:
        for chunk in chunks:
            line = (
                json.dumps(
                    chunk.payload(),
                    ensure_ascii=False,
                    separators=(",", ":"),
                    sort_keys=True,
                )
                + "\n"
            ).encode("utf-8")
            if len(encoded) + len(line) > MAX_CHUNKS_JSONL_BYTES:
                raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
            encoded.extend(line)
    except WorkerError:
        raise
    except (TypeError, ValueError) as error:
        raise WorkerError(ParserErrorCode.FAILED) from error
    return bytes(encoded)


def _normalize_text(value: str) -> str:
    normalized = unicodedata.normalize("NFC", value).replace("\r\n", "\n").replace("\r", "\n")
    normalized = normalized.strip()
    if any(_is_forbidden_control(character) for character in normalized):
        raise WorkerError(ParserErrorCode.INVALID_INPUT)
    return normalized


def _bounded_section_name(value: str) -> str:
    inline = " ".join(value.split())
    encoded = inline.encode("utf-8")
    if len(encoded) <= MAX_SECTION_ITEM_BYTES:
        return inline
    end = MAX_SECTION_ITEM_BYTES
    while end > 0:
        try:
            return encoded[:end].decode("utf-8")
        except UnicodeDecodeError as error:
            end = error.start
    raise WorkerError(ParserErrorCode.FAILED)


def _validate_section_path(section_path: tuple[str, ...]) -> None:
    try:
        encoded = json.dumps(
            section_path,
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise WorkerError(ParserErrorCode.FAILED) from error
    if len(encoded) > MAX_SECTION_PATH_JSON_BYTES:
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)


def _split_utf8(value: str) -> list[str]:
    encoded = value.encode("utf-8")
    pieces: list[str] = []
    offset = 0
    while offset < len(encoded):
        end = min(offset + MAX_CHUNK_TEXT_BYTES, len(encoded))
        while end > offset:
            try:
                piece = encoded[offset:end].decode("utf-8")
                break
            except UnicodeDecodeError as error:
                end = offset + error.start
        else:
            raise WorkerError(ParserErrorCode.FAILED)
        if piece.strip():
            pieces.append(piece)
        offset = end
    return pieces


def _page_range(item: object) -> tuple[int | None, int | None]:
    provenance = getattr(item, "prov", [])
    pages = sorted({entry.page_no for entry in provenance})
    if not pages:
        return None, None
    if pages[0] < 1 or pages[-1] > MAX_PAGES:
        raise WorkerError(ParserErrorCode.FAILED)
    return pages[0], pages[-1]


def _render_table(item: TableItem) -> str:
    data = item.data
    if data.num_rows < 0 or data.num_cols < 0 or data.num_rows * data.num_cols > MAX_TABLE_CELLS:
        raise WorkerError(ParserErrorCode.OUTPUT_TOO_LARGE)
    rows: list[str] = []
    cells = sorted(
        data.table_cells,
        key=lambda cell: (
            cell.start_row_offset_idx,
            cell.start_col_offset_idx,
            cell.end_row_offset_idx,
            cell.end_col_offset_idx,
            cell.text,
        ),
    )
    for cell in cells:
        if (
            cell.start_row_offset_idx < 0
            or cell.start_col_offset_idx < 0
            or cell.end_row_offset_idx <= cell.start_row_offset_idx
            or cell.end_col_offset_idx <= cell.start_col_offset_idx
            or cell.end_row_offset_idx > data.num_rows
            or cell.end_col_offset_idx > data.num_cols
        ):
            raise WorkerError(ParserErrorCode.FAILED)
        text = " ".join(_normalize_text(cell.text).split())
        if not text:
            continue
        start = f"R{cell.start_row_offset_idx + 1}C{cell.start_col_offset_idx + 1}"
        if cell.end_row_offset_idx - cell.start_row_offset_idx > 1 or (
            cell.end_col_offset_idx - cell.start_col_offset_idx > 1
        ):
            end = f"R{cell.end_row_offset_idx}C{cell.end_col_offset_idx}"
            start = f"{start}-{end}"
        rows.append(f"{start}: {text}")
    return "\n".join(rows)


def _is_forbidden_control(character: str) -> bool:
    code = ord(character)
    return (code <= 0x1F and character not in {"\n", "\t"}) or 0x7F <= code <= 0x9F
