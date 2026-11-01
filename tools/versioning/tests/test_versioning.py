"""Version identity tests and actual local Git tag integration tests."""
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "version.py"
spec = importlib.util.spec_from_file_location("fusionpanel_versioning", MODULE)
v = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = v
spec.loader.exec_module(v)
FIXED = datetime(2026, 11, 1, 14, 30, 27, 123456, tzinfo=timezone.utc)


class SemVerTests(unittest.TestCase):
    def test_valid_versions_and_build_metadata(self):
        for value in ["0.1.0-dev.1", "1.0.0", "1.2.3-alpha-beta.9+2026.01.01.01.01.001.gabc123def456", "1.0.0+001"]:
            with self.subTest(value=value):
                self.assertIsInstance(v.SemVer.parse(value), v.SemVer)

    def test_invalid_versions(self):
        for value in ["v1.0.0", "01.0.0", "1.01.0", "1.0", "1.0.0-01", "1.0.0-a.01", "1.0.0-", "1.0.0+", "1.0.0-a..b", "1.0.0 x", "1.0.0-ä", "1.0.0\n"]:
            with self.subTest(value=value), self.assertRaises(v.VersionError):
                v.SemVer.parse(value)

    def test_semver_precedence_and_ignored_metadata(self):
        ordered = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0", "1.0.1", "1.1.0", "2.0.0"]
        parsed = list(map(v.SemVer.parse, ordered))
        self.assertEqual(sorted(reversed(parsed)), parsed)
        self.assertEqual(v.SemVer.parse("1.0.0+a"), v.SemVer.parse("1.0.0+b"))
        self.assertEqual(hash(v.SemVer.parse("1.0.0+a")), hash(v.SemVer.parse("1.0.0+b")))
        self.assertLess(v.SemVer.parse("0.1.0-dev.2"), v.SemVer.parse("0.1.0-dev.10"))

    def test_exact_user_date_pattern_and_full_instant(self):
        stamp, local_iso, utc_iso = v.timestamp(FIXED)
        self.assertEqual(stamp, "2026.11.01.15.30.123")
        self.assertEqual(local_iso, "2026-11-01T15:30:27.123+01:00")
        self.assertEqual(utc_iso, "2026-11-01T14:30:27.123+00:00")

    def test_dst_and_naive_time(self):
        self.assertEqual(v.timestamp(datetime(2026, 7, 1, 12, 0, tzinfo=timezone.utc))[0], "2026.07.01.14.00.000")
        with self.assertRaises(v.VersionError):
            v.timestamp(datetime(2026, 11, 1))

    def test_tag_format_validation(self):
        self.assertEqual(v.validate_tag_name("v0.1.0-dev.1+2026.11.01.15.30.007.gabcdef123456").base, "0.1.0-dev.1")
        for tag in ["0.1.0", "v0.1.0", "v0.1.0+2026.13.01.15.30.007.gabcdef123456", "v0.1.0+2026.11.01.15.30.7.gabcdef123456", "v0.1.0+2026.11.01.15.30.007.gabcdef"]:
            with self.subTest(tag=tag), self.assertRaises(v.VersionError):
                v.validate_tag_name(tag)


class GitIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="fusionpanel-versioning-")
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.git("init", "-b", "master")
        self.git("config", "user.name", "Version Test")
        self.git("config", "user.email", "version-test@example.invalid")
        self.write_version("0.1.0-dev.1")
        self.commit("Initial synthetic repository")

    def git(self, *args):
        return v.git(self.repo, *args)

    def write_version(self, version):
        (self.repo / "VERSION").write_text(version + "\n")
        (self.repo / "Cargo.toml").write_text(f'[workspace]\n[workspace.package]\nversion = "{version}"\n')

    def commit(self, message):
        self.git("add", ".")
        self.git("commit", "-m", message)

    def test_annotated_tag_targets_exact_commit_and_manifest(self):
        sha = self.git("rev-parse", "HEAD")
        result = v.create_tag(self.repo, FIXED)
        self.assertEqual(result["version"], f"0.1.0-dev.1+2026.11.01.15.30.123.g{sha[:12]}")
        self.assertEqual(self.git("cat-file", "-t", "refs/tags/" + result["tag"]), "tag")
        self.assertEqual(self.git("rev-parse", result["tag"] + "^{commit}"), sha)
        manifest = json.loads(self.git("for-each-ref", "--format=%(contents)", "refs/tags/" + result["tag"]))
        self.assertEqual(manifest["commit"], sha)
        self.assertEqual(len(v.history(self.repo)), 1)
        self.assertTrue(v.check(self.repo)["configuration_valid"])

    def test_same_semver_cannot_be_republished_or_retagged(self):
        original = v.create_tag(self.repo, FIXED)
        original_object = self.git("rev-parse", original["tag"])
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)
        (self.repo / "new.txt").write_text("New code")
        self.commit("New implementation")
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)
        self.assertEqual(self.git("rev-parse", original["tag"]), original_object)
        self.assertEqual(len(self.git("tag", "--list").splitlines()), 1)

    def test_monotonic_prerelease_bump_and_reject_downgrade(self):
        v.create_tag(self.repo, FIXED)
        self.write_version("0.1.0-dev.2")
        self.commit("Next snapshot")
        second = v.create_tag(self.repo, FIXED)
        self.assertTrue(second["tag"].startswith("v0.1.0-dev.2+"))
        self.write_version("0.1.0-dev.1")
        self.commit("Invalid downgrade")
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)
        with self.assertRaises(v.VersionError):
            v.check(self.repo)

    def test_dirty_tracked_or_untracked_files_rejected(self):
        (self.repo / "VERSION").write_text("0.1.0-dev.2\n")
        with self.assertRaises(v.VersionError):
            v.describe(self.repo, FIXED)
        self.write_version("0.1.0-dev.1")
        (self.repo / "untracked.txt").write_text("untracked")
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)

    def test_master_required_and_detached_head_rejected(self):
        self.git("switch", "-c", "feature/test")
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)
        self.git("checkout", "--detach")
        with self.assertRaises(v.VersionError):
            v.describe(self.repo, FIXED)

    def test_version_manifest_mismatch_and_metadata_rejected(self):
        (self.repo / "VERSION").write_text("0.1.0-dev.2\n")
        with self.assertRaises(v.VersionError):
            v.check(self.repo)
        self.write_version("0.1.0-dev.1+manually-authored")
        with self.assertRaises(v.VersionError):
            v.check(self.repo)

    def test_lightweight_or_hash_mismatched_history_rejected(self):
        sha = self.git("rev-parse", "HEAD")
        tag = f"v0.1.0-dev.1+2026.11.01.15.30.123.g{sha[:12]}"
        self.git("tag", tag)
        with self.assertRaises(v.VersionError):
            v.history(self.repo)
        self.git("tag", "-d", tag)
        self.git("tag", "-a", "v0.1.0-dev.1+2026.11.01.15.30.123.g000000000000", "-m", "invalid manifest")
        with self.assertRaises(v.VersionError):
            v.history(self.repo)

    def test_invalid_or_tampered_annotation_rejected(self):
        sha = self.git("rev-parse", "HEAD")
        tag = f"v0.1.0-dev.1+2026.11.01.15.30.123.g{sha[:12]}"
        self.git("tag", "-a", tag, "-m", "not JSON")
        with self.assertRaises(v.VersionError):
            v.history(self.repo)

    def test_nonobject_annotation_rejected_without_traceback(self):
        sha = self.git("rev-parse", "HEAD")
        tag = f"v0.1.0-dev.1+2026.11.01.15.30.123.g{sha[:12]}"
        self.git("tag", "-a", tag, "-m", "[]")
        with self.assertRaises(v.VersionError):
            v.history(self.repo)

    def test_version_file_has_no_padding_or_extra_lines(self):
        for raw in [" 0.1.0-dev.1\n", "0.1.0-dev.1 \n", "0.1.0-dev.1\n\n"]:
            (self.repo / "VERSION").write_text(raw)
            with self.subTest(raw=raw), self.assertRaises(v.VersionError):
                v.configured_version(self.repo)

    def test_cli_configuration_check_is_portable(self):
        result = subprocess.run([sys.executable, str(MODULE), "check", "--repo", str(self.repo)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)["configuration_valid"])
        self.git("switch", "-c", "feature/no-tag")
        result = subprocess.run([sys.executable, str(MODULE), "tag", "--repo", str(self.repo)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("master", result.stderr)
        self.assertEqual(self.git("tag", "--list"), "")


if __name__ == "__main__":
    unittest.main()
