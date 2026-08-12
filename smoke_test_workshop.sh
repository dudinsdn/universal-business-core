#!/usr/bin/env bash
# Smoke test khusus Capability Workshop (ServiceOrder).
#
# Terpisah dari smoke_test.sh (yang khusus Core) — konsisten dengan
# prinsip proyek: Capability terpisah dari Core, jadi test-nya pun
# terpisah. Kalau nanti ada Capability lain (Laundry, Klinik, dst),
# masing-masing dapat file sendiri dengan pola yang sama.
#
# Jalankan server dulu di terminal lain:
#   DATABASE_URL="postgres://..." cargo run -p api
#
# Lalu:
#   bash smoke_test_workshop.sh
#
# Cakupan:
# - Setup: Tenant, Business (jenis "workshop"), Customer
# - ServiceOrder: create, validasi deskripsi kosong, business tidak
#   ditemukan, idempotency
# - Siklus status: start, complete (dengan/tanpa link Transaction),
#   cancel, beserta transisi yang ditolak
# - Optimistic locking (versi basi)
# - Soft delete
# - Incremental sync (updated_since), termasuk soft-deleted
# - Business dihapus -> create ServiceOrder ditolak

set -uo pipefail

BASE="${BASE:-http://localhost:3000}"
PASS=0
FAIL=0
BODY="/tmp/ubc_smoke_test_workshop_body.json"

command -v curl >/dev/null || {
  echo "curl tidak ditemukan"
  exit 1
}
command -v python3 >/dev/null || {
  echo "python3 tidak ditemukan"
  exit 1
}

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

check() {
  local desc="$1" expected="$2" actual="$3"

  if [ "$actual" = "$expected" ]; then
    echo "PASS  $desc (status $actual)"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — expected $expected, dapat $actual"
    echo "      body: $(cat "$BODY" 2>/dev/null)"
    FAIL=$((FAIL + 1))
  fi
}

check_contains() {
  local field="$1" substring="$2" desc="$3"
  local actual

  actual=$(python3 -c \
    "import json; print(json.load(open('$BODY')).get('$field',''))" \
    2>/dev/null)

  if [[ "$actual" == *"$substring"* ]]; then
    echo "PASS  $desc (mengandung \"$substring\")"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — \"$actual\" tidak mengandung \"$substring\""
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
    echo "FAIL  $desc — expected \"$expected\", dapat \"$actual\""
    FAIL=$((FAIL + 1))
  fi
}

json_value() {
  local field="$1"
  python3 -c \
    "import json; print(json.load(open('$BODY'))['$field'])" \
    2>/dev/null
}

req() {
  # req METHOD PATH [JSON_BODY]
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

contains_id() {
  local id="$1"

  python3 - "$id" <<'PY'
import json
import sys

wanted = sys.argv[1]

try:
    data = json.load(open("/tmp/ubc_smoke_test_workshop_body.json"))
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
    data = json.load(open("/tmp/ubc_smoke_test_workshop_body.json"))
    match = [item for item in data if item.get("id") == wanted]
    print("yes" if match and match[0].get("is_deleted") is True else "no")
except Exception:
    print("no")
PY
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

echo "=========================================="
echo "Capability Workshop — Smoke Test"
echo "BASE=$BASE"
echo "=========================================="

echo
echo "=== 0. SETUP (Tenant, Business, Customer) ==="

STATUS=$(req POST /tenants '{"name":"Tenant Bengkel"}')
check "POST /tenants" 201 "$STATUS"
TENANT_ID=$(json_value id)
echo "  tenant_id = $TENANT_ID"

STATUS=$(req POST "/tenants/$TENANT_ID/businesses" \
  '{"name":"Bengkel Jaya","business_type":"workshop"}')
check "POST business (jenis workshop)" 201 "$STATUS"
BUSINESS_ID=$(json_value id)
echo "  business_id = $BUSINESS_ID"

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" '{"name":"Budi Santoso"}')
check "POST customer" 201 "$STATUS"
CUSTOMER_ID=$(json_value id)
echo "  customer_id = $CUSTOMER_ID"

echo
echo "=== 1. CREATE SERVICE ORDER ==="

echo "--- Create valid ---"
STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"Ganti oli dan servis rem"}')
check "POST /businesses/{id}/service-orders valid" 201 "$STATUS"
check_json_field "status" "received" "status awal 'received'"
check_json_field "transaction_id" "None" "transaction_id kosong di awal"
SERVICE_ORDER_ID=$(json_value id)
echo "  service_order_id = $SERVICE_ORDER_ID"

echo "--- Create deskripsi kosong ---"
STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"   "}')
check "POST service-orders deskripsi kosong -> 400" 400 "$STATUS"
check_contains "error" "kosong" "pesan error deskripsi kosong"

