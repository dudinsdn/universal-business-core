#!/usr/bin/env bash
# Generic HTTP/JSON helpers.
req() {
  # req METHOD PATH [JSON_BODY]
  # Mengembalikan HTTP status dan menyimpan body ke $BODY.
  local method="$1"
  local path="$2"
  local body="${3:-}"

  if [ -n "$body" ]; then
    curl -sS -o "$BODY" -w "%{http_code}" \
      -X "$method" "$BASE$path" \
      -H 'content-type: application/json' \
      -d "$body"
  else
    curl -sS -o "$BODY" -w "%{http_code}" \
      -X "$method" "$BASE$path"
  fi
}

json_value() {
  local field="$1"
  python3 -c \
    "import json; print(json.load(open('$BODY'))['$field'])" \
    2>/dev/null
}

contains_id() {
  local id="$1"

  python3 - "$id" <<'PY'
import json
import sys

wanted = sys.argv[1]

try:
    data = json.load(open("/tmp/ubc_smoke_test_body.json"))
    print("yes" if any(item.get("id") == wanted for item in data) else "no")
except Exception:
    print("no")
PY
}

deleted_id_is_true() {
  local id="$1"

  python3 - "$id" <<'PY'
import json
import sys

wanted = sys.argv[1]

try:
    data = json.load(open("/tmp/ubc_smoke_test_body.json"))
    match = [item for item in data if item.get("id") == wanted]
    print("yes" if match and match[0].get("is_deleted") is True else "no")
except Exception:
    print("no")
PY
}
