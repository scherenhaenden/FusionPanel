#!/usr/bin/env python3
"""Manage isolated Compose fixtures with generated, private test credentials."""

import argparse
import fcntl
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

COMPOSE_FILE = Path(__file__).resolve().parent / "docker-compose.yml"
PROJECTS_DIR = Path(tempfile.gettempdir()) / "fusionpanel-test-fixture-secrets"
PROJECT_PATTERN = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,62}$")

def check_docker():
    docker = shutil.which("docker")
    if not docker:
        print("BLOCKED: Docker CLI is not installed.", file=sys.stderr)
        sys.exit(1)
    return docker

def project_paths(project):
    if not PROJECT_PATTERN.fullmatch(project):
        raise ValueError("project must contain only letters, digits, dot, underscore, or hyphen")
    if PROJECTS_DIR.is_symlink():
        raise ValueError("temporary fixture credential directory must not be a symbolic link")
    PROJECTS_DIR.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(PROJECTS_DIR, 0o700)
    return PROJECTS_DIR / f"{project}.env", PROJECTS_DIR / f"{project}.lock"

def write_credentials(env_path):
    values = {
        "FP_TEST_POSTGRES_PASSWORD": secrets.token_urlsafe(32),
        "FP_TEST_MARIADB_ROOT_PASSWORD": secrets.token_urlsafe(32),
        "FP_TEST_MARIADB_PASSWORD": secrets.token_urlsafe(32),
        "FP_TEST_MINIO_PASSWORD": secrets.token_urlsafe(32),
    }
    content = "".join(f"{name}={value}\n" for name, value in values.items())
    temporary = env_path.with_name(f".{env_path.name}.{secrets.token_hex(8)}.tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, env_path)
    finally:
        if temporary.exists():
            temporary.unlink()
    os.chmod(env_path, 0o600)

def run_compose(project, env_path, services=None):
    docker = check_docker()
    command = [
        docker,
        "compose",
        "-p",
        project,
        "--env-file",
        str(env_path),
        "-f",
        str(COMPOSE_FILE),
        "up",
        "-d"
    ]
    if services:
        command.extend(services)

    result = subprocess.run(command, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        err = result.stderr.lower()
        if "failed to convert whiteout file" in err or "failed to extract layer" in err or "error" in err or "not found" in err:
            print(f"BLOCKED: Docker Compose up failed due to environment constraints or missing image: {result.stderr.strip()}", file=sys.stderr)
            sys.exit(1)
        print(f"Docker Compose up failed with exit code {result.returncode}; output suppressed to protect credentials.", file=sys.stderr)
    return result.returncode

def run_compose_down(project, env_path):
    docker = check_docker()
    command = [
        docker,
        "compose",
        "-p",
        project,
        "--env-file",
        str(env_path),
        "-f",
        str(COMPOSE_FILE),
        "down",
        "--volumes",
        "--remove-orphans"
    ]
    result = subprocess.run(command, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        print(f"Docker Compose down failed with exit code {result.returncode}; output suppressed to protect credentials.", file=sys.stderr)
    return result.returncode

def wait_for_service(project, env_path, services=None, timeout=60):
    docker = check_docker()
    start_time = time.time()

    services_to_wait = services if services else []

    while time.time() - start_time < timeout:
        cmd = [
            docker, "compose", "-p", project, "--env-file", str(env_path),
            "-f", str(COMPOSE_FILE), "ps", "--format", "json"
        ]
        if services_to_wait:
            cmd.extend(services_to_wait)

        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            print("BLOCKED: Failed to get compose ps.", file=sys.stderr)
            sys.exit(1)

        output = res.stdout.strip()
        if not output:
             # Just try again until timeout
             time.sleep(1)
             continue

        all_healthy = True
        lines = [line.strip() for line in output.split('\n') if line.strip()]
        try:
            for line in lines:
                container = json.loads(line)
                health = container.get("Health", "")
                if health != "healthy":
                    all_healthy = False
                    break
        except Exception as e:
            all_healthy = False

        if all_healthy and lines:
            return 0
        time.sleep(1)

    print(f"BLOCKED: Timeout waiting for services to become healthy after {timeout} seconds.", file=sys.stderr)
    sys.exit(1)

def get_env_details(env_path):
    # Parse env file for password
    if not os.path.exists(env_path):
        print(f"BLOCKED: env file missing.", file=sys.stderr)
        sys.exit(1)
    with open(env_path, "r", encoding="utf-8") as f:
        content = f.read()
    env_vars = dict(line.strip().split('=', 1) for line in content.splitlines() if '=' in line)
    return env_vars

def print_env(project, env_path):
    env_vars = get_env_details(env_path)

    # We need the port allocated.
    docker = check_docker()
    cmd = [
        docker, "compose", "-p", project, "--env-file", str(env_path),
        "-f", str(COMPOSE_FILE), "port", "mariadb", "3306"
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print("BLOCKED: Failed to get mariadb port.", file=sys.stderr)
        sys.exit(1)

    port_str = res.stdout.strip().split(':')[-1]

    out = {
        "host": "127.0.0.1",
        "port": int(port_str),
        "root_user": "root",
        "root_password_env_name": "FP_TEST_MARIADB_ROOT_PASSWORD",
        "app_user": "fp_user",
        "database": "fp_db"
    }
    print(json.dumps(out))
    return 0

def run_client(project, env_path, statement):
    docker = check_docker()
    env_vars = get_env_details(env_path)

    # We use credentials via env or a 0600 temp file.
    # We inject via environment variable so it doesn't show in argv.
    cmd = [
        docker, "compose", "-p", project, "--env-file", str(env_path),
        "-f", str(COMPOSE_FILE), "exec",
        "-T",
        "-e", f"MYSQL_PWD={env_vars['FP_TEST_MARIADB_ROOT_PASSWORD']}",
        "mariadb", "mariadb", "-u", "root", "-e", statement
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Client statement failed: {res.stderr}", file=sys.stderr)
        sys.exit(1)

    print(res.stdout, end="")
    return 0

def verify_clean(project):
    docker = check_docker()
    cmd_containers = [docker, "ps", "-a", "--filter", f"label=com.docker.compose.project={project}", "--format", "{{.ID}}"]
    res = subprocess.run(cmd_containers, capture_output=True, text=True)
    containers = res.stdout.strip()

    cmd_volumes = [docker, "volume", "ls", "--filter", f"label=com.docker.compose.project={project}", "--format", "{{.Name}}"]
    res = subprocess.run(cmd_volumes, capture_output=True, text=True)
    volumes = res.stdout.strip()

    cmd_networks = [docker, "network", "ls", "--filter", f"label=com.docker.compose.project={project}", "--format", "{{.Name}}"]
    res = subprocess.run(cmd_networks, capture_output=True, text=True)
    networks = res.stdout.strip()

    if containers or volumes or networks:
        print("BLOCKED: Verify-clean failed. Resources left behind:", file=sys.stderr)
        if containers: print(f"Containers: {containers}", file=sys.stderr)
        if volumes: print(f"Volumes: {volumes}", file=sys.stderr)
        if networks: print(f"Networks: {networks}", file=sys.stderr)
        sys.exit(1)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)

    parser_up = subparsers.add_parser("up")
    parser_up.add_argument("services", nargs="*")

    parser_down = subparsers.add_parser("down")

    parser_wait = subparsers.add_parser("wait")
    parser_wait.add_argument("services", nargs="*")

    parser_env = subparsers.add_parser("env")

    parser_client = subparsers.add_parser("client")
    parser_client.add_argument("statement")

    parser_verify = subparsers.add_parser("verify-clean")

    parser.add_argument("--project", default="fusionpanel-test")
    args = parser.parse_args()

    try:
        env_path, lock_path = project_paths(args.project)
    except ValueError as exc:
        parser.error(str(exc))

    if args.action == "verify-clean":
        return verify_clean(args.project)

    lock_fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX)

        if args.action == "up":
            if not env_path.exists():
                write_credentials(env_path)
            elif env_path.is_symlink() or not env_path.is_file():
                print("Fixture credential path is not a regular file.", file=sys.stderr)
                return 2
            else:
                os.chmod(env_path, 0o600)
            exit_code = run_compose(args.project, env_path, args.services)
            if exit_code == 0:
                print(f"Disposable fixture project is running: {args.project}")
            return exit_code

        if not env_path.exists():
            print(f"No generated fixture credentials exist for project {args.project}.")
            return 0
        if env_path.is_symlink() or not env_path.is_file():
            print("Fixture credential path is not a regular file.", file=sys.stderr)
            return 2

        if args.action == "down":
            exit_code = run_compose_down(args.project, env_path)
            if exit_code == 0:
                env_path.unlink()
                print(f"Disposable fixture project and generated credentials removed: {args.project}")
            return exit_code

        elif args.action == "wait":
            return wait_for_service(args.project, env_path, args.services)

        elif args.action == "env":
            return print_env(args.project, env_path)

        elif args.action == "client":
            return run_client(args.project, env_path, args.statement)

    finally:
        fcntl.flock(lock_fd, fcntl.LOCK_UN)
        os.close(lock_fd)


if __name__ == "__main__":
    sys.exit(main())
