from __future__ import annotations

import hashlib
import json
import unittest
from unittest.mock import patch

from docling_core.types.doc import (
    BoundingBox,
    DocItemLabel,
    DoclingDocument,
    GroupLabel,
    ProvenanceItem,
    TableCell,
    TableData,
)

from meetinglens_docling_worker.chunks import build_chunks, encode_chunks_jsonl
from meetinglens_docling_worker.constants import ParserErrorCode
from meetinglens_docling_worker.errors import WorkerError


class ChunkTests(unittest.TestCase):
    def test_builds_deterministic_order_section_page_and_table_locators(self) -> None:
        document = DoclingDocument(name="synthetic")
        document.add_title("Synthetic title")
        document.add_heading("First section", level=2)
        document.add_text(
            label=DocItemLabel.TEXT,
            text="Paragraph evidence.",
            prov=ProvenanceItem(
                page_no=2,
                bbox=BoundingBox(l=0, t=0, r=1, b=1),
                charspan=(0, 19),
            ),
        )
        list_group = document.add_group(label=GroupLabel.LIST)
        document.add_list_item("List evidence.", parent=list_group)
        document.add_table(
            TableData(
                num_rows=2,
                num_cols=2,
                table_cells=[
                    _cell(1, 1, "Beta"),
                    _cell(0, 0, "Alpha"),
                ],
            )
        )

        first = build_chunks(document)
        second = build_chunks(document)

        self.assertEqual(first, second)
        self.assertEqual(
            [chunk.kind for chunk in first],
            ["heading", "heading", "paragraph", "list_item", "table"],
        )
        self.assertEqual(first[2].locator.page_start, 2)
        self.assertEqual(first[2].locator.page_end, 2)
        self.assertEqual(
            first[2].locator.section_path,
            ("Synthetic title", "First section"),
        )
        self.assertEqual(first[4].locator.table_ordinal, 0)
        self.assertEqual(first[4].text, "R1C1: Alpha\nR2C2: Beta")
        for ordinal, chunk in enumerate(first):
            self.assertEqual(chunk.ordinal, ordinal)
            self.assertEqual(
                chunk.content_sha256,
                hashlib.sha256(chunk.text.encode("utf-8")).hexdigest(),
            )

    def test_long_unicode_text_splits_on_utf8_boundaries(self) -> None:
        document = DoclingDocument(name="synthetic")
        document.add_text(label=DocItemLabel.TEXT, text="甲乙丙丁")
        with patch("meetinglens_docling_worker.chunks.MAX_CHUNK_TEXT_BYTES", 7):
            chunks = build_chunks(document)
        self.assertEqual([chunk.text for chunk in chunks], ["甲乙", "丙丁"])
        self.assertTrue(all(len(chunk.text.encode("utf-8")) <= 7 for chunk in chunks))

    def test_total_chunk_text_limit_fails_closed(self) -> None:
        document = DoclingDocument(name="synthetic")
        document.add_text(label=DocItemLabel.TEXT, text="bounded")
        with patch("meetinglens_docling_worker.chunks.MAX_TOTAL_CHUNK_TEXT_BYTES", 3):
            with self.assertRaises(WorkerError) as context:
                build_chunks(document)
        self.assertEqual(context.exception.code, ParserErrorCode.OUTPUT_TOO_LARGE)

    def test_canonical_section_path_json_accepts_4096_bytes_and_rejects_4097(self) -> None:
        accepted = _document_with_section_path(final_name_bytes=222)
        chunks = build_chunks(accepted)
        canonical = json.dumps(
            chunks[-1].locator.section_path,
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
        self.assertEqual(len(canonical), 4096)

        rejected = _document_with_section_path(final_name_bytes=223)
        with self.assertRaises(WorkerError) as context:
            build_chunks(rejected)
        self.assertEqual(context.exception.code, ParserErrorCode.OUTPUT_TOO_LARGE)

    def test_chunk_repr_does_not_disclose_text_or_section_names(self) -> None:
        document = DoclingDocument(name="synthetic")
        document.add_heading("Private synthetic section")
        document.add_text(label=DocItemLabel.TEXT, text="Private synthetic evidence.")
        chunks = build_chunks(document)
        rendered = repr(chunks[-1]) + repr(chunks[-1].locator)
        self.assertNotIn("Private synthetic", rendered)

    def test_jsonl_has_only_the_frozen_schema_and_one_record_per_line(self) -> None:
        document = DoclingDocument(name="synthetic")
        document.add_text(label=DocItemLabel.TEXT, text="Synthetic evidence.")
        chunks = build_chunks(document)
        encoded = encode_chunks_jsonl(chunks)
        self.assertTrue(encoded.endswith(b"\n"))
        records = [json.loads(line) for line in encoded.splitlines()]
        self.assertEqual(len(records), 1)
        self.assertEqual(
            set(records[0]),
            {"schemaVersion", "ordinal", "kind", "text", "contentSha256", "locator"},
        )
        self.assertEqual(
            set(records[0]["locator"]),
            {"pageStart", "pageEnd", "sectionPath", "tableOrdinal"},
        )
        rendered = encoded.decode().lower()
        self.assertNotIn("inputpath", rendered)
        self.assertNotIn("outputpath", rendered)
        self.assertNotIn('"uri"', rendered)


def _cell(row: int, column: int, text: str) -> TableCell:
    return TableCell(
        text=text,
        start_row_offset_idx=row,
        end_row_offset_idx=row + 1,
        start_col_offset_idx=column,
        end_col_offset_idx=column + 1,
    )


def _document_with_section_path(final_name_bytes: int) -> DoclingDocument:
    document = DoclingDocument(name="synthetic")
    for level in range(1, 16):
        document.add_heading("甲" * 85, level=level)
    document.add_heading("乙" * 74 + "z" * (final_name_bytes - 222), level=16)
    return document


if __name__ == "__main__":
    unittest.main()
