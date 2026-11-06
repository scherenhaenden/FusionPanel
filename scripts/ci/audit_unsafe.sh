#!/bin/bash
set -euo pipefail

echo "Auditing Rust files for unsafe_code forbid..."

failed=0

# Check main.rs, lib.rs and tests/*.rs
while IFS= read -r file; do
    if ! grep -q '#!\[forbid(unsafe_code)\]' "$file"; then
        echo "::error::File $file does not forbid unsafe code (#![forbid(unsafe_code)] missing)."
        failed=1
    fi
done < <(find . -name "lib.rs" -o -name "main.rs" -o -path "*/tests/*.rs" | grep -v "/target/" | grep -v "/.git/")

if [ "$failed" -eq 1 ]; then
    /bin/false
fi
echo "Audit passed."
