import argparse
import fcntl
import json
import os
import platform
import secrets
import shutil
import subprocess
import sys
import tempfile
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
        self.run_id = str(uuid.uuid4())
        self.completion_status = "RUNNING"
        self.metadata = {
            "command": [sys.executable, *sys.argv],
            "python_version": platform.python_version(),
            "platform": sys.platform,
            "git_commit": self._git_commit(),
            "completed_suites": [],
        }

    @staticmethod
    def _git_commit():
        try:
            result = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=Path(__file__).resolve().parents[1],
                capture_output=True,
                text=True,
                timeout=5,
            )
            return result.stdout.strip() if result.returncode == 0 else None
        except (OSError, subprocess.TimeoutExpired):
            return None

    def add_result(self, suite, name, status, duration_ms, details=None, blocked_by=None):
        self.results.append({
            "suite": suite,
            "name": name,
            "status": status,  # PASS, FAIL, BLOCKED, NOT_RUN, SKIPPED
            "duration_ms": duration_ms,
            "details": details or {},
            "blocked_by": blocked_by,
            "timestamp": datetime.now(timezone.utc).isoformat()
        })

    def export(self, filepath):
        filepath = Path(filepath)
        filepath.parent.mkdir(parents=True, exist_ok=True)
        report = {
            "run_id": self.run_id,
            "start_time": self.start_time.isoformat(),
            "end_time": datetime.now(timezone.utc).isoformat() if self.completion_status != "RUNNING" else None,
            "completion_status": self.completion_status,
            "metadata": self.metadata,
            "results": self.results,
            "summary": {
                "total": len(self.results),
                "passed": sum(1 for r in self.results if r["status"] == "PASS"),
                "failed": sum(1 for r in self.results if r["status"] == "FAIL"),
                "blocked": sum(1 for r in self.results if r["status"] == "BLOCKED"),
                "not_run": sum(1 for r in self.results if r["status"] == "NOT_RUN"),
                "skipped": sum(1 for r in self.results if r["status"] == "SKIPPED"),
            }
        }
        lock_path = filepath.with_name(filepath.name + ".lock")
        with open(lock_path, "a", encoding="utf-8") as lock_file:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
            temporary_path = None
            try:
                with tempfile.NamedTemporaryFile(
                    mode="w",
                    encoding="utf-8",
                    dir=filepath.parent,
                    prefix=f".{filepath.name}.",
                    suffix=".tmp",
                    delete=False,
                ) as output:
                    temporary_path = Path(output.name)
                    json.dump(report, output, indent=2)
                    output.flush()
                    os.fsync(output.fileno())
                os.replace(temporary_path, filepath)
                directory_fd = os.open(filepath.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
                try:
                    os.fsync(directory_fd)
                finally:
                    os.close(directory_fd)
            finally:
                if temporary_path and temporary_path.exists():
                    temporary_path.unlink()
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)


