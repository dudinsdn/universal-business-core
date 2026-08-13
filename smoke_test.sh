#!/usr/bin/env bash
# Smoke test seluruh endpoint API Universal Business Core.
#
# Jalankan server dulu di terminal lain:
#   DATABASE_URL="postgres://..." cargo run -p api
#
# Lalu:
#   bash smoke_test.sh
#
# Cakupan:
# - Tenant: CRUD, soft delete, optimistic locking, idempotency, sync
# - Business: create/update/delete, duplicate name, optimistic locking,
#             idempotency, soft delete, sync
# - Customer: create, rename, phone update/clear, optimistic locking,
#             idempotency, soft delete, sync
# - Transaction: create, customer link, validation, idempotency,
#                soft delete, sync
# - Relationship: create, self-relationship rejection, validation,
#                 idempotency, soft delete, sync
# - Interaction: create, note opsional, validation, idempotency,
#                soft delete, sync

set -uo pipefail

BASE="${BASE:-http://localhost:3000}"
PASS=0
FAIL=0
BODY="/tmp/ubc_smoke_test_body.json"

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
    echo "FAIL  $desc — expected $expected, got $actual"
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
    echo "PASS  $desc (contain \"$substring\")"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — \"$actual\" exclude \"$substring\""
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

json_value() {
  local field="$1"
  python3 -c \
    "import json; print(json.load(open('$BODY'))['$field'])" \
    2>/dev/null
}

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
echo "Universal Business Core — Smoke Test"
echo "BASE=$BASE"
echo "=========================================="

echo
echo "=== 1. TENANT ==="

STATUS=$(req POST /tenants '{"name":"Tenant Smoke Test"}')
check "POST /tenants valid" 201 "$STATUS"
TENANT_ID=$(json_value id)
echo "  tenant_id = $TENANT_ID"

STATUS=$(req POST /tenants '{"name":"   "}')
check "POST /tenants nama kosong -> 400" 400 "$STATUS"
check_contains "error" "kosong" "pesan error nama kosong"

STATUS=$(req GET "/tenants/$TENANT_ID")
check "GET /tenants/{id}" 200 "$STATUS"

STATUS=$(req GET "/tenants/00000000-0000-0000-0000-000000000000")
check "GET /tenants/{id-random} -> 404" 404 "$STATUS"

STATUS=$(req GET "/tenants/bukan-uuid")
check "GET /tenants/{id-invalid} -> 400" 400 "$STATUS"

STATUS=$(req PATCH "/tenants/$TENANT_ID" \
  '{"name":"Tenant Smoke Test Baru","expected_version":0}')
check "PATCH /tenants/{id} versi benar" 200 "$STATUS"

STATUS=$(req PATCH "/tenants/$TENANT_ID" \
  '{"name":"Tenant Telat","expected_version":0}')
check "PATCH /tenants/{id} versi basi -> 409" 409 "$STATUS"
check_contains "error" "versi" "pesan error optimistic locking tenant"

echo
echo "=== 2. BUSINESS ==="

STATUS=$(req POST "/tenants/$TENANT_ID/businesses" \
  '{"name":"Toko Baju","business_type":"retail"}')
check "POST /tenants/{id}/businesses valid" 201 "$STATUS"
BUSINESS_ID=$(json_value id)
echo "  business_id = $BUSINESS_ID"

STATUS=$(req POST "/tenants/$TENANT_ID/businesses" \
  '{"name":"Toko Baju","business_type":"retail"}')
check "POST .../businesses nama duplikat -> 409" 409 "$STATUS"
check_contains "error" "nama business" "pesan error nama business duplikat"

STATUS=$(req POST \
  "/tenants/00000000-0000-0000-0000-000000000000/businesses" \
  '{"name":"X","business_type":"retail"}')
check "POST .../businesses tenant tidak ada -> 404" 404 "$STATUS"

STATUS=$(req PATCH "/businesses/$BUSINESS_ID" \
  '{"name":"Toko Baju Baru","expected_version":0}')
check "PATCH /businesses/{id} versi benar" 200 "$STATUS"

STATUS=$(req PATCH "/businesses/$BUSINESS_ID" \
  '{"name":"Telat","expected_version":0}')
