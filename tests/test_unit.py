import contextlib
import io
import sys
import unittest
import time
import json
import tempfile
import threading
from pathlib import Path
from unittest import mock

import runner
from runner import EvidenceTestResult, TestEvidence, mask_secret, poll_readiness

class TestRunnerUnit(unittest.TestCase):
    def test_mask_secret(self):
        """Test secret masking logic."""
        self.assertEqual(mask_secret(""), "")
        self.assertEqual(mask_secret("short"), "*****")
        self.assertEqual(mask_secret("thisisareallylongsecretstring"), "this*********************ring")

    def test_poll_readiness_success(self):
        """Test poll readiness when condition is met."""
        calls = 0
        def check_fn():
            nonlocal calls
            calls += 1
            return calls >= 3

        start = time.time()
        result = poll_readiness(check_fn, timeout_seconds=2, interval=0.1)
        end = time.time()

        self.assertTrue(result)
        self.assertEqual(calls, 3)
        self.assertLess(end - start, 1.0)

    def test_poll_readiness_timeout(self):
        """Test poll readiness when condition times out."""
        def check_fn():
            return False

        start = time.time()
        result = poll_readiness(check_fn, timeout_seconds=1, interval=0.1)
        end = time.time()

        self.assertFalse(result)
        self.assertGreaterEqual(end - start, 1.0)

    def test_evidence_export_counts_blocked_without_calling_it_pass(self):
        evidence = TestEvidence()
        evidence.add_result("integration", "db", "BLOCKED", 3, blocked_by="fixture unavailable")
        evidence.add_result("unit", "mask", "PASS", 1)
        with tempfile.TemporaryDirectory() as tmpdir:
            output = Path(tmpdir) / "evidence.json"
            evidence.export(output)
            report = json.loads(output.read_text())
        self.assertEqual(report["summary"], {
            "total": 2,
            "passed": 1,
            "failed": 0,
            "blocked": 1,
            "not_run": 0,
            "skipped": 0,
        })
        self.assertTrue(report["metadata"]["command"])
        self.assertTrue(report["metadata"]["python_version"])
        self.assertEqual(report["completion_status"], "RUNNING")
        self.assertIsNone(report["end_time"])
        self.assertEqual(report["results"][0]["status"], "BLOCKED")
        self.assertEqual(report["results"][0]["blocked_by"], "fixture unavailable")

    def test_blocked_error_is_preserved_and_secret_is_masked(self):
        evidence = TestEvidence()
        result = EvidenceTestResult(io.StringIO(), False, 0, evidence, "integration")
        token = "fp_test_sensitive_credential"

        class TestCase(unittest.TestCase):
            def runTest(self):
                pass

        case = TestCase()
        case.token = token
        error = Exception(f"BLOCKED_BY: fixture unavailable; token={token}")
        result.addError(case, (Exception, error, None))

        self.assertEqual(evidence.results[0]["status"], "BLOCKED")
        self.assertEqual(evidence.results[0]["blocked_by"], "fixture unavailable; token=" + mask_secret(token))
        self.assertNotIn(token, evidence.results[0]["details"]["error"])
        self.assertEqual(result.errors, [])
        self.assertFalse(result.wasSuccessful())

    def test_skips_and_failed_subtests_are_recorded(self):
        class SkippedCase(unittest.TestCase):
            def runTest(self):
                self.skipTest("optional browser driver not installed")

        class SubTestCase(unittest.TestCase):
            def runTest(self):
                with self.subTest(value="invalid"):
                    self.assertEqual("expected", "observed")

        evidence = TestEvidence()
        runner_instance = runner.EvidenceTestRunner(evidence, "unit", verbosity=0, stream=io.StringIO())
        result = runner_instance.run(unittest.TestSuite([SkippedCase(), SubTestCase()]))

        self.assertFalse(result.wasSuccessful())
        self.assertEqual([row["status"] for row in evidence.results], ["SKIPPED", "FAIL"])

    def test_concurrent_evidence_exports_remain_valid_json(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            output = Path(tmpdir) / "shared-evidence.json"

            def write_report(index):
                evidence = TestEvidence()
                evidence.completion_status = "complete"
                evidence.add_result("unit", f"writer-{index}", "PASS", 1)
                evidence.export(output)

            threads = [threading.Thread(target=write_report, args=(index,)) for index in range(4)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join(timeout=5)
                self.assertFalse(thread.is_alive(), "evidence writer deadlocked")

            report = json.loads(output.read_text())
            self.assertEqual(report["completion_status"], "complete")
            self.assertEqual(report["summary"]["passed"], 1)

    def test_empty_suite_exits_nonzero(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            evidence_file = Path(tmpdir) / "empty-suite.json"
            argv = ["runner.py", "--suite", "unit", "--evidence-out", str(evidence_file)]
            with mock.patch.object(sys, "argv", argv), \
                 mock.patch("runner.unittest.TestLoader.discover", return_value=unittest.TestSuite()), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(SystemExit) as exit_info:
                    runner.main()
            self.assertEqual(exit_info.exception.code, 1)
            report = json.loads(evidence_file.read_text())
            self.assertEqual(report["summary"]["not_run"], 1)
            self.assertEqual(report["results"][0]["status"], "NOT_RUN")
