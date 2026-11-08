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


def validate_tag_name(tag: str) -> str:
    """Validate the user-facing date/hash identity."""
    match = TAG_METADATA.fullmatch(tag)
    if not match:
        raise VersionError("Version must be yyyy.MM.dd.HH.mm.sss.g<12-lowercase-hex-SHA>")
    try:
        datetime(*map(int, match.groups()[:5]))
    except ValueError as error:
        raise VersionError(f"Invalid date in version: {error}") from error
    return tag


def history(repo: Path) -> list[tuple[str, str, str]]:
    """Validate new and legacy annotated release tags without rewriting either."""
    entries = []
    for tag in git(repo, "tag", "--list").splitlines():
        legacy = tag.startswith("v")
        if not legacy and not re.match(r"^\d{4}\.", tag):
            continue
        if legacy:
            version = SemVer.parse(tag[1:])
            identity = version.metadata or ""
            validate_tag_name(identity)
        else:
            version = None
            identity = validate_tag_name(tag)
        if git(repo, "cat-file", "-t", "refs/tags/" + tag) != "tag":
            raise VersionError(f"Release tag must be annotated: {tag}")
        commit = git(repo, "rev-parse", tag + "^{commit}")
        if identity.rsplit(".g", 1)[1] != commit[:12]:
            raise VersionError(f"Tag hash does not match target commit: {tag}")
        try:
            annotation = json.loads(git(repo, "for-each-ref", "--format=%(contents)", "refs/tags/" + tag))
            if not isinstance(annotation, dict):
                raise ValueError("manifest must be a JSON object")
            instant = datetime.fromisoformat(annotation["created_at_utc"])
            stamp, local_iso, utc_iso = timestamp(instant)
        except (ValueError, TypeError, KeyError) as error:
            raise VersionError(f"Invalid release manifest on tag {tag}: {error}") from error
        expected_version = tag[1:] if legacy else tag
        if (annotation.get("version") != expected_version or annotation.get("commit") != commit
                or annotation.get("tag") != tag or annotation.get("date_pattern") != "yyyy.MM.dd.HH.mm.sss"
                or annotation.get("timezone") != "Europe/Berlin"
                or annotation.get("schema_version") != (1 if legacy else 2)
                or annotation.get("branch") != "master" or annotation.get("short_commit") != commit[:12]
                or annotation.get("created_at_local") != local_iso or annotation.get("created_at_utc") != utc_iso
                or identity != f"{stamp}.g{commit[:12]}"):
            raise VersionError(f"Release manifest does not match tag identity: {tag}")
        if legacy and (annotation.get("base_version") != version.base or
                       annotation.get("release_kind") != ("prerelease" if version.prerelease else "release")):
            raise VersionError(f"Legacy release manifest does not match tag identity: {tag}")
        if not legacy and annotation.get("release_kind") not in ("release", "prerelease"):
            raise VersionError(f"Invalid release kind in tag: {tag}")
        entries.append((tag, identity, commit))
    return entries


def check(repo: Path) -> dict:
    entries = history(repo)
    return {"release_tag_count": len(entries), "configuration_valid": True,
            "release_ready": False, "note": "check validates history; describe/tag also require a clean master"}


def describe(repo: Path, now: datetime | None = None) -> dict:
    entries = history(repo)
    if git(repo, "branch", "--show-current") != "master":
        raise VersionError("Release snapshots must target master")
    if git(repo, "status", "--porcelain", "--untracked-files=all"):
        raise VersionError("Release snapshots require a completely clean working tree")
    commit = git(repo, "rev-parse", "HEAD")
    if any(previous_commit == commit for _, _, previous_commit in entries):
        raise VersionError("This commit already has a release tag")
    stamp, local_iso, utc_iso = timestamp(now or datetime.now(timezone.utc))
    version = validate_tag_name(f"{stamp}.g{commit[:12]}")
    return {"schema_version": 2, "version": version, "tag": version,
            "commit": commit, "short_commit": commit[:12], "branch": "master",
            "date_pattern": "yyyy.MM.dd.HH.mm.sss", "timezone": "Europe/Berlin",
            "created_at_local": local_iso, "created_at_utc": utc_iso,
            "release_kind": "prerelease"}


def create_tag(repo: Path, now: datetime | None = None) -> dict:
    manifest = describe(repo, now)
    # No force flag and no remote command: Git rejects any existing reference.
    git(repo, "tag", "-a", manifest["tag"], manifest["commit"], "-F", "-", input_text=json.dumps(manifest, indent=2) + "\n")
    if git(repo, "rev-parse", manifest["tag"] + "^{commit}") != manifest["commit"]:
        raise VersionError("Created tag does not target the expected commit")
    return manifest


def verify(repo: Path, tag: str) -> dict:
    validate_tag_name(tag)
    if not any(existing == tag for existing, _, _ in history(repo)):
        raise VersionError(f"Release tag not found: {tag}")
    return json.loads(git(repo, "for-each-ref", "--format=%(contents)", "refs/tags/" + tag))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "describe", "tag", "verify"])
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--tag", help="Annotated date/hash tag to verify")
    args = parser.parse_args()
    try:
        repo = repository(args.repo)
        if args.command == "verify":
            if not args.tag:
                raise VersionError("verify requires --tag")
            output = verify(repo, args.tag)
        else:
            output = {"check": check, "describe": describe, "tag": create_tag}[args.command](repo)
        print(json.dumps(output, indent=2))
        return 0
    except VersionError as error:
        print(f"Version policy error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
