#!/usr/bin/env python3
import hashlib
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

import pymupdf

MODULE_PATH = Path(__file__).with_name("pymupdf_pro_integration.py")
SPEC = importlib.util.spec_from_file_location("pymupdf_pro_integration", MODULE_PATH)
BRIDGE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BRIDGE)
BRIDGE._ensure_pro_unlocked = lambda *_args, **_kwargs: None


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def create_text_pdf(path: Path, text: str = "KEEP") -> list[float]:
    document = pymupdf.open()
    page = document.new_page(width=612, height=792)
    page.insert_text(pymupdf.Point(72, 100), text, fontname="helv", fontsize=12)
    document.save(path)
    document.close()

    document = pymupdf.open(path)
    spans = [
        span
        for block in document[0].get_text("dict").get("blocks", [])
        for line in block.get("lines", [])
        for span in line.get("spans", [])
        if text in span.get("text", "")
    ]
    document.close()
    if len(spans) != 1:
        raise AssertionError(f"expected one {text!r} span, found {len(spans)}")
    return [float(value) for value in spans[0]["bbox"]]


class ApplyManyEditsContractTests(unittest.TestCase):
    def test_no_overlap_is_non_destructive_and_not_success(self):
        with tempfile.TemporaryDirectory(prefix="apply-report-no-overlap-") as temp:
            root = Path(temp)
            source = root / "source.pdf"
            output = root / "existing-output.pdf"
            create_text_pdf(source)
            shutil.copy2(source, output)
            source_before = sha256(source)
            output_before = sha256(output)

            report = BRIDGE.apply_many_edits(
                str(source),
                str(output),
                [
                    {
                        "page": 0,
                        "rect": [300.0, 300.0, 360.0, 330.0],
                        "new_text": "REPLACEMENT",
                    }
                ],
            )

            self.assertFalse(report["success"])
            self.assertEqual(report["schema_version"], 1)
            self.assertEqual(
                (report["requested"], report["matched"], report["placed"], report["failed"]),
                (1, 0, 0, 1),
            )
            self.assertFalse(report["output_published"])
            self.assertIsNone(report["output_sha256"])
            self.assertEqual(report["source_sha256"], source_before)
            self.assertEqual(report["method_per_edit"], ["no-match"])
            self.assertEqual(len(report["edits"]), 1)
            self.assertFalse(report["edits"][0]["matched"])
            self.assertFalse(report["edits"][0]["placed"])
            self.assertEqual(sha256(source), source_before)
            self.assertEqual(sha256(output), output_before)

            document = pymupdf.open(output)
            observed = "".join(page.get_text() for page in document)
            document.close()
            self.assertIn("KEEP", observed)
            self.assertNotIn("REPLACEMENT", observed)

    def test_exact_success_has_complete_counts_hashes_and_evidence(self):
        with tempfile.TemporaryDirectory(prefix="apply-report-success-") as temp:
            root = Path(temp)
            source = root / "source.pdf"
            output = root / "output.pdf"
            bbox = create_text_pdf(source, "OLD")

            report = BRIDGE.apply_many_edits(
                str(source),
                str(output),
                [{"page": 0, "rect": bbox, "new_text": "NEW"}],
            )

            self.assertTrue(report["success"], json.dumps(report, indent=2))
            self.assertEqual(
                (report["requested"], report["matched"], report["placed"], report["failed"]),
                (1, 1, 1, 0),
            )
            self.assertTrue(report["output_published"])
            self.assertEqual(report["source_sha256"], sha256(source))
            self.assertEqual(report["output_sha256"], sha256(output))
            self.assertEqual(len(report["method_per_edit"]), 1)
            self.assertEqual(len(report["edits"]), 1)
            self.assertTrue(report["edits"][0]["matched"])
            self.assertTrue(report["edits"][0]["placed"])

            document = pymupdf.open(output)
            observed = "".join(page.get_text() for page in document)
            document.close()
            self.assertIn("NEW", observed)
            self.assertNotIn("OLD", observed)


if __name__ == "__main__":
    unittest.main(verbosity=2)
