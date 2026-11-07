#!/bin/bash
set -euo pipefail

echo "Auditing for forbidden mock bypasses and ignored tests..."

failed=0

# Check for #[ignore] in tests
if grep -rnw . --include=\*.rs -e '#\[ignore\]' | grep -v "/target/" | grep -v "/.git/"; then
    echo "::error::Found ignored tests! All ignored or service-gated test suites must be reported as BLOCKED with their prerequisite."
    failed=1
fi

# Check for assert!(true) placeholders in tests/closure/
if [ -d "tests/closure" ]; then
    if find tests/closure/ -name "*.rs" -exec grep -Hn "assert!(true)" {} + 2>/dev/null | grep -q "assert!(true)"; then
        find tests/closure/ -name "*.rs" -exec grep -Hn "assert!(true)" {} + 2>/dev/null
        echo "::error::Found assert!(true) placeholders in tests/closure/! Forbidden."
        failed=1
    fi

    # Check for #[should_panic] in tests/closure/
    if find tests/closure/ -name "*.rs" -exec grep -Hn "#\[should_panic\]" {} + 2>/dev/null | grep -q "#\[should_panic\]"; then
        find tests/closure/ -name "*.rs" -exec grep -Hn "#\[should_panic\]" {} + 2>/dev/null
        echo "::error::Found #[should_panic] in tests/closure/! Forbidden."
        failed=1
    fi
fi

if [ "$failed" -eq 1 ]; then
    /bin/false
fi
echo "Mock/Ignore bypass audit passed."
