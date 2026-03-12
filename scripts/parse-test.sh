#!/usr/bin/env bash
# Test if R files parse successfully (ignoring runtime errors)
# Usage: ./scripts/parse-test.sh <dir> [--verbose]

set -euo pipefail

DIR="${1:?Usage: parse-test.sh <dir> [--verbose]}"
VERBOSE="${2:-}"
BINARY="./target/release/r"
TIMEOUT_BIN=""

if command -v timeout >/dev/null 2>&1; then
    TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_BIN="gtimeout"
fi

# Build release for speed
cargo build --release --quiet 2>/dev/null

PASS=0
FAIL=0
CRASH=0
TOTAL=0
FAIL_FILES=()
CRASH_FILES=()

for f in $(find "$DIR" -name '*.R' -type f | sort); do
    TOTAL=$((TOTAL + 1))
    # Run with timeout, capture stderr
    if [ -n "$TIMEOUT_BIN" ]; then
        output=$("$TIMEOUT_BIN" 5 "$BINARY" "$f" 2>&1 || true)
    else
        output=$("$BINARY" "$f" 2>&1 || true)
    fi

    if echo "$output" | grep -q "^Error in parse:"; then
        FAIL=$((FAIL + 1))
        FAIL_FILES+=("$f")
        if [ "$VERBOSE" = "--verbose" ]; then
            # Extract first line of parse error
            errmsg=$(echo "$output" | grep "^Error in parse:" | head -1)
            echo "FAIL  $f"
            echo "      $errmsg"
        fi
    elif echo "$output" | grep -q "panicked at"; then
        CRASH=$((CRASH + 1))
        CRASH_FILES+=("$f")
        if [ "$VERBOSE" = "--verbose" ]; then
            errmsg=$(echo "$output" | grep "panicked at" | head -1)
            echo "CRASH $f"
            echo "      $errmsg"
        fi
    else
        PASS=$((PASS + 1))
        if [ "$VERBOSE" = "--verbose" ]; then
            echo "OK    $f"
        fi
    fi
done

echo ""
echo "=== Parse Test Results for $DIR ==="
echo "Total:   $TOTAL"
echo "Passed:  $PASS ($((PASS * 100 / TOTAL))%)"
echo "Failed:  $FAIL ($((FAIL * 100 / TOTAL))%)"
echo "Crashed: $CRASH ($((CRASH * 100 / TOTAL))%)"

if [ ${#CRASH_FILES[@]} -gt 0 ]; then
    echo ""
    echo "--- Crashes (panics) ---"
    for f in "${CRASH_FILES[@]}"; do
        echo "  $f"
    done
fi

if [ "$VERBOSE" != "--verbose" ] && [ ${#FAIL_FILES[@]} -gt 0 ]; then
    echo ""
    echo "--- Parse failures (first 20) ---"
    for f in "${FAIL_FILES[@]:0:20}"; do
        echo "  $f"
    done
    if [ ${#FAIL_FILES[@]} -gt 20 ]; then
        echo "  ... and $((${#FAIL_FILES[@]} - 20)) more"
    fi
fi