check "PATCH /businesses/{id} versi basi -> 409" 409 "$STATUS"

echo
echo "=== 3. CUSTOMER ==="

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" \
  '{"name":"Budi Santoso","phone":"081234567890"}')
check "POST /businesses/{id}/customers valid" 201 "$STATUS"
CUSTOMER_ID=$(json_value id)
echo "  customer_id = $CUSTOMER_ID"

check_json_field "phone" "081234567890" \
  "customer menyimpan nomor telepon"

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" \
  '{"name":"   "}')
check "POST customer nama kosong -> 400" 400 "$STATUS"
check_contains "error" "kosong" "pesan error nama customer kosong"

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" \
  '{"name":"Customer Invalid Phone","phone":"abc"}')
check "POST customer phone invalid -> 400" 400 "$STATUS"

STATUS=$(req PATCH "/customers/$CUSTOMER_ID" \
  '{"name":"Budi Santoso Baru","expected_version":0}')
check "PATCH /customers/{id} versi benar" 200 "$STATUS"

STATUS=$(req PATCH "/customers/$CUSTOMER_ID" \
  '{"name":"Budi Telat","expected_version":0}')
check "PATCH /customers/{id} versi basi -> 409" 409 "$STATUS"
check_contains "error" "versi" "pesan error optimistic locking customer"

STATUS=$(req PATCH "/customers/$CUSTOMER_ID/phone" \
  '{"phone":"081298765432","expected_version":1}')
check "PATCH /customers/{id}/phone versi benar" 200 "$STATUS"
check_json_field "phone" "081298765432" \
  "nomor telepon customer berhasil diubah"

STATUS=$(req PATCH "/customers/$CUSTOMER_ID/phone" \
  '{"phone":"081211111111","expected_version":1}')
check "PATCH /customers/{id}/phone versi basi -> 409" 409 "$STATUS"

STATUS=$(req PATCH "/customers/$CUSTOMER_ID/phone" \
  '{"phone":null,"expected_version":2}')
check "PATCH /customers/{id}/phone null -> 200" 200 "$STATUS"
check_json_field "phone" "None" \
  "nomor telepon customer berhasil dihapus"

echo
echo "=== 4. TRANSACTION ==="

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "kind":"sale",
    "amount":50000,
    "occurred_at":"2026-08-08T00:00:00Z"
  }')
check "POST /businesses/{id}/transactions valid" 201 "$STATUS"
TRANSACTION_ID=$(json_value id)
echo "  transaction_id = $TRANSACTION_ID"

check_json_field "kind" "sale" \
  "transaction kind tersimpan sebagai lowercase"
check_json_field "amount" "50000" \
  "transaction amount tersimpan"
check_json_field "customer_id" "$CUSTOMER_ID" \
  "transaction terhubung ke customer"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"   ","amount":10000}')
check "POST transaction kind kosong -> 400" 400 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"sale online","amount":10000}')
check "POST transaction kind invalid -> 400" 400 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"sale","amount":0}')
check "POST transaction amount 0 -> 400" 400 "$STATUS"
check_contains "error" "lebih besar dari nol" \
  "pesan error amount tidak valid"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"sale","amount":-1}')
check "POST transaction amount negatif -> 400" 400 "$STATUS"

echo
echo "=== 5. IDEMPOTENCY ==="

IDEMP_TENANT=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo "  idemp_tenant_id = $IDEMP_TENANT"

STATUS=$(req POST /tenants \
  '{"id":"'"$IDEMP_TENANT"'","name":"Tenant Idempotent"}')
check "POST tenant idempotent pertama" 201 "$STATUS"

STATUS=$(req POST /tenants \
  '{"id":"'"$IDEMP_TENANT"'","name":"Tenant Berbeda"}')
check "POST tenant retry -> 200" 200 "$STATUS"
check_json_field "name" "Tenant Idempotent" \
  "retry tenant mengembalikan entity pertama"

IDEMP_BUSINESS=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo "  idemp_business_id = $IDEMP_BUSINESS"

STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  '{"id":"'"$IDEMP_BUSINESS"'","name":"Bisnis Idem","business_type":"retail"}')
check "POST business idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  '{"id":"'"$IDEMP_BUSINESS"'","name":"Nama Baru","business_type":"retail"}')
check "POST business retry -> 200" 200 "$STATUS"
check_json_field "name" "Bisnis Idem" \
  "retry business mengembalikan entity pertama"

IDEMP_CUSTOMER=$(python3 -c 'import uuid; print(uuid.uuid4())')

STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  '{"name":"Bisnis Customer Idem","business_type":"retail"}')
check "POST business untuk customer idempotency" 201 "$STATUS"
IDEMP_BUSINESS_FOR_CUSTOMER=$(json_value id)

STATUS=$(req POST "/businesses/$IDEMP_BUSINESS_FOR_CUSTOMER/customers" \
  '{"id":"'"$IDEMP_CUSTOMER"'","name":"Customer Idem","phone":"081234567890"}')
check "POST customer idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/businesses/$IDEMP_BUSINESS_FOR_CUSTOMER/customers" \
  '{"id":"'"$IDEMP_CUSTOMER"'","name":"Customer Berbeda"}')
check "POST customer retry -> 200" 200 "$STATUS"
check_json_field "name" "Customer Idem" \
  "retry customer mengembalikan entity pertama"

IDEMP_TRANSACTION=$(python3 -c 'import uuid; print(uuid.uuid4())')

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"id":"'"$IDEMP_TRANSACTION"'","kind":"sale","amount":100000}')
check "POST transaction idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"id":"'"$IDEMP_TRANSACTION"'","kind":"sale","amount":999999}')
check "POST transaction retry -> 200" 200 "$STATUS"
check_json_field "amount" "100000" \
  "retry transaction mengembalikan amount pertama"

echo
echo "=== 6. SYNC TENANT ==="

STATUS=$(req GET "/tenants")
check "GET /tenants full sync" 200 "$STATUS"

STATUS=$(req GET "/tenants?updated_since=bukan-timestamp")
check "GET /tenants invalid timestamp -> 400" 400 "$STATUS"
check_contains "error" "RFC 3339" \
  "pesan error timestamp tenant"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST /tenants '{"name":"Tenant Setelah Cursor"}')
check "POST tenant setelah cursor" 201 "$STATUS"
CURSOR_TENANT_ID=$(json_value id)

STATUS=$(req GET "/tenants?updated_since=$CURSOR")
check "GET incremental tenant" 200 "$STATUS"
assert_yes "$(contains_id "$CURSOR_TENANT_ID")" \
  "tenant baru muncul pada incremental sync"

echo
echo "=== 7. SYNC BUSINESS ==="

STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses")
check "GET businesses full sync" 200 "$STATUS"

STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses?updated_since=abc")
check "GET businesses invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  '{"name":"Sync Business","business_type":"retail"}')
check "POST business setelah cursor" 201 "$STATUS"
SYNC_BUSINESS_ID=$(json_value id)

STATUS=$(req GET \
  "/tenants/$IDEMP_TENANT/businesses?updated_since=$CURSOR")
check "GET incremental businesses" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_BUSINESS_ID")" \
  "business baru muncul pada incremental sync"

STATUS=$(req DELETE "/businesses/$SYNC_BUSINESS_ID" \
  '{"expected_version":0}')
check "DELETE business -> 204" 204 "$STATUS"

STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses")
check "GET businesses setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_BUSINESS_ID")" \
  "business soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 8. SYNC CUSTOMER ==="

STATUS=$(req GET "/businesses/$BUSINESS_ID/customers")
check "GET customers full sync" 200 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/customers?updated_since=abc")
check "GET customers invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" \
  '{"name":"Customer Sync Test","phone":"081200000000"}')
check "POST customer setelah cursor" 201 "$STATUS"
SYNC_CUSTOMER_ID=$(json_value id)

STATUS=$(req GET \
  "/businesses/$BUSINESS_ID/customers?updated_since=$CURSOR")
check "GET incremental customers" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_CUSTOMER_ID")" \
  "customer baru muncul pada incremental sync"

STATUS=$(req DELETE "/customers/$SYNC_CUSTOMER_ID" \
  '{"expected_version":0}')
