#!/usr/bin/env bash
# Shared smoke-test state.

BASE="${BASE:-http://localhost:3000}"
PASS=0
FAIL=0
BODY="${BODY:-/tmp/ubc_smoke_test_body.json}"

cleanup() {
  rm -f "$BODY"
}

trap cleanup EXIT

if ! curl -sS --connect-timeout 2 -o /dev/null "$BASE"; then
  echo
  echo "ERROR: API server tidak dapat dihubungi"
  echo "       $BASE"
  echo
  echo "=========================================="
  echo "Hasil: $PASS PASS, $FAIL FAIL"
  echo "=========================================="
  exit 2
fi
