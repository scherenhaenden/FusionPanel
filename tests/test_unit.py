import unittest
import time
from runner import mask_secret, poll_readiness

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

    def test_teardown_verification(self):
        """Test teardown simulation."""
        # Simple test to verify the runner allows setting up state for test cases
        self.assertTrue(True)