echo "--- Create business tidak ditemukan ---"
STATUS=$(req POST \
  "/businesses/00000000-0000-0000-0000-000000000000/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"X"}')
check "POST service-orders business tidak ada -> 404" 404 "$STATUS"

echo "--- Idempotent create ---"
IDEMP_ID=$(python3 -c 'import uuid; print(uuid.uuid4())')
STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"id":"'"$IDEMP_ID"'","customer_id":"'"$CUSTOMER_ID"'","description":"Servis AC"}')
check "POST service-order idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"id":"'"$IDEMP_ID"'","customer_id":"'"$CUSTOMER_ID"'","description":"Deskripsi lain"}')
check "POST service-order retry -> 200" 200 "$STATUS"
check_json_field "description" "Servis AC" \
  "retry service-order mengembalikan entity pertama"

echo
echo "=== 2. SIKLUS STATUS ==="

echo "--- Complete langsung dari 'received' (tanpa start) ditolak ---"
STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/complete" \
  '{"expected_version":0}')
check "PATCH complete dari 'received' -> 409" 409 "$STATUS"
check_contains "error" "tidak bisa mengubah status" \
  "pesan error transisi tidak valid"

echo "--- Start versi basi ---"
STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/start" \
  '{"expected_version":99}')
check "PATCH start versi basi -> 409" 409 "$STATUS"

echo "--- Start versi benar ---"
STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/start" \
  '{"expected_version":0}')
check "PATCH start versi benar -> 200" 200 "$STATUS"
check_json_field "status" "in_progress" "status berubah jadi 'in_progress'"

echo "--- Buat Transaction untuk ditautkan ---"
STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"service","amount":150000}')
check "POST transaction untuk penagihan servis" 201 "$STATUS"
TRANSACTION_ID=$(json_value id)

echo "--- Complete dengan link transaction_id ---"
STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/complete" \
  '{"expected_version":1,"transaction_id":"'"$TRANSACTION_ID"'"}')
check "PATCH complete dengan transaction_id -> 200" 200 "$STATUS"
check_json_field "status" "completed" "status berubah jadi 'completed'"
check_json_field "transaction_id" "$TRANSACTION_ID" \
  "transaction_id tertaut ke service order"

echo "--- Cancel setelah completed ditolak ---"
STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/cancel" \
  '{"expected_version":2}')
check "PATCH cancel dari 'completed' -> 409" 409 "$STATUS"

echo "--- Cancel dari 'received' (service order baru) ---"
STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"Servis yang dibatalkan"}')
CANCEL_TARGET_ID=$(json_value id)

STATUS=$(req PATCH "/service-orders/$CANCEL_TARGET_ID/cancel" \
  '{"expected_version":0}')
check "PATCH cancel dari 'received' -> 200" 200 "$STATUS"
check_json_field "status" "cancelled" "status berubah jadi 'cancelled'"

echo
echo "=== 3. SOFT DELETE & SYNC ==="

STATUS=$(req GET "/businesses/$BUSINESS_ID/service-orders")
check "GET service-orders full sync" 200 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/service-orders?updated_since=bukan-tanggal")
check "GET service-orders invalid timestamp -> 400" 400 "$STATUS"

STATUS=$(req GET \
  "/businesses/00000000-0000-0000-0000-000000000000/service-orders")
check "GET service-orders business tidak ada -> 404" 404 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"Servis untuk dihapus"}')
check "POST service-order setelah cursor" 201 "$STATUS"
SYNC_TARGET_ID=$(json_value id)

STATUS=$(req GET \
  "/businesses/$BUSINESS_ID/service-orders?updated_since=$CURSOR")
check "GET incremental service-orders" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_TARGET_ID")" \
  "service order baru muncul pada incremental sync"

STATUS=$(req DELETE "/service-orders/$SYNC_TARGET_ID" '{"expected_version":0}')
check "DELETE service-order -> 204" 204 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/service-orders")
check "GET service-orders setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_TARGET_ID")" \
  "service order soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 4. BUSINESS DIHAPUS -> CREATE SERVICE ORDER DITOLAK ==="

# Selesaikan/hapus dulu semua entity aktif di bawah business supaya boleh
# dihapus — pola sama seperti smoke_test.sh bagian 14.
req DELETE "/service-orders/$SERVICE_ORDER_ID" '{"expected_version":2}' >/dev/null
req DELETE "/service-orders/$CANCEL_TARGET_ID" '{"expected_version":1}' >/dev/null
req DELETE "/customers/$CUSTOMER_ID" '{"expected_version":0}' >/dev/null
req DELETE "/transactions/$TRANSACTION_ID" '{"expected_version":0}' >/dev/null

STATUS=$(req DELETE "/businesses/$BUSINESS_ID" '{"expected_version":0}')
check "DELETE business -> 204" 204 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"Harusnya ditolak"}')
check "POST service-order pada business terhapus -> 409" 409 "$STATUS"
check_contains "error" "business sudah dihapus" \
  "pesan error business terhapus"

echo
echo "=========================================="
echo "Hasil: $PASS PASS, $FAIL FAIL"
echo "=========================================="

if [ "$FAIL" -eq 0 ]; then
  exit 0
else
  exit 1
fi
