import os
import shutil
import socket
import subprocess
import tempfile
import unittest
import urllib.error
import urllib.request
from pathlib import Path
from runner import poll_readiness

COMPOSE_FILE = Path(__file__).resolve().parent / "fixtures" / "docker-compose.yml"
COMPOSE_PROJECT = os.environ.get("FP_TEST_COMPOSE_PROJECT", "fusionpanel-test")
FIXTURE_ENV_FILE = Path(tempfile.gettempdir()) / "fusionpanel-test-fixture-secrets" / f"{COMPOSE_PROJECT}.env"


def _compose(*args, timeout=15):
    docker = shutil.which("docker")
    if not docker:
        raise Exception("BLOCKED_BY: Docker CLI is not installed")
    if FIXTURE_ENV_FILE.is_symlink() or not FIXTURE_ENV_FILE.is_file():
        raise Exception("BLOCKED_BY: generated fixture credentials are missing; run tests/fixtures/compose.py up")
    try:
        return subprocess.run(
            [docker, "compose", "-p", COMPOSE_PROJECT, "--env-file", str(FIXTURE_ENV_FILE), "-f", str(COMPOSE_FILE), *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise Exception("BLOCKED_BY: Docker Compose command timed out")
    except OSError as exc:
        raise Exception(f"BLOCKED_BY: could not run Docker Compose: {exc}")


def _host_port(service, container_port):
    """Resolve the loopback host port Compose assigned to a disposable service."""
    result = _compose("port", service, str(container_port))
    if result.returncode:
        raise Exception(f"BLOCKED_BY: {service} fixture port is unavailable: {result.stderr.strip()}")
    try:
        return int(result.stdout.strip().rsplit(":", 1)[1])
    except (IndexError, ValueError):
        raise Exception(f"BLOCKED_BY: Compose did not report a usable port for {service}")


def _compose_exec(service, script):
    """Run a bounded check inside a disposable Compose fixture container."""
    result = _compose("exec", "-T", service, "sh", "-ec", script)
    if result.returncode and ("is not running" in result.stderr or "no such service" in result.stderr):
        raise Exception(f"BLOCKED_BY: {service} fixture is not running: {result.stderr.strip()}")
    return result


def _wait_for_container_health(service, timeout_seconds=30):
    """Wait for a database's actual Compose healthcheck before running SQL assertions."""
    container = _compose("ps", "-q", service)
    container_id = container.stdout.strip()
    if container.returncode or not container_id:
        raise Exception(f"BLOCKED_BY: {service} fixture is not running")
    docker = shutil.which("docker")

    def is_healthy():
        result = subprocess.run(
            [docker, "inspect", "--format", "{{.State.Health.Status}}", container_id],
            capture_output=True,
            text=True,
            timeout=5,
        )
        return result.returncode == 0 and result.stdout.strip() == "healthy"

    if not poll_readiness(is_healthy, timeout_seconds=timeout_seconds, interval=0.5):
        raise Exception(f"BLOCKED_BY: {service} fixture did not become healthy within {timeout_seconds}s")


def _wait_for_host_port(port, label):
    """Wait for a published fixture port to accept connections, with a finite timeout."""
    import time

    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return
        except OSError:
            time.sleep(0.25)
    raise Exception(f"BLOCKED_BY: {label} fixture is unavailable on loopback port {port}")


class TestIntegrationFixtures(unittest.TestCase):
    def test_nginx_fixture(self):
        """A fixture-specific response proves this is the disposable Nginx service."""
        self.evidence_details = {"expected": "disposable Nginx fixture marker", "host_port": None}
        port = _host_port("nginx", 80)
        self.evidence_details["host_port"] = port
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=3) as response:
                self.assertEqual(response.getcode(), 200)
                body = response.read().decode().strip()
                self.assertEqual(body, "FusionPanel disposable Nginx fixture")
                self.evidence_details["observed"] = {"status_code": response.getcode(), "body": body}
        except urllib.error.URLError:
            raise Exception("BLOCKED_BY: Nginx fixture is not available (run docker-compose)")

    def test_postgres_fixture_executes_query(self):
        """Connect to the actual fixture database and verify its configured identity."""
        port = _host_port("postgres", 5432)
        _wait_for_container_health("postgres")
        _wait_for_host_port(port, "PostgreSQL")
        result = _compose_exec(
            "postgres",
            'PGPASSWORD="$POSTGRES_PASSWORD" exec psql -h 127.0.0.1 '
            '-U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atqc '
            '"SELECT current_database() || \':\' || current_user"',
        )
        if result.returncode:
            self.fail(f"PostgreSQL query failed: {result.stderr.strip()}")
        self.assertEqual(result.stdout.strip(), "fp_db:fp_user")
        self.evidence_details = {
            "expected": "fp_db:fp_user",
            "observed": result.stdout.strip(),
            "host_port": port,
        }

    def test_mariadb_fixture_executes_query(self):
        """Connect to the actual fixture database and verify its configured identity."""
        port = _host_port("mariadb", 3306)
        _wait_for_container_health("mariadb")
        _wait_for_host_port(port, "MariaDB")
        result = _compose_exec(
            "mariadb",
            'MYSQL_PWD="$MYSQL_PASSWORD" exec mariadb --batch --skip-column-names '
            '-h 127.0.0.1 -u "$MYSQL_USER" "$MYSQL_DATABASE" '
            '-e "SELECT CONCAT(DATABASE(), \':\', SUBSTRING_INDEX(CURRENT_USER(), \'@\', 1))"',
        )
        if result.returncode:
            self.fail(f"MariaDB query failed: {result.stderr.strip()}")
        self.assertEqual(result.stdout.strip(), "fp_db:fp_user")
        self.evidence_details = {
            "expected": "fp_db:fp_user",
            "observed": result.stdout.strip(),
            "host_port": port,
        }

    def test_minio_fixture(self):
        """Test the object-store health endpoint on its loopback-only published port."""
        port = _host_port("minio", 9000)
        self.evidence_details = {"expected": "HTTP 200 from MinIO live health endpoint", "host_port": port}
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/minio/health/live", timeout=3) as response:
                self.assertEqual(response.getcode(), 200)
                self.evidence_details["observed"] = {"status_code": response.getcode()}
        except urllib.error.URLError:
            raise Exception("BLOCKED_BY: MinIO fixture is not available (run docker-compose)")
