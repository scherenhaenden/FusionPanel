import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "discover.py"


class DiscoverTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def write(self, path, contents=""):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents)

    def discover(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT)], cwd=self.root, text=True, capture_output=True,
            env={**os.environ, "GITHUB_OUTPUT": ""},
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return {key: json.loads(value) for key, value in (line.split("=", 1) for line in result.stdout.splitlines())}

    def test_root_workspace_members_are_deduplicated_but_still_own_tests(self):
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\nexclude = ["crates/excluded"]\n')
        for member in ("a", "b", "excluded"):
            self.write(f"crates/{member}/Cargo.toml", '[package]\nname = "fixture"\nversion = "0.1.0"\n')
        self.write("crates/a/tests/test_feature.rs")
        self.write("standalone/Cargo.toml", '[workspace]\n')
        self.write("frontend/package.json", '{}')

        result = self.discover()
        self.assertEqual(result["cargo_manifests"], ["Cargo.toml", "crates/excluded/Cargo.toml", "standalone/Cargo.toml"])
        self.assertEqual(result["package_jsons"], ["frontend/package.json"])

    def test_root_package_keeps_other_manifests(self):
        self.write("Cargo.toml", '[package]\nname = "root"\nversion = "0.1.0"\n')
        self.write("other/Cargo.toml", '[workspace]\n')
        self.assertEqual(self.discover()["cargo_manifests"], ["Cargo.toml", "other/Cargo.toml"])

    def test_orphan_tests_remain_errors(self):
        self.write("Cargo.toml", '[workspace]\nmembers = []\n')
        self.write("unowned/test_feature.rs")
        result = subprocess.run(
            [sys.executable, str(SCRIPT)], cwd=self.root, text=True, capture_output=True,
            env={**os.environ, "GITHUB_OUTPUT": ""},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Orphan tests detected", result.stdout)

    def test_invalid_root_manifest_fails_closed(self):
        self.write("Cargo.toml", '[workspace\n')
        result = subprocess.run(
            [sys.executable, str(SCRIPT)], cwd=self.root, text=True, capture_output=True,
            env={**os.environ, "GITHUB_OUTPUT": ""},
        )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