check "DELETE customer -> 204" 204 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/customers")
check "GET customers setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_CUSTOMER_ID")" \
  "customer soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 9. SYNC TRANSACTION ==="

STATUS=$(req GET "/businesses/$BUSINESS_ID/transactions")
check "GET transactions full sync" 200 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/transactions?updated_since=abc")
check "GET transactions invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"sale","amount":75000,"occurred_at":"2026-08-08T01:00:00Z"}')
check "POST transaction setelah cursor" 201 "$STATUS"
SYNC_TRANSACTION_ID=$(json_value id)

STATUS=$(req GET \
  "/businesses/$BUSINESS_ID/transactions?updated_since=$CURSOR")
check "GET incremental transactions" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_TRANSACTION_ID")" \
  "transaction baru muncul pada incremental sync"

STATUS=$(req DELETE "/transactions/$SYNC_TRANSACTION_ID" \
  '{"expected_version":0}')
check "DELETE transaction -> 204" 204 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/transactions")
check "GET transactions setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_TRANSACTION_ID")" \
  "transaction soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 10. RELATIONSHIP ==="

STATUS=$(req POST "/businesses/$BUSINESS_ID/customers" \
  '{"name":"Ani Wijaya"}')
check "POST customer kedua untuk relationship" 201 "$STATUS"
CUSTOMER_B_ID=$(json_value id)
echo "  customer_b_id = $CUSTOMER_B_ID"

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"sibling"
  }')
check "POST /businesses/{id}/relationships valid" 201 "$STATUS"
RELATIONSHIP_ID=$(json_value id)
echo "  relationship_id = $RELATIONSHIP_ID"

check_json_field "relationship_type" "sibling" \
  "relationship_type tersimpan sebagai lowercase"
check_json_field "from_customer_id" "$CUSTOMER_ID" \
  "relationship from_customer_id sesuai"
check_json_field "to_customer_id" "$CUSTOMER_B_ID" \
  "relationship to_customer_id sesuai"

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_ID"'",
    "relationship_type":"sibling"
  }')
check "POST relationship self -> 409" 409 "$STATUS"
check_contains "error" "tidak bisa berelasi dengan dirinya sendiri" \
  "pesan error self-relationship"

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"   "
  }')
check "POST relationship jenis kosong -> 400" 400 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"family member!"
  }')
check "POST relationship jenis invalid -> 400" 400 "$STATUS"

STATUS=$(req POST \
  "/businesses/00000000-0000-0000-0000-000000000000/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"sibling"
  }')
check "POST relationship business tidak ada -> 404" 404 "$STATUS"

IDEMP_RELATIONSHIP=$(python3 -c 'import uuid; print(uuid.uuid4())')

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "id":"'"$IDEMP_RELATIONSHIP"'",
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"referral"
  }')
check "POST relationship idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "id":"'"$IDEMP_RELATIONSHIP"'",
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"guardian"
  }')
check "POST relationship retry -> 200" 200 "$STATUS"
check_json_field "relationship_type" "referral" \
  "retry relationship mengembalikan entity pertama"

echo
echo "=== 11. SYNC RELATIONSHIP ==="

STATUS=$(req GET "/businesses/$BUSINESS_ID/relationships")
check "GET relationships full sync" 200 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/relationships?updated_since=abc")
check "GET relationships invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/businesses/$BUSINESS_ID/relationships" \
  '{
    "from_customer_id":"'"$CUSTOMER_ID"'",
    "to_customer_id":"'"$CUSTOMER_B_ID"'",
    "relationship_type":"guardian"
  }')
check "POST relationship setelah cursor" 201 "$STATUS"
SYNC_RELATIONSHIP_ID=$(json_value id)

STATUS=$(req GET \
  "/businesses/$BUSINESS_ID/relationships?updated_since=$CURSOR")
check "GET incremental relationships" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_RELATIONSHIP_ID")" \
  "relationship baru muncul pada incremental sync"

STATUS=$(req DELETE "/relationships/$SYNC_RELATIONSHIP_ID" \
  '{"expected_version":0}')
check "DELETE relationship -> 204" 204 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/relationships")
check "GET relationships setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_RELATIONSHIP_ID")" \
  "relationship soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 12. INTERACTION ==="

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"call",
    "note":"Follow up jadwal kontrol",
    "occurred_at":"2026-08-08T02:00:00Z"
  }')
