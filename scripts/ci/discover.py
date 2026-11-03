#!/usr/bin/env python3
import os
import json
import sys
import subprocess

def main():
    cargo_manifests = []
    package_jsons = []
    manifest_dirs = []

    # Discover all manifests
    for root, dirs, files in os.walk("."):
        # Exclude directories
        dirs[:] = [d for d in dirs if d not in [".git", "target", "node_modules", "dist", "build"]]

        is_manifest_dir = False
        if "Cargo.toml" in files:
            path = os.path.join(root, "Cargo.toml").removeprefix("./")
            cargo_manifests.append(path)
            is_manifest_dir = True

        if "package.json" in files:
            path = os.path.join(root, "package.json").removeprefix("./")
            package_jsons.append(path)
            is_manifest_dir = True

        if is_manifest_dir:
            # store the absolute or normalized directory path
            manifest_dirs.append(os.path.abspath(root))

    cargo_manifests.sort()
    package_jsons.sort()

    # Detect Orphan Tests
    orphan_tests = []
    for root, dirs, files in os.walk("."):
        dirs[:] = [d for d in dirs if d not in [".git", "target", "node_modules", "dist", "build"]]
        for f in files:
            if f.endswith("_test.rs") or f.startswith("test_") or f.endswith(".test.ts") or f.endswith(".spec.ts"):
                full_path = os.path.abspath(os.path.join(root, f))
                # Check if it belongs to any manifest dir
                is_covered = any(full_path.startswith(md) for md in manifest_dirs)
                if not is_covered:
                    orphan_tests.append(os.path.join(root, f).removeprefix("./"))

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
