import argparse
import json
import logging
import os
import secrets
import subprocess
import sys
import time
import unittest
import uuid
from datetime import datetime, timezone
from pathlib import Path


def mask_secret(secret: str) -> str:
    if not secret:
        return ""
    if len(secret) <= 8:
        return "*" * len(secret)
    return secret[:4] + "*" * (len(secret) - 8) + secret[-4:]


class TestEvidence:
    def __init__(self):
        self.results = []
        self.start_time = datetime.now(timezone.utc)

    def add_result(self, suite, name, status, duration_ms, details=None, blocked_by=None):
        self.results.append({
            "suite": suite,
            "name": name,
            "status": status,  # PASS, FAIL, BLOCKED, NOT_RUN
            "duration_ms": duration_ms,
            "details": details or {},
            "blocked_by": blocked_by,
            "timestamp": datetime.now(timezone.utc).isoformat()
        })

    def export(self, filepath):
        report = {
            "start_time": self.start_time.isoformat(),
            "end_time": datetime.now(timezone.utc).isoformat(),
            "results": self.results,
            "summary": {
                "total": len(self.results),
                "passed": sum(1 for r in self.results if r["status"] == "PASS"),
                "failed": sum(1 for r in self.results if r["status"] == "FAIL"),
                "blocked": sum(1 for r in self.results if r["status"] == "BLOCKED"),
            }
        }
        with open(filepath, "w") as f:
            json.dump(report, f, indent=2)


def generate_isolated_credentials():
    """Generates an isolated token for a test run."""
    return f"fp_test_{secrets.token_hex(16)}"


def poll_readiness(check_fn, timeout_seconds=10, interval=1):
    """Polls a readiness check until it succeeds or times out."""
    start = time.time()
    while time.time() - start < timeout_seconds:
        if check_fn():
            return True
        time.sleep(interval)
    return False


class FusionPanelTestCase(unittest.TestCase):
    evidence = None
    suite_name = "unknown"

    def setUp(self):
        self.start_time = time.time()
        self.token = generate_isolated_credentials()

    def tearDown(self):
        duration_ms = int((time.time() - self.start_time) * 1000)
        # We record the result in a custom way using test attributes if set, else assume NOT_RUN unless explicitly handled
        # But standard unittest runner does not make it easy. We will just hook into the runner.


class EvidenceTestResult(unittest.TextTestResult):
    def __init__(self, stream, descriptions, verbosity, evidence, suite_name):
        super().__init__(stream, descriptions, verbosity)
        self.evidence = evidence
        self.suite_name = suite_name
        self.test_start_times = {}

    def startTest(self, test):
        super().startTest(test)
        self.test_start_times[test] = time.time()

    def addSuccess(self, test):
        super().addSuccess(test)
        duration_ms = int((time.time() - self.test_start_times[test]) * 1000)
        self.evidence.add_result(self.suite_name, str(test), "PASS", duration_ms)

    def addError(self, test, err):
        super().addError(test, err)
        self._add_failure_or_error(test, err)

    def addFailure(self, test, err):
        super().addFailure(test, err)
        self._add_failure_or_error(test, err)

    def _add_failure_or_error(self, test, err):
        duration_ms = int((time.time() - self.test_start_times.get(test, time.time())) * 1000)

        # Check if the exception message contains our BLOCKED signal
        exc_type, exc_value, exc_traceback = err
        msg = str(exc_value)

        # We try to find the token in the test object to mask it
        token = getattr(test, "token", None)
        if token:
            msg = msg.replace(token, mask_secret(token))

        if "BLOCKED_BY:" in msg:
            blocked_by = msg.split("BLOCKED_BY:")[1].strip()
            self.evidence.add_result(self.suite_name, str(test), "BLOCKED", duration_ms, details={"error": msg}, blocked_by=blocked_by)
        else:
            self.evidence.add_result(self.suite_name, str(test), "FAIL", duration_ms, details={"error": msg})

class EvidenceTestRunner(unittest.TextTestRunner):
    def __init__(self, evidence, suite_name, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.evidence = evidence
        self.suite_name = suite_name

    def _makeResult(self):
        return EvidenceTestResult(self.stream, self.descriptions, self.verbosity, self.evidence, self.suite_name)


def main():
    parser = argparse.ArgumentParser(description="FusionPanel Test Runner")
    parser.add_argument("--suite", choices=["unit", "integration", "e2e", "all"], default="all", help="Test suite to run")
    parser.add_argument("--evidence-out", default="tests/test_evidence.json", help="Path to write JSON evidence")
    args = parser.parse_args()

    evidence = TestEvidence()

    # Setup sys.path to discover tests
    sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

    suites_to_run = ["unit", "integration", "e2e"] if args.suite == "all" else [args.suite]

    success = True

    for suite_name in suites_to_run:
        print(f"--- Running suite: {suite_name} ---")
        loader = unittest.TestLoader()

        # For simplicity in this example, we expect files like test_e2e.py, test_unit.py, etc.
        # We will use pattern matching based on the suite name.
        if suite_name == "e2e":
            suite = loader.discover(start_dir=os.path.dirname(__file__), pattern="test_e2e.py")
        elif suite_name == "unit":
            suite = loader.discover(start_dir=os.path.dirname(__file__), pattern="test_unit*.py")
        elif suite_name == "integration":
            suite = loader.discover(start_dir=os.path.dirname(__file__), pattern="test_integration*.py")
        else:
            suite = unittest.TestSuite()

        if suite.countTestCases() == 0:
            print(f"No tests found for suite {suite_name}")
            continue

        runner = EvidenceTestRunner(evidence, suite_name, verbosity=2)
        result = runner.run(suite)
        if not result.wasSuccessful():
            success = False

    evidence.export(args.evidence_out)
    print(f"Evidence exported to {args.evidence_out}")

    if not success:
        sys.exit(1)


if __name__ == "__main__":
    main()
