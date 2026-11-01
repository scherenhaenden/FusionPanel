#!/usr/bin/env python3
"""Validate and create immutable FusionPanel release tags using local Git only."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
from functools import total_ordering
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from zoneinfo import ZoneInfo

ZONE = ZoneInfo("Europe/Berlin")
SEMVER = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$")
TAG_METADATA = re.compile(r"^(\d{4})\.(\d{2})\.(\d{2})\.(\d{2})\.(\d{2})\.(\d{3})\.g([0-9a-f]{12})$")


class VersionError(ValueError):
    """A version, repository state or release tag violates policy."""


@total_ordering
@dataclass(frozen=True, eq=False)
class SemVer:
    major: int
    minor: int
    patch: int
    prerelease: tuple[str, ...] = ()
    metadata: str | None = None

    @classmethod
    def parse(cls, value: str) -> "SemVer":
        match = SEMVER.fullmatch(value)
        if not match:
            raise VersionError(f"Not strict SemVer: {value!r}")
        prerelease = tuple(match[4].split(".")) if match[4] else ()
        if any(part.isdigit() and len(part) > 1 and part.startswith("0") for part in prerelease):
            raise VersionError("Numeric prerelease identifiers cannot have leading zeroes")
        return cls(int(match[1]), int(match[2]), int(match[3]), prerelease, match[5])

    @property
    def base(self) -> str:
        value = f"{self.major}.{self.minor}.{self.patch}"
        return value + ("-" + ".".join(self.prerelease) if self.prerelease else "")

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return (self.major, self.minor, self.patch, self.prerelease) == (other.major, other.minor, other.patch, other.prerelease)

    def __hash__(self) -> int:
        return hash((self.major, self.minor, self.patch, self.prerelease))

    def __lt__(self, other: "SemVer") -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        core, other_core = (self.major, self.minor, self.patch), (other.major, other.minor, other.patch)
        if core != other_core:
            return core < other_core
        if not self.prerelease or not other.prerelease:
            return bool(self.prerelease) and not other.prerelease
        for left, right in zip(self.prerelease, other.prerelease):
            if left == right:
                continue
            if left.isdigit() and right.isdigit():
                return int(left) < int(right)
            if left.isdigit() != right.isdigit():
                return left.isdigit()
            return left < right
        return len(self.prerelease) < len(other.prerelease)


def timestamp(value: datetime) -> tuple[str, str, str]:
    if value.tzinfo is None or value.utcoffset() is None:
        raise VersionError("Release time must be timezone-aware")
    local = value.astimezone(ZONE)
    stamp = local.strftime("%Y.%m.%d.%H.%M.") + f"{local.microsecond // 1000:03d}"
    return stamp, local.isoformat(timespec="milliseconds"), value.astimezone(timezone.utc).isoformat(timespec="milliseconds")


def git(repo: Path, *args: str, input_text: str | None = None) -> str:
    result = subprocess.run(["git", "-C", str(repo), *args], input=input_text, text=True, capture_output=True)
    if result.returncode:
        raise VersionError(result.stderr.strip() or f"Git command failed: {args[0]}")
    return result.stdout.strip()


def repository(path: Path) -> Path:
    return Path(git(path, "rev-parse", "--show-toplevel")).resolve()


def configured_version(repo: Path) -> SemVer:
    try:
        raw_text = (repo / "VERSION").read_text()
        raw = raw_text.removesuffix("\n")
        if raw_text not in (raw, raw + "\n") or "\n" in raw or raw != raw.strip():
            raise VersionError("VERSION must contain exactly one unpadded SemVer line")
        manifest = tomllib.loads((repo / "Cargo.toml").read_text())
        cargo_version = manifest["workspace"]["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        raise VersionError(f"Cannot read VERSION and workspace package version: {error}") from error
    version = SemVer.parse(raw)
    if version.metadata:
        raise VersionError("VERSION must contain only core/prerelease; date/hash metadata is generated")
    if cargo_version != raw:
        raise VersionError("VERSION and workspace.package.version must match exactly")
    return version


def validate_tag_name(tag: str) -> SemVer:
    if not tag.startswith("v"):
        raise VersionError("Release tags must start with v")
    version = SemVer.parse(tag[1:])
    match = TAG_METADATA.fullmatch(version.metadata or "")
    if not match:
        raise VersionError("Tag metadata must be yyyy.MM.dd.HH.mm.sss.g<12-lowercase-hex-SHA>")
    try:
        datetime(*map(int, match.groups()[:5]))
    except ValueError as error:
        raise VersionError(f"Invalid date in tag: {error}") from error
    return version


def history(repo: Path) -> list[tuple[str, SemVer, str]]:
    entries = []
    for tag in git(repo, "tag", "--list", "v*").splitlines():
        version = validate_tag_name(tag)
        if git(repo, "cat-file", "-t", "refs/tags/" + tag) != "tag":
            raise VersionError(f"Release tag must be annotated: {tag}")
        commit = git(repo, "rev-parse", tag + "^{commit}")
        if version.metadata.rsplit(".g", 1)[1] != commit[:12]:
            raise VersionError(f"Tag hash does not match target commit: {tag}")
        try:
            annotation = json.loads(git(repo, "for-each-ref", "--format=%(contents)", "refs/tags/" + tag))
            if not isinstance(annotation, dict):
                raise VersionError("Tag annotation must be a JSON manifest object")
            instant = datetime.fromisoformat(annotation["created_at_utc"])
            stamp, local_iso, utc_iso = timestamp(instant)
        except (ValueError, TypeError, KeyError) as error:
            raise VersionError(f"Invalid release manifest on tag {tag}: {error}") from error
        if (annotation.get("version") != tag[1:] or annotation.get("base_version") != version.base
                or annotation.get("commit") != commit or annotation.get("tag") != tag
                or annotation.get("date_pattern") != "yyyy.MM.dd.HH.mm.sss"
                or annotation.get("timezone") != "Europe/Berlin"
                or annotation.get("schema_version") != 1
                or annotation.get("branch") != "master"
                or annotation.get("short_commit") != commit[:12]
                or annotation.get("release_kind") != ("prerelease" if version.prerelease else "release")
                or annotation.get("created_at_local") != local_iso
                or annotation.get("created_at_utc") != utc_iso
                or version.metadata != f"{stamp}.g{commit[:12]}"):
            raise VersionError(f"Release manifest does not match tag identity: {tag}")
        entries.append((tag, version, commit))
    bases = [version.base for _, version, _ in entries]
    if len(set(bases)) != len(bases):
        raise VersionError("A core/prerelease version was published more than once")
    return entries


def check(repo: Path) -> dict:
    version = configured_version(repo)
    entries = history(repo)
    latest = max((v for _, v, _ in entries), default=None)
    if latest and version < latest:
        raise VersionError("Configured version is older than the latest published SemVer")
    return {"base_version": version.base, "release_tag_count": len(entries), "configuration_valid": True,
            "release_ready": False, "note": "check validates configuration/history; describe/tag also enforce clean master and publication rules"}


def describe(repo: Path, now: datetime | None = None) -> dict:
    version = configured_version(repo)
    entries = history(repo)
    if git(repo, "branch", "--show-current") != "master":
        raise VersionError("Release snapshots must target master")
    if git(repo, "status", "--porcelain", "--untracked-files=all"):
        raise VersionError("Release snapshots require a completely clean working tree")
    if entries and not all(version > previous for _, previous, _ in entries):
        raise VersionError("Bump VERSION and Cargo.toml: every new tag must increase SemVer core/prerelease")
    commit = git(repo, "rev-parse", "HEAD")
    stamp, local_iso, utc_iso = timestamp(now or datetime.now(timezone.utc))
    full = f"{version.base}+{stamp}.g{commit[:12]}"
    tag = "v" + full
    validate_tag_name(tag)
    return {"schema_version": 1, "base_version": version.base, "version": full, "tag": tag,
            "commit": commit, "short_commit": commit[:12], "branch": "master",
            "date_pattern": "yyyy.MM.dd.HH.mm.sss", "timezone": "Europe/Berlin",
            "created_at_local": local_iso, "created_at_utc": utc_iso,
            "release_kind": "prerelease" if version.prerelease else "release"}


def create_tag(repo: Path, now: datetime | None = None) -> dict:
    manifest = describe(repo, now)
    # No force flag and no remote command: Git rejects any existing reference.
    git(repo, "tag", "-a", manifest["tag"], manifest["commit"], "-F", "-", input_text=json.dumps(manifest, indent=2) + "\n")
    if git(repo, "rev-parse", manifest["tag"] + "^{commit}") != manifest["commit"]:
        raise VersionError("Created tag does not target the expected commit")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "describe", "tag"])
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    args = parser.parse_args()
    try:
        repo = repository(args.repo)
        output = {"check": check, "describe": describe, "tag": create_tag}[args.command](repo)
        print(json.dumps(output, indent=2))
        return 0
    except VersionError as error:
        print(f"Version policy error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
