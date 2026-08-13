#!/usr/bin/env bash
# Generic assertions. Error text is never hardcoded here.
#
# check_contains FIELD DESCRIPTION
# Verifies that FIELD exists and is non-empty, then prints the exact
# JSON response returned by the endpoint.

print_server_response() {
  if [ -s "$BODY" ]; then
    python3 -m json.tool "$BODY" 2>/dev/null || cat "$BODY"
  else
    echo "      <empty>"
  fi
}

check() {
  local desc="$1" expected="$2" actual="$3"

  if [ "$actual" = "$expected" ]; then
    echo "PASS  $desc (status $actual)"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — expected $expected, got $actual"
    print_server_error
    FAIL=$((FAIL + 1))
  fi
}

check_contains() {
  local field="$1"
  local desc="$2"
  local actual

  actual=$(python3 -c "import json; print(json.load(open('$BODY')).get('$field',''))" 2>/dev/null)

  if [ -n "$actual" ]; then
    echo "PASS  $desc"
    print_server_response
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — field "$field" kosong/tidak ditemukan"
    print_server_response
    FAIL=$((FAIL + 1))
  fi
}

check_json_field() {
  local field="$1" expected="$2" desc="$3"
  local actual

  actual=$(python3 -c \
    "import json; print(json.load(open('$BODY'))['$field'])" \
    2>/dev/null)

  if [ "$actual" = "$expected" ]; then
    echo "PASS  $desc"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — expected \"$expected\", got \"$actual\""
    FAIL=$((FAIL + 1))
  fi
}

assert_yes() {
  local value="$1" desc="$2"

  if [ "$value" = "yes" ]; then
    echo "PASS  $desc"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc"
    FAIL=$((FAIL + 1))
  fi
}
