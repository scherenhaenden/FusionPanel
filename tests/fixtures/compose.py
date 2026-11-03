#!/usr/bin/env python3
"""Manage isolated Compose fixtures with generated, private test credentials."""

import argparse
import fcntl
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


COMPOSE_FILE = Path(__file__).resolve().parent / "docker-compose.yml"
PROJECTS_DIR = Path(tempfile.gettempdir()) / "fusionpanel-test-fixture-secrets"
PROJECT_PATTERN = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,62}$")


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


def run_compose(project, env_path, action):
    docker = shutil.which("docker")
    if not docker:
        print("Docker CLI is not installed.", file=sys.stderr)
        return 127
    command = [
        docker,
        "compose",
        "-p",
        project,
        "--env-file",
        str(env_path),
        "-f",
        str(COMPOSE_FILE),
    ]
    command.extend(["up", "-d"] if action == "up" else ["down", "--volumes", "--remove-orphans"])
    result = subprocess.run(command, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        print(f"Docker Compose {action} failed with exit code {result.returncode}; output suppressed to protect credentials.", file=sys.stderr)
    return result.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("up", "down"))
    parser.add_argument("--project", default="fusionpanel-test")
    args = parser.parse_args()

    try:
        env_path, lock_path = project_paths(args.project)
    except ValueError as exc:
        parser.error(str(exc))

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
            exit_code = run_compose(args.project, env_path, args.action)
            if exit_code == 0:
                print(f"Disposable fixture project is running: {args.project}")
            return exit_code

        if not env_path.exists():
            print(f"No generated fixture credentials exist for project {args.project}.")
            return 0
        if env_path.is_symlink() or not env_path.is_file():
            print("Fixture credential path is not a regular file.", file=sys.stderr)
            return 2
        exit_code = run_compose(args.project, env_path, args.action)
        if exit_code == 0:
            env_path.unlink()
            print(f"Disposable fixture project and generated credentials removed: {args.project}")
        return exit_code
    finally:
        fcntl.flock(lock_fd, fcntl.LOCK_UN)
        os.close(lock_fd)


if __name__ == "__main__":
    sys.exit(main())
