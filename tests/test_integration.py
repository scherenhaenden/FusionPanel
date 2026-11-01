import os
import unittest
import urllib.request
import urllib.error
import subprocess

class TestIntegrationFixtures(unittest.TestCase):
    def test_nginx_fixture(self):
        """Test if Nginx fixture is accessible or blocked."""
        # Typically integration tests check the environment variable or try connecting
        # Since we run inside standard CI without docker composed already up for the python tests by default,
        # we will use the test to document BLOCKED if it fails to connect, as per requirements.
        try:
            response = urllib.request.urlopen("http://localhost:80", timeout=2)
            self.assertEqual(response.getcode(), 200)
        except urllib.error.URLError:
            raise Exception("BLOCKED_BY: Nginx fixture is not available (run docker-compose)")

    def test_postgres_fixture(self):
        """Test if Postgres fixture is accessible."""
        import shutil
        # Just use pg_isready if available to check, else block.
        # This exercises the real dependency or explicitly blocks.
        pg_isready = shutil.which("pg_isready")
        if not pg_isready:
            raise Exception("BLOCKED_BY: pg_isready binary not found for postgres check")

        result = subprocess.run(
            [pg_isready, "-h", "localhost", "-p", "5432", "-U", "fp_user"],
            capture_output=True,
            text=True,
            timeout=10
        )
        if result.returncode != 0:
            raise Exception("BLOCKED_BY: Postgres fixture is not available (run docker-compose)")
        self.assertIn("accepting connections", result.stdout)

    def test_mariadb_fixture(self):
        """Test if MariaDB fixture is accessible."""
        import shutil
        mysqladmin = shutil.which("mysqladmin")
        if not mysqladmin:
            raise Exception("BLOCKED_BY: mysqladmin binary not found for mariadb check")

        import os
        env = os.environ.copy()
        # Avoid putting the password in argv
        env["MYSQL_PWD"] = "fp_password"
        result = subprocess.run(
            [mysqladmin, "-h", "127.0.0.1", "-P", "3306", "-u", "fp_user", "ping"],
            env=env,
            capture_output=True,
            text=True,
            timeout=10
        )
        if result.returncode != 0:
            raise Exception("BLOCKED_BY: MariaDB fixture is not available (run docker-compose)")
        self.assertIn("mysqld is alive", result.stdout)

    def test_minio_fixture(self):
        """Test if Minio fixture is accessible."""
        try:
            response = urllib.request.urlopen("http://localhost:9000/minio/health/live", timeout=2)
            self.assertEqual(response.getcode(), 200)
        except urllib.error.URLError:
            raise Exception("BLOCKED_BY: Minio fixture is not available (run docker-compose)")
