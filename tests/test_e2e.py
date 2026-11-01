import os
import shutil
import subprocess
import socket
import tempfile
import unittest
import urllib.request
import urllib.error
from pathlib import Path
from runner import generate_isolated_credentials, mask_secret, poll_readiness

class E2EJourneys(unittest.TestCase):
    def setUp(self):
        repo_root = Path(__file__).resolve().parents[1]
        self.api_bin = os.environ.get("FUSIONPANEL_API_BIN") or shutil.which("fusionpanel-api")
        self.cli_bin = os.environ.get("FUSIONPANEL_CLI_BIN") or shutil.which("fusionpanel-cli")
        if not self.api_bin and (repo_root / "target/debug/fusionpanel-api").is_file():
            self.api_bin = str(repo_root / "target/debug/fusionpanel-api")
        if not self.cli_bin and (repo_root / "target/debug/fusionpanel-cli").is_file():
            self.cli_bin = str(repo_root / "target/debug/fusionpanel-cli")
        self.token = generate_isolated_credentials()
        self.api_process = None
        self.api_port = self._available_port()
        self.api_log = tempfile.TemporaryFile(mode="w+b")
        self.home = tempfile.TemporaryDirectory(prefix="fusionpanel-e2e-home-")
        self.evidence_details = {"expected": "real API and CLI authentication journey completes"}

    def _isolated_environment(self):
        """Preserve only runtime essentials, preventing inherited app credentials/config."""
        env = {key: os.environ[key] for key in ("PATH", "LANG", "LC_ALL", "LD_LIBRARY_PATH") if key in os.environ}
        env["HOME"] = self.home.name
        env["TMPDIR"] = self.home.name
        return env

    @staticmethod
    def _available_port():
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            return sock.getsockname()[1]

    def tearDown(self):
        if self.api_process:
            self.api_process.terminate()
            try:
                self.api_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.api_process.kill()
                self.api_process.wait(timeout=5)
            self.assertIsNotNone(self.api_process.poll(), "API process survived teardown")
            self.evidence_details["cleanup"] = {
                "api_process_reaped": self.api_process.poll() is not None,
                "api_exit_code": self.api_process.returncode,
            }
        self.api_log.seek(0)
        log_output = self.api_log.read().decode(errors="replace")
        try:
            self.assertNotIn(self.token, log_output, "generated token leaked into API logs")
        finally:
            self.api_log.close()
            self.home.cleanup()

    def test_api_cli_journey(self):
        """
        Tests the actual API-process/CLI journey.
        Starts the built API with a generated token, polls bounded readiness,
        runs the real CLI, validates JSON/exit codes, and exercises invalid authentication.
        """
        if not self.api_bin or not Path(self.api_bin).is_file() or not os.access(self.api_bin, os.X_OK):
            raise Exception("BLOCKED_BY: fusionpanel-api binary not found; build with cargo build -p fusionpanel-api")
        if not self.cli_bin or not Path(self.cli_bin).is_file() or not os.access(self.cli_bin, os.X_OK):
            raise Exception("BLOCKED_BY: fusionpanel-cli binary not found; build with cargo build -p fusionpanel-cli")

        # Start the real API process
        env = self._isolated_environment()
        env["FUSIONPANEL_ADMIN_TOKEN"] = self.token
        env["FUSIONPANEL_LISTEN"] = f"127.0.0.1:{self.api_port}"

        # Start API (we redirect to DEVNULL or capture, but E2E should capture sanitized evidence eventually)
        self.api_process = subprocess.Popen(
            [self.api_bin],
            env=env,
            stdout=self.api_log,
            stderr=subprocess.STDOUT,
        )

        # Poll readiness
        def check_readiness():
            if self.api_process.poll() is not None:
                return False
            try:
                response = urllib.request.urlopen(f"http://127.0.0.1:{self.api_port}/v1/health", timeout=1)
                with response:
                    ready = response.getcode() == 200
                    if ready:
                        self.evidence_details["api_health"] = {"status_code": response.getcode()}
                    return ready
            except urllib.error.URLError:
                return False

        is_ready = poll_readiness(check_readiness, timeout_seconds=5, interval=0.5)
        self.assertTrue(is_ready, "API failed to become ready within the timeout")

        # Run real CLI with valid token
        cli_env = self._isolated_environment()
        cli_env["FUSIONPANEL_TOKEN"] = self.token
        cli_result = subprocess.run(
            [self.cli_bin, "--server", f"http://127.0.0.1:{self.api_port}", "--json", "health"],
            env=cli_env,
            capture_output=True,
            text=True,
            timeout=10
        )
        self.assertEqual(cli_result.returncode, 0, f"CLI execution failed: {cli_result.stderr}")
        try:
            import json
            parsed = json.loads(cli_result.stdout)
            self.assertEqual(parsed["status"], "ok")
            self.assertEqual(parsed["service"], "fusionpanel-api")
            self.evidence_details["cli_health"] = {
                "exit_code": cli_result.returncode,
                "json": parsed,
            }
        except json.JSONDecodeError:
            self.fail("CLI output was not valid JSON")

        # Run CLI with invalid token
        invalid_cli_env = self._isolated_environment()
        invalid_cli_env["FUSIONPANEL_TOKEN"] = "invalid_token"
        invalid_cli_result = subprocess.run(
            [self.cli_bin, "--server", f"http://127.0.0.1:{self.api_port}", "capabilities"],
            env=invalid_cli_env,
            capture_output=True,
            text=True,
            timeout=10
        )
        self.assertNotEqual(invalid_cli_result.returncode, 0, "CLI should have failed with invalid auth")
        self.assertIn("unauthorized", invalid_cli_result.stderr.lower(), "Expected unauthorized error message")
        self.evidence_details["invalid_auth"] = {
            "exit_code": invalid_cli_result.returncode,
            "unauthorized_observed": "unauthorized" in invalid_cli_result.stderr.lower(),
        }

        valid_caps = subprocess.run(
            [self.cli_bin, "--server", f"http://127.0.0.1:{self.api_port}", "--json", "capabilities"],
            env=cli_env,
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(valid_caps.returncode, 0, valid_caps.stderr)
        self.assertIn("control-plane.health.read", valid_caps.stdout)
        self.evidence_details["capabilities"] = {
            "exit_code": valid_caps.returncode,
            "health_read_capability_observed": "control-plane.health.read" in valid_caps.stdout,
        }

        # Ensure secret masking works in evidence (token should be masked if logged)
        masked_token = mask_secret(self.token)
        self.assertNotEqual(self.token, masked_token)