check "POST /businesses/{id}/interactions valid" 201 "$STATUS"
INTERACTION_ID=$(json_value id)
echo "  interaction_id = $INTERACTION_ID"

check_json_field "interaction_type" "call" \
  "interaction_type tersimpan sebagai lowercase"
check_json_field "customer_id" "$CUSTOMER_ID" \
  "interaction terhubung ke customer"
check_json_field "note" "Follow up jadwal kontrol" \
  "interaction note tersimpan"

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"visit"
  }')
check "POST interaction tanpa note -> 201" 201 "$STATUS"
check_json_field "note" "None" \
  "interaction tanpa note tersimpan sebagai null"

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"call",
    "note":"   "
  }')
check "POST interaction note kosong -> 400" 400 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"   "
  }')
check "POST interaction jenis kosong -> 400" 400 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"phone call!"
  }')
check "POST interaction jenis invalid -> 400" 400 "$STATUS"

STATUS=$(req POST \
  "/businesses/00000000-0000-0000-0000-000000000000/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"call"
  }')
check "POST interaction business tidak ada -> 404" 404 "$STATUS"

IDEMP_INTERACTION=$(python3 -c 'import uuid; print(uuid.uuid4())')

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "id":"'"$IDEMP_INTERACTION"'",
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"call"
  }')
check "POST interaction idempotent pertama" 201 "$STATUS"

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "id":"'"$IDEMP_INTERACTION"'",
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"visit"
  }')
check "POST interaction retry -> 200" 200 "$STATUS"
check_json_field "interaction_type" "call" \
  "retry interaction mengembalikan entity pertama"

echo
echo "=== 13. SYNC INTERACTION ==="

STATUS=$(req GET "/businesses/$BUSINESS_ID/interactions")
check "GET interactions full sync" 200 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/interactions?updated_since=abc")
check "GET interactions invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

STATUS=$(req POST "/businesses/$BUSINESS_ID/interactions" \
  '{
    "customer_id":"'"$CUSTOMER_ID"'",
    "interaction_type":"email"
  }')
check "POST interaction setelah cursor" 201 "$STATUS"
SYNC_INTERACTION_ID=$(json_value id)

STATUS=$(req GET \
  "/businesses/$BUSINESS_ID/interactions?updated_since=$CURSOR")
check "GET incremental interactions" 200 "$STATUS"
assert_yes "$(contains_id "$SYNC_INTERACTION_ID")" \
  "interaction baru muncul pada incremental sync"

STATUS=$(req DELETE "/interactions/$SYNC_INTERACTION_ID" \
  '{"expected_version":0}')
check "DELETE interaction -> 204" 204 "$STATUS"

STATUS=$(req GET "/businesses/$BUSINESS_ID/interactions")
check "GET interactions setelah soft delete" 200 "$STATUS"
assert_yes "$(deleted_id_is_true "$SYNC_INTERACTION_ID")" \
  "interaction soft-deleted tetap muncul dengan is_deleted=true"

echo
echo "=== 14. SOFT DELETE / OPTIMISTIC LOCKING ==="

STATUS=$(req DELETE "/tenants/$TENANT_ID" \
  '{"expected_version":1}')
check "DELETE tenant dengan business aktif -> 409" 409 "$STATUS"

STATUS=$(req DELETE "/businesses/$BUSINESS_ID" \
  '{"expected_version":1}')
check "DELETE /businesses/{id} -> 204" 204 "$STATUS"

STATUS=$(req DELETE "/tenants/$TENANT_ID" \
  '{"expected_version":1}')
check "DELETE /tenants/{id} -> 204" 204 "$STATUS"

STATUS=$(req GET "/tenants/$TENANT_ID")
check "GET tenant setelah soft delete -> 200" 200 "$STATUS"
check_json_field "is_deleted" "True" \
  "tenant ditandai is_deleted=true"

echo
echo "=========================================="
echo "Hasil: $PASS PASS, $FAIL FAIL"
echo "=========================================="

if [ "$FAIL" -eq 0 ]; then
  exit 0
else
  exit 1
fi
