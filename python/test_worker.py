from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import unittest
from pathlib import Path

from bridge_protocol import OPERATIONS, PROTOCOL_VERSION, ProtocolError, parse_response
from worker import classify_error

ROOT = Path(__file__).resolve().parents[1]
WORKER = ROOT / "python" / "worker.py"


def request(operation: str, payload: dict[str, object]) -> dict[str, object]:
    return {
        "protocol_version": PROTOCOL_VERSION,
        "operation_id": "00000000-0000-4000-8000-000000000999",
        "operation": operation,
        "submitted_at_unix_ms": 1_000,
        "deadline_unix_ms": int(time.time() * 1000) + 60_000,
        "input_sha256": None,
        "payload": payload,
    }


class WorkerProcess:
    def __init__(self) -> None:
        env = os.environ.copy()
        env["PYTHONPATH"] = str(ROOT / "python")
        env["PYTHONUNBUFFERED"] = "1"
        self.process = subprocess.Popen(
            [sys.executable, str(WORKER)],
            cwd=ROOT,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )
        assert self.process.stdout is not None
        self.handshake = json.loads(self.process.stdout.readline())

    def send(self, value: object) -> dict[str, object]:
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        line = value if isinstance(value, str) else json.dumps(value)
        self.process.stdin.write(line + "\n")
        self.process.stdin.flush()
        return json.loads(self.process.stdout.readline())

    def close(self) -> tuple[int, str]:
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        assert self.process.stderr is not None
        self.process.stdin.close()
        code = self.process.wait(timeout=10)
        stderr = self.process.stderr.read()
        self.process.stdout.close()
        self.process.stderr.close()
        return code, stderr

    def terminate(self) -> None:
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait(timeout=10)
        for stream in (self.process.stdin, self.process.stdout, self.process.stderr):
            if stream is not None and not stream.closed:
                stream.close()


class WorkerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.worker = WorkerProcess()

    def tearDown(self) -> None:
        self.worker.terminate()

    def test_handshake_and_ping_are_versioned_and_complete(self) -> None:
        handshake = self.worker.handshake
        self.assertEqual(handshake["event"], "handshake")
        self.assertEqual(handshake["protocol_version"], PROTOCOL_VERSION)
        self.assertEqual(handshake["operations"], list(OPERATIONS))
        self.assertIsInstance(handshake["worker_pid"], int)
        self.assertIn("ready", handshake)
        self.assertIn("pymupdf_version", handshake)

        response = parse_response(self.worker.send(request("ping", {})))
        self.assertEqual(response["disposition"], "succeeded")
        self.assertEqual(response["payload"]["handshake"]["worker_pid"], handshake["worker_pid"])

    def test_malformed_request_is_rejected_without_killing_worker(self) -> None:
        error = self.worker.send("{not-json")
        self.assertEqual(error["event"], "protocol_error")
        self.assertEqual(error["code"], "MALFORMED_JSON")
        response = parse_response(self.worker.send(request("ping", {})))
        self.assertEqual(response["disposition"], "succeeded")

    def test_operation_failure_is_typed_and_correlated(self) -> None:
        operation = request(
            "render_page_to_png",
            {
                "pdf_path": "fixtures/does-not-exist.pdf",
                "page_num": 0,
                "dpi": 144.0,
            },
        )
        response = parse_response(self.worker.send(operation))
        self.assertEqual(response["operation_id"], operation["operation_id"])
        self.assertEqual(response["operation"], "render_page_to_png")
        self.assertEqual(response["disposition"], "failed")
        self.assertIsNotNone(response["failure"])
        self.assertEqual(response["failure"]["code"], "INPUT_NOT_FOUND")
        self.assertEqual(response["failure"]["class"], "FileNotFoundError")
        self.assertNotIn("traceback", response["failure"]["context"])

    def test_exception_taxonomy_is_stable_and_retry_aware(self) -> None:
        cases = [
            (FileNotFoundError("missing"), "INPUT_NOT_FOUND", False),
            (PermissionError("denied"), "PERMISSION_DENIED", False),
            (TimeoutError("late"), "PYTHON_TIMEOUT", True),
            (ConnectionError("offline"), "PYTHON_CONNECTION_ERROR", True),
            (MemoryError("exhausted"), "PYTHON_MEMORY_EXHAUSTED", False),
            (ValueError("bad"), "PYTHON_INVALID_VALUE", False),
            (RuntimeError("PDF_NOT_EDITABLE: fixture"), "PDF_NOT_EDITABLE", False),
            (ProtocolError("BAD_PROTOCOL", "bad"), "BAD_PROTOCOL", False),
        ]
        for error, expected_code, retryable in cases:
            with self.subTest(expected_code=expected_code):
                failure = classify_error(error, "ping")
                self.assertEqual(failure["code"], expected_code)
                self.assertEqual(failure["retryable"], retryable)
                self.assertEqual(failure["context"], {"operation": "ping"})

    def test_eof_shuts_worker_down_cleanly(self) -> None:
        code, _stderr = self.worker.close()
        self.assertEqual(code, 0)


if __name__ == "__main__":
    unittest.main()
