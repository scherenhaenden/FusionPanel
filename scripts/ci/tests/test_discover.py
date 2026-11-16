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

    def git(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.root, text=True, capture_output=True, check=True
        )

    def init_git(self):
        self.git("init", "-q")
        self.git("config", "user.name", "CI Discovery Test")
        self.git("config", "user.email", "ci-discovery@example.invalid")

    def discover_changed(self, base, head):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--changed-from", base, "--changed-to", head],
            cwd=self.root,
            text=True,
            capture_output=True,
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

    def test_changed_manifests_select_only_owning_packages(self):
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/api"]\n')
        self.write("crates/api/Cargo.toml", "[package]\nname='api'\nversion='0.1.0'\n")
        self.write("crates/api/src/lib.rs", "pub fn value() -> u8 { 1 }\n")
        self.write("crates/agent/Cargo.toml", "[workspace]\n[package]\nname='agent'\nversion='0.1.0'\n")
        self.write("crates/agent/src/lib.rs", "pub fn value() -> u8 { 1 }\n")
        self.write("integration/jobs/Cargo.toml", "[workspace]\n[package]\nname='jobs'\nversion='0.1.0'\n")
        self.write("integration/jobs/tests/jobs.rs", "#[test] fn works() {}\n")
        self.write("frontend/package.json", '{"scripts":{"test":"true"}}\n')
        self.write("frontend/src/app.ts", "export const value = 1;\n")
        self.write("tests/e2e/browser/package.json", '{"scripts":{"test":"true"}}\n')
        self.write("tests/e2e/browser/tests/app.spec.js", "test('ok', () => {});\n")
        self.write(".github/workflows/ci.yml", "name: CI\n")
        self.write("docs/README.md", "baseline\n")
        self.init_git()
        self.git("add", "-A")
        self.git("commit", "-qm", "baseline")
        base = self.git("rev-parse", "HEAD").stdout.strip()

        self.write("crates/api/src/lib.rs", "pub fn value() -> u8 { 2 }\n")
        self.write("crates/agent/src/lib.rs", "pub fn value() -> u8 { 2 }\n")
        self.write("integration/jobs/tests/jobs.rs", "#[test] fn works() { assert!(true); }\n")
        self.write("frontend/src/app.ts", "export const value = 2;\n")
        self.write("tests/e2e/browser/tests/app.spec.js", "test('changed', () => {});\n")
        self.write(".github/workflows/ci.yml", "name: changed CI\n")
        self.write("docs/README.md", "changed docs\n")
        self.git("add", "-A")
        self.git("commit", "-qm", "changes")
        head = self.git("rev-parse", "HEAD").stdout.strip()

        self.assertEqual(
            self.discover_changed(base, head),
            {
                "cargo_manifests": [
                    "Cargo.toml",
                    "crates/agent/Cargo.toml",
                    "integration/jobs/Cargo.toml",
                ],
                "package_jsons": ["frontend/package.json", "tests/e2e/browser/package.json"],
            },
        )

    def test_docs_only_change_selects_no_build_manifests(self):
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/api"]\n')
        self.write("crates/api/Cargo.toml", "[package]\nname='api'\nversion='0.1.0'\n")
        self.write("docs/README.md", "baseline\n")
        self.init_git()
        self.git("add", "-A")
        self.git("commit", "-qm", "baseline")
        base = self.git("rev-parse", "HEAD").stdout.strip()
        self.write("docs/README.md", "docs only\n")
        self.git("add", "-A")
        self.git("commit", "-qm", "docs")
        head = self.git("rev-parse", "HEAD").stdout.strip()

        self.assertEqual(
            self.discover_changed(base, head),
            {"cargo_manifests": [], "package_jsons": []},
        )

    def test_root_workspace_members_flag_remains_available(self):
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\nexclude = ["crates/excluded"]\n')
        self.write("crates/a/Cargo.toml", '[package]\nname="a"\nversion="0.1.0"\n')
        self.write("crates/excluded/Cargo.toml", '[package]\nname="excluded"\nversion="0.1.0"\n')
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--root-workspace-members"],
            cwd=self.root,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["crates/a/Cargo.toml"])


if __name__ == "__main__":
    unittest.main()
