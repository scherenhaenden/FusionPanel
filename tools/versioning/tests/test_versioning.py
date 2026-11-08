"""Date/hash release identities and real local Git tag validation."""
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / 'version.py'
spec = importlib.util.spec_from_file_location('fusionpanel_versioning', MODULE)
v = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = v
spec.loader.exec_module(v)
FIXED = datetime(2026, 11, 1, 14, 30, 27, 123456, tzinfo=timezone.utc)


class VersionTests(unittest.TestCase):
    def test_timestamp_and_timezone(self):
        self.assertEqual(v.timestamp(FIXED), ('2026.11.01.15.30.123', '2026-11-01T15:30:27.123+01:00', '2026-11-01T14:30:27.123+00:00'))
        self.assertEqual(v.timestamp(datetime(2026, 7, 1, 12, 0, tzinfo=timezone.utc))[0], '2026.07.01.14.00.000')
        with self.assertRaises(v.VersionError):
            v.timestamp(datetime(2026, 11, 1))

    def test_exact_format(self):
        valid = '2026.11.01.15.30.007.gabcdef123456'
        self.assertEqual(v.validate_tag_name(valid), valid)
        for invalid in ['v'+valid, '0.1.0+'+valid, '2026.13.01.15.30.007.gabcdef123456', '2026.11.01.15.30.7.gabcdef123456', '2026.11.01.15.30.007.gabcdef']:
            with self.subTest(invalid=invalid), self.assertRaises(v.VersionError):
                v.validate_tag_name(invalid)


class GitIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='fusionpanel-versioning-')
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.git('init', '-b', 'master')
        self.git('config', 'user.name', 'Version Test')
        self.git('config', 'user.email', 'version-test@example.invalid')
        self.commit('initial')

    def git(self, *args):
        return v.git(self.repo, *args)

    def commit(self, message):
        (self.repo / 'file').write_text(message)
        self.git('add', '.')
        self.git('commit', '-m', message)

    def test_exact_annotated_tag_and_verify(self):
        sha = self.git('rev-parse', 'HEAD')
        manifest = v.create_tag(self.repo, FIXED)
        expected = f'2026.11.01.15.30.123.g{sha[:12]}'
        self.assertEqual(manifest['version'], expected)
        self.assertEqual(manifest['tag'], expected)
        self.assertEqual(self.git('cat-file', '-t', f'refs/tags/{expected}'), 'tag')
        self.assertEqual(v.verify(self.repo, expected), manifest)
        with self.assertRaises(v.VersionError):
            v.create_tag(self.repo, FIXED)
        self.assertEqual(self.git('rev-parse', f'{expected}^{{commit}}'), sha)

    def test_next_commit_needs_no_semver_bump(self):
        v.create_tag(self.repo, FIXED)
        self.commit('next')
        self.assertTrue(v.create_tag(self.repo, FIXED)['version'].startswith('2026.11.01.15.30.123.g'))
        self.assertEqual(len(v.history(self.repo)), 2)

    def test_preserves_and_validates_legacy_tag(self):
        sha = self.git('rev-parse', 'HEAD')
        version = f'0.1.0-dev.1+2026.11.01.15.30.123.g{sha[:12]}'
        tag = f'v{version}'
        annotation = {'schema_version': 1, 'base_version': '0.1.0-dev.1', 'version': version, 'tag': tag, 'commit': sha, 'short_commit': sha[:12], 'branch': 'master', 'date_pattern': 'yyyy.MM.dd.HH.mm.sss', 'timezone': 'Europe/Berlin', 'created_at_local': '2026-11-01T15:30:27.123+01:00', 'created_at_utc': '2026-11-01T14:30:27.123+00:00', 'release_kind': 'prerelease'}
        self.git('tag', '-a', tag, '-m', json.dumps(annotation))
        tag_object = self.git('rev-parse', tag)
        self.assertEqual(len(v.history(self.repo)), 1)
        self.commit('new version')
        v.create_tag(self.repo, FIXED)
        self.assertEqual(self.git('rev-parse', tag), tag_object)
        self.assertEqual(len(v.history(self.repo)), 2)

    def test_bad_tag_is_rejected(self):
        sha = self.git('rev-parse', 'HEAD')
        tag = f'2026.11.01.15.30.123.g{sha[:12]}'
        self.git('tag', tag)
        with self.assertRaises(v.VersionError):
            v.history(self.repo)
        self.git('tag', '-d', tag)
        self.git('tag', '-a', tag, '-m', '{}')
        with self.assertRaises(v.VersionError):
            v.history(self.repo)

    def test_dirty_and_wrong_branch_rejected(self):
        (self.repo / 'untracked').write_text('change')
        with self.assertRaises(v.VersionError):
            v.describe(self.repo, FIXED)
        (self.repo / 'untracked').unlink()
        self.git('switch', '-c', 'feature')
        with self.assertRaises(v.VersionError):
            v.describe(self.repo, FIXED)

    def test_cli_verify_missing_and_branch_guard(self):
        result = subprocess.run([sys.executable, str(MODULE), 'verify', '--repo', str(self.repo), '--tag', '2026.11.01.15.30.123.gabcdef123456'], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not found', result.stderr)
        self.git('switch', '-c', 'feature')
        result = subprocess.run([sys.executable, str(MODULE), 'tag', '--repo', str(self.repo)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('master', result.stderr)


if __name__ == '__main__':
    unittest.main()
