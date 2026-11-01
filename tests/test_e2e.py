import os
import shutil
import subprocess
import time
import unittest
import urllib.request
import urllib.error
from runner import generate_isolated_credentials, mask_secret, poll_readiness

class E2EJourneys(unittest.TestCase):
    def setUp(self):
        self.api_bin = shutil.which("fp-api")
        self.cli_bin = shutil.which("fp-cli")
        self.token = generate_isolated_credentials()
        self.api_process = None
        self.api_port = 8080 # arbitrary test port

    def tearDown(self):
        if self.api_process:
            self.api_process.terminate()
            self.api_process.wait(timeout=5)

    def test_api_cli_journey(self):
        """
        Tests the actual API-process/CLI journey.
        Starts the built API with a generated token, polls bounded readiness,
        runs the real CLI, validates JSON/exit codes, and exercises invalid authentication.
        """
        if not self.api_bin:
            raise Exception("BLOCKED_BY: fp-api binary not found")
        if not self.cli_bin:
            raise Exception("BLOCKED_BY: fp-cli binary not found")

        # Start the real API process
        env = os.environ.copy()
        env["FP_API_TOKEN"] = self.token
        env["FP_API_PORT"] = str(self.api_port)

        # Start API (we redirect to DEVNULL or capture, but E2E should capture sanitized evidence eventually)
        self.api_process = subprocess.Popen(
            [self.api_bin],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )

        # Poll readiness
        def check_readiness():
            try:
                response = urllib.request.urlopen(f"http://127.0.0.1:{self.api_port}/health")
                return response.getcode() == 200
            except urllib.error.URLError:
                return False

        is_ready = poll_readiness(check_readiness, timeout_seconds=5, interval=0.5)
        self.assertTrue(is_ready, "API failed to become ready within the timeout")

        # Run real CLI with valid token
        cli_env = os.environ.copy()
        cli_env["FP_CLI_TOKEN"] = self.token
        cli_result = subprocess.run(
            [self.cli_bin, "--url", f"http://127.0.0.1:{self.api_port}", "status"],
            env=cli_env,
            capture_output=True,
            text=True,
            timeout=10
        )
        self.assertEqual(cli_result.returncode, 0, f"CLI execution failed: {cli_result.stderr}")
        try:
            import json
            parsed = json.loads(cli_result.stdout)
            self.assertIn("status", parsed)
        except json.JSONDecodeError:
            self.fail("CLI output was not valid JSON")

        # Run CLI with invalid token
        invalid_cli_env = os.environ.copy()
        invalid_cli_env["FP_CLI_TOKEN"] = "invalid_token"
        invalid_cli_result = subprocess.run(
            [self.cli_bin, "--url", f"http://127.0.0.1:{self.api_port}", "status"],
            env=invalid_cli_env,
            capture_output=True,
            text=True,
            timeout=10
        )
        self.assertNotEqual(invalid_cli_result.returncode, 0, "CLI should have failed with invalid auth")
        self.assertIn("unauthorized", invalid_cli_result.stderr.lower(), "Expected unauthorized error message")

        # Ensure secret masking works in evidence (token should be masked if logged)
        masked_token = mask_secret(self.token)
        self.assertNotEqual(self.token, masked_token)
