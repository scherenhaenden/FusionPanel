#!/usr/bin/env python3
"""Discover package manifests and optionally limit the matrix to changed packages."""

import argparse
import glob
import json
import os
import subprocess
import sys
import tomllib

EXCLUDE_DIRS = {".git", "target", "node_modules", "dist", "build", "docs", "__pycache__"}


def root_workspace_members(repo_root):
    """Find explicit members of the root workspace without resolving dependencies."""
    manifest = os.path.join(repo_root, "Cargo.toml")
    if not os.path.isfile(manifest):
        return set()
    with open(manifest, "rb") as f:
        workspace = tomllib.load(f).get("workspace", {})

    def expand(patterns):
        return {
            os.path.realpath(path)
            for pattern in patterns
            for path in glob.glob(os.path.join(repo_root, pattern), recursive=True)
            if os.path.isfile(os.path.join(path, "Cargo.toml"))
        }

    return expand(workspace.get("members", [])) - expand(workspace.get("exclude", []))


def discover_manifests(repo_root):
    cargo_manifests = []
    package_jsons = []
    manifest_dirs = set()
    workspace_members = root_workspace_members(repo_root)

    for root, dirs, files in os.walk(repo_root):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        abs_root = os.path.abspath(root)
        rel_root = os.path.relpath(root, repo_root)

        if "Cargo.toml" in files:
            path = os.path.join(rel_root, "Cargo.toml") if rel_root != "." else "Cargo.toml"
            # Root workspace members are tested together by the root manifest.
            if abs_root == repo_root or os.path.realpath(abs_root) not in workspace_members:
                cargo_manifests.append(path)
            if abs_root != repo_root:
                manifest_dirs.add(abs_root)

        if "package.json" in files:
            path = os.path.join(rel_root, "package.json") if rel_root != "." else "package.json"
            package_jsons.append(path)
            manifest_dirs.add(abs_root)

    return sorted(cargo_manifests), sorted(package_jsons), manifest_dirs


def select_changed_manifests(repo_root, cargo_manifests, package_jsons, base_ref, head_ref):
    """Return manifests owning changed files, mapping root workspace members to Cargo.toml."""
    changed = subprocess.run(
        ["git", "diff", "--name-only", "--diff-filter=ACMRT", f"{base_ref}...{head_ref}"],
        cwd=repo_root,
        check=True,
        text=True,
        capture_output=True,
    ).stdout.splitlines()

    cargo_by_dir = {
        os.path.realpath(os.path.join(repo_root, os.path.dirname(manifest))): manifest
        for manifest in cargo_manifests
    }
    npm_by_dir = {
        os.path.realpath(os.path.join(repo_root, os.path.dirname(manifest))): manifest
        for manifest in package_jsons
    }
    workspace_members = root_workspace_members(repo_root)
    root = os.path.realpath(repo_root)
    selected_cargo = set()
    selected_npm = set()

    for changed_path in changed:
        absolute = os.path.realpath(os.path.join(repo_root, changed_path))
        current = os.path.dirname(absolute)
        parents = []
        while True:
            parents.append(current)
            if current == root:
                break
            current = os.path.dirname(current)
        parents = list(dict.fromkeys(os.path.realpath(parent) for parent in parents))

        # Pick the nearest standalone Cargo package. Workspace members map to the
        # root workspace so one cargo test covers their actual shared unit suite.
        nested_cargo = next(
            (parent for parent in parents if parent != root and parent in cargo_by_dir),
            None,
        )
        if nested_cargo in workspace_members:
            selected_cargo.add("Cargo.toml")
        elif nested_cargo:
            selected_cargo.add(cargo_by_dir[nested_cargo])
        elif changed_path in {"Cargo.toml", "Cargo.lock"} and root in cargo_by_dir:
            selected_cargo.add(cargo_by_dir[root])
        elif any(os.path.commonpath((absolute, member)) == member for member in workspace_members) and root in cargo_by_dir:
            selected_cargo.add(cargo_by_dir[root])

        nested_npm = next((parent for parent in parents if parent in npm_by_dir), None)
        if nested_npm:
            selected_npm.add(npm_by_dir[nested_npm])

    return sorted(selected_cargo), sorted(selected_npm)


def orphan_tests(repo_root, manifest_dirs):
    orphaned = []
    for root, dirs, files in os.walk(repo_root):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for name in files:
            is_cargo_test = name.endswith(".rs") and (name.startswith("test_") or name.endswith("_test.rs"))
            is_npm_test = name.endswith((".test.ts", ".spec.ts", ".test.js", ".spec.js"))
            if not (is_cargo_test or is_npm_test):
                continue

            full_path = os.path.abspath(os.path.join(root, name))
            rel_path = os.path.relpath(full_path, repo_root)
            is_in_package = any(full_path.startswith(md + os.sep) for md in manifest_dirs)
            is_in_root_tests = rel_path.startswith("tests" + os.sep) or rel_path.startswith("src" + os.sep)
            if not is_in_package and not is_in_root_tests:
                orphaned.append(rel_path)
    return orphaned


def write_outputs(cargo_manifests, package_jsons):
    output_file = os.environ.get("GITHUB_OUTPUT")
    if output_file:
        with open(output_file, "a", encoding="utf-8") as f:
            f.write(f"cargo_manifests={json.dumps(cargo_manifests)}\n")
            f.write(f"package_jsons={json.dumps(package_jsons)}\n")
    else:
        print(f"cargo_manifests={json.dumps(cargo_manifests)}")
        print(f"package_jsons={json.dumps(package_jsons)}")


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv == ["--root-workspace-members"]:
        repo_root = os.path.abspath(".")
        for member in sorted(root_workspace_members(repo_root)):
            print(os.path.join(os.path.relpath(member, repo_root), "Cargo.toml"))
        return 0

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--changed-from", help="base commit for changed-manifest selection")
    parser.add_argument("--changed-to", help="head commit for changed-manifest selection (defaults to HEAD)")
    args = parser.parse_args(argv)
    if args.changed_to and not args.changed_from:
        parser.error("--changed-to requires --changed-from")

    repo_root = os.path.abspath(".")
    all_cargo, all_npm, manifest_dirs = discover_manifests(repo_root)
    orphaned = orphan_tests(repo_root, manifest_dirs)
    if orphaned:
        print(f"::error::Orphan tests detected: {', '.join(orphaned)}")
        return 1

    if args.changed_from:
        cargo_manifests, package_jsons = select_changed_manifests(
            repo_root, all_cargo, all_npm, args.changed_from, args.changed_to or "HEAD"
        )
    else:
        cargo_manifests, package_jsons = all_cargo, all_npm

    write_outputs(cargo_manifests, package_jsons)
    return 0


if __name__ == "__main__":
    sys.exit(main())
