#!/usr/bin/env python3
import os
import json
import sys
import subprocess

EXCLUDE_DIRS = {".git", "target", "node_modules", "dist", "build", "docs", "__pycache__"}

def main():
    cargo_manifests = []
    package_jsons = []
    manifest_dirs = set()

    repo_root = os.path.abspath(".")

    # Discover all manifests excluding ignored directories
    for root, dirs, files in os.walk("."):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]

        rel_root = os.path.relpath(root, ".")
        abs_root = os.path.abspath(root)

        if "Cargo.toml" in files:
            path = os.path.join(rel_root, "Cargo.toml").removeprefix("./")
            # Don't add root workspace to individual package matrix if subcrates exist
            cargo_manifests.append(path)
            if abs_root != repo_root:
                manifest_dirs.add(abs_root)

        if "package.json" in files:
            path = os.path.join(rel_root, "package.json").removeprefix("./")
            package_jsons.append(path)
            manifest_dirs.add(abs_root)

    cargo_manifests.sort()
    package_jsons.sort()

    # Detect Orphan Tests:
    # A test file is orphaned if it does not reside under any concrete package directory,
    # or if its only enclosing manifest is the root repository directory outside ./tests or ./src.
    orphan_tests = []
    for root, dirs, files in os.walk("."):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for f in files:
            is_cargo_test = f.endswith(".rs") and (f.startswith("test_") or f.endswith("_test.rs"))
            is_npm_test = (f.endswith(".test.ts") or f.endswith(".spec.ts") or f.endswith(".test.js") or f.endswith(".spec.js"))
            if is_cargo_test or is_npm_test:
                full_path = os.path.abspath(os.path.join(root, f))
                rel_path = os.path.relpath(full_path, repo_root)

                # Check if it is inside one of the concrete package directories
                is_in_package = any(full_path.startswith(md + os.sep) for md in manifest_dirs)

                # Or inside root ./tests or ./src
                is_in_root_tests = rel_path.startswith("tests" + os.sep) or rel_path.startswith("src" + os.sep)

                if not is_in_package and not is_in_root_tests:
                    orphan_tests.append(rel_path)

    if orphan_tests:
        print(f"::error::Orphan tests detected: {', '.join(orphan_tests)}")
        sys.exit(1)

    output_file = os.environ.get("GITHUB_OUTPUT")
    if output_file:
        with open(output_file, "a") as f:
            f.write(f"cargo_manifests={json.dumps(cargo_manifests)}\n")
            f.write(f"package_jsons={json.dumps(package_jsons)}\n")
    else:
        print(f"cargo_manifests={json.dumps(cargo_manifests)}")
        print(f"package_jsons={json.dumps(package_jsons)}")

if __name__ == "__main__":
    main()