def capture_fixture_metadata(project):
    """Capture only non-secret fixture image and runtime versions for evidence."""
    docker = shutil.which("docker")
    if not docker:
        return {"metadata_capture": "blocked", "reason": "Docker CLI is not installed"}

    compose_file = Path(__file__).resolve().parent / "fixtures" / "docker-compose.yml"
    prefix = [docker, "compose", "-p", project, "-f", str(compose_file)]
    try:
        config = subprocess.run(
            [*prefix, "config", "--format", "json"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        compose_version = subprocess.run(
            [docker, "compose", "version", "--short"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        engine_version = subprocess.run(
            [docker, "version", "--format", "{{.Server.Version}}"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if config.returncode:
            return {"metadata_capture": "blocked", "reason": "Docker Compose configuration could not be resolved"}
        services = json.loads(config.stdout).get("services", {})
        return {
            "metadata_capture": "complete",
            "project": project,
            "compose_version": compose_version.stdout.strip() if compose_version.returncode == 0 else None,
            "docker_engine_version": engine_version.stdout.strip() if engine_version.returncode == 0 else None,
            "images": {name: service.get("image") for name, service in sorted(services.items())},
        }
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError):
        return {"metadata_capture": "blocked", "reason": "Docker fixture metadata could not be captured"}


def generate_isolated_credentials():
    """Generates an isolated token for a test run."""
    return f"fp_test_{secrets.token_hex(16)}"


def poll_readiness(check_fn, timeout_seconds=10, interval=1):
    """Polls a readiness check until it succeeds or times out."""
    start = time.monotonic()
    while time.monotonic() - start < timeout_seconds:
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
        self.blocked_count = 0

    def startTest(self, test):
        super().startTest(test)
        self.test_start_times[test] = time.monotonic()

    def addSuccess(self, test):
        super().addSuccess(test)
        duration_ms = int((time.monotonic() - self.test_start_times[test]) * 1000)
        details = getattr(test, "evidence_details", {})
        self.evidence.add_result(self.suite_name, str(test), "PASS", duration_ms, details=details)

    def addError(self, test, err):
        if self._add_failure_or_error(test, err):
            self.blocked_count += 1
            if self.showAll:
                self.stream.writeln("BLOCKED")
            return
        super().addError(test, err)

    def addFailure(self, test, err):
        if self._add_failure_or_error(test, err):
            self.blocked_count += 1
            if self.showAll:
                self.stream.writeln("BLOCKED")
            return
        super().addFailure(test, err)

    def addSkip(self, test, reason):
        super().addSkip(test, reason)
        duration_ms = int((time.monotonic() - self.test_start_times.get(test, time.monotonic())) * 1000)
        self.evidence.add_result(self.suite_name, str(test), "SKIPPED", duration_ms, details={"reason": reason})

    def addSubTest(self, test, subtest, err):
        if err is None:
            super().addSubTest(test, subtest, err)
            return
        if self._add_failure_or_error(test, err, name=f"{test} [subtest: {subtest._subDescription()}]"):
            self.blocked_count += 1
            if self.showAll:
                self.stream.writeln("BLOCKED")
            return
        super().addSubTest(test, subtest, err)

    def wasSuccessful(self):
        return super().wasSuccessful() and self.blocked_count == 0

    def _add_failure_or_error(self, test, err, name=None):
        duration_ms = int((time.monotonic() - self.test_start_times.get(test, time.monotonic())) * 1000)

        # Check if the exception message contains our BLOCKED signal
        exc_type, exc_value, exc_traceback = err
        msg = str(exc_value)

        # We try to find the token in the test object to mask it
        token = getattr(test, "token", None)
        if token:
            msg = msg.replace(token, mask_secret(token))

        if "BLOCKED_BY:" in msg:
            blocked_by = msg.split("BLOCKED_BY:")[1].strip()
            self.evidence.add_result(self.suite_name, name or str(test), "BLOCKED", duration_ms, details={"error": msg}, blocked_by=blocked_by)
            return True
        else:
            self.evidence.add_result(self.suite_name, name or str(test), "FAIL", duration_ms, details={"error": msg})
            return False

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
    if "integration" in suites_to_run:
        evidence.metadata["integration_fixtures"] = capture_fixture_metadata(
            os.environ.get("FP_TEST_COMPOSE_PROJECT", "fusionpanel-test")
        )

    evidence_path = Path(args.evidence_out)
    evidence.metadata["evidence_path"] = str(evidence_path.resolve())
    evidence.export(evidence_path)  # Mark this run RUNNING before any work starts.

    success = True
    found_tests = False

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
            evidence.add_result(suite_name, "suite discovery", "NOT_RUN", 0, details={"reason": "no tests discovered"})
            success = False
            evidence.export(evidence_path)
            continue

        found_tests = True
        runner = EvidenceTestRunner(evidence, suite_name, verbosity=2)
        result = runner.run(suite)
        if not result.wasSuccessful():
            success = False
        evidence.metadata["completed_suites"].append(suite_name)
        if any(item["status"] in {"BLOCKED", "NOT_RUN", "SKIPPED"} for item in evidence.results):
            success = False
        evidence.export(evidence_path)

    evidence.completion_status = "complete" if success and found_tests else "failed"
    evidence.export(evidence_path)
    print(f"Evidence exported to {args.evidence_out}")

    if not success or not found_tests:
        sys.exit(1)


if __name__ == "__main__":
    main()
