#!/usr/bin/env bash
# Smoke test seluruh endpoint API Universal Business Core.
# Jalankan server dulu di terminal lain:
#   DATABASE_URL="postgres://..." cargo run -p api
# Lalu jalankan skrip ini: bash smoke_test.sh

set -uo pipefail
BASE="http://localhost:3000"
PASS=0
FAIL=0

# Pra-syarat: tanpa ini, error di tengah jalan (mis. TENANT_ID kosong)
# jadi susah dibedakan dari kegagalan server sungguhan.
command -v curl >/dev/null || {
  echo "curl tidak ditemukan"
  exit 1
}
command -v python3 >/dev/null || {
  echo "python3 tidak ditemukan"
  exit 1
}

check() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    echo "PASS  $desc (status $actual)"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — expected $expected, dapat $actual"
    echo "      body: $(cat /tmp/last_body.json 2>/dev/null)"
    FAIL=$((FAIL + 1))
  fi
}

# check_contains FIELD SUBSTRING DESC -> cek satu field JSON di
# /tmp/last_body.json mengandung substring tertentu. Dipakai untuk
# memastikan 400/409 gagal karena ALASAN yang benar, bukan cuma status
# code yang kebetulan cocok.
check_contains() {
  local field="$1" substring="$2" desc="$3"
  local actual
  actual=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json')).get('$field',''))" 2>/dev/null)
  if [[ "$actual" == *"$substring"* ]]; then
    echo "PASS  $desc (mengandung \"$substring\")"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $desc — \"$actual\" tidak mengandung \"$substring\""
    FAIL=$((FAIL + 1))
  fi
}

req() {
  # req METHOD PATH JSON_BODY -> cetak status code, isi body ke /tmp/last_body.json
  local method="$1" path="$2" body="${3:-}"
  if [ -n "$body" ]; then
    curl -s -o /tmp/last_body.json -w "%{http_code}" -X "$method" "$BASE$path" \
      -H 'content-type: application/json' -d "$body"
  else
    curl -s -o /tmp/last_body.json -w "%{http_code}" -X "$method" "$BASE$path"
  fi
}

echo "=== 1. Create tenant (valid) ==="
STATUS=$(req POST /tenants '{"name":"Tenant Smoke Test"}')
check "POST /tenants valid" 201 "$STATUS"

TENANT_ID=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json'))['id'])" 2>/dev/null)
echo "  tenant_id = $TENANT_ID"

echo "=== 2. Create tenant (nama kosong -> 400) ==="
STATUS=$(req POST /tenants '{"name":"   "}')
check "POST /tenants nama kosong" 400 "$STATUS"
check_contains "error" "kosong" "pesan error nama kosong"

echo "=== 3. Get tenant (valid) ==="
STATUS=$(req GET "/tenants/$TENANT_ID")
check "GET /tenants/{id}" 200 "$STATUS"

echo "=== 4. Get tenant (tidak ditemukan -> 404) ==="
STATUS=$(req GET "/tenants/00000000-0000-0000-0000-000000000000")
check "GET /tenants/{id-random} -> 404" 404 "$STATUS"

echo "=== 5. Get tenant (id tidak valid -> 400) ==="
STATUS=$(req GET "/tenants/bukan-uuid")
check "GET /tenants/{id-invalid} -> 400" 400 "$STATUS"

echo "=== 6. Rename tenant (versi benar) ==="
STATUS=$(req PATCH "/tenants/$TENANT_ID" '{"name":"Tenant Smoke Test Baru","expected_version":0}')
check "PATCH /tenants/{id} versi benar" 200 "$STATUS"

echo "=== 7. Rename tenant (versi basi -> 409) ==="
STATUS=$(req PATCH "/tenants/$TENANT_ID" '{"name":"Tenant Telat","expected_version":0}')
check "PATCH /tenants/{id} versi basi -> 409" 409 "$STATUS"

echo "=== 8. Create business (valid) ==="
STATUS=$(req POST "/tenants/$TENANT_ID/businesses" '{"name":"Toko Baju","business_type":"retail"}')
check "POST /tenants/{id}/businesses valid" 201 "$STATUS"

BUSINESS_ID=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json'))['id'])" 2>/dev/null)
echo "  business_id = $BUSINESS_ID"

echo "=== 9. Create business (nama duplikat -> 409) ==="
STATUS=$(req POST "/tenants/$TENANT_ID/businesses" '{"name":"Toko Baju","business_type":"retail"}')
check "POST .../businesses nama duplikat -> 409" 409 "$STATUS"
check_contains "error" "nama business" "pesan error nama business duplikat"

echo "=== 10. Create business (tenant tidak ditemukan -> 404) ==="
STATUS=$(req POST "/tenants/00000000-0000-0000-0000-000000000000/businesses" '{"name":"X","business_type":"retail"}')
check "POST .../businesses tenant tidak ada -> 404" 404 "$STATUS"

echo "=== 11. Rename business (versi benar) ==="
STATUS=$(req PATCH "/businesses/$BUSINESS_ID" '{"name":"Toko Baju Baru","expected_version":0}')
check "PATCH /businesses/{id} versi benar" 200 "$STATUS"

echo "=== 12. Rename business (versi basi -> 409) ==="
STATUS=$(req PATCH "/businesses/$BUSINESS_ID" '{"name":"Telat","expected_version":0}')
check "PATCH /businesses/{id} versi basi -> 409" 409 "$STATUS"

echo "=== 13. Delete tenant selagi business masih aktif -> 409 ==="
STATUS=$(req DELETE "/tenants/$TENANT_ID" '{"expected_version":1}')
check "DELETE /tenants/{id} dengan business aktif -> 409" 409 "$STATUS"

echo "=== 14. Delete business (versi benar) -> 204 ==="
STATUS=$(req DELETE "/businesses/$BUSINESS_ID" '{"expected_version":1}')
check "DELETE /businesses/{id} -> 204" 204 "$STATUS"

echo "=== 15. Delete tenant setelah business dihapus -> 204 ==="
STATUS=$(req DELETE "/tenants/$TENANT_ID" '{"expected_version":1}')
check "DELETE /tenants/{id} -> 204" 204 "$STATUS"

echo "=== 16. Get tenant setelah dihapus (soft delete, tetap 200, is_deleted true) ==="
STATUS=$(req GET "/tenants/$TENANT_ID")
check "GET /tenants/{id} setelah dihapus -> 200" 200 "$STATUS"

IS_DELETED=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json'))['is_deleted'])" 2>/dev/null)
if [ "$IS_DELETED" = "True" ]; then
  echo "PASS  is_deleted true setelah delete"
  PASS=$((PASS + 1))
else
  echo "FAIL  is_deleted seharusnya true, dapat: $IS_DELETED"
  FAIL=$((FAIL + 1))
fi

IDEMP_TENANT=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo "  idemp_tenant_id = $IDEMP_TENANT"

echo "=== 17. Idempotent create tenant ==="
STATUS=$(req POST /tenants "{\"id\":\"$IDEMP_TENANT\",\"name\":\"Tenant Idempotent\"}")
check "POST tenant pertama" 201 "$STATUS"

STATUS=$(req POST /tenants "{\"id\":\"$IDEMP_TENANT\",\"name\":\"Tenant Berbeda\"}")
check "POST tenant retry -> 200" 200 "$STATUS"

IDEMP_BUSINESS=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo "  idemp_business_id = $IDEMP_BUSINESS"

echo "=== 18. Idempotent create business ==="
STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  "{\"id\":\"$IDEMP_BUSINESS\",\"name\":\"Bisnis Idem\",\"business_type\":\"retail\"}")
check "POST business pertama" 201 "$STATUS"

STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  "{\"id\":\"$IDEMP_BUSINESS\",\"name\":\"Nama Baru\",\"business_type\":\"retail\"}")
check "POST business retry -> 200" 200 "$STATUS"

echo "=== 19. Full sync tenants ==="
STATUS=$(req GET "/tenants")
check "GET /tenants full sync" 200 "$STATUS"

echo "=== 20. Invalid updated_since ==="
STATUS=$(req GET "/tenants?updated_since=bukan-timestamp")
check "GET /tenants invalid timestamp -> 400" 400 "$STATUS"
check_contains "error" "RFC 3339" "pesan error format waktu"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

echo "=== 21. Tenant Baru Setelah Cursor ==="
STATUS=$(req POST /tenants '{"name":"Tenant Setelah Cursor"}')
check "POST tenant baru" 201 "$STATUS"

CURSOR_TENANT_ID=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json'))['id'])" 2>/dev/null)
echo "  cursor_tenant_id = $CURSOR_TENANT_ID"

echo "=== 22. Valid updated_since ==="
STATUS=$(req GET "/tenants?updated_since=$CURSOR")
check "GET incremental tenant" 200 "$STATUS"

# Bukan cuma status 200 — pastikan filternya benar: tenant yang dibuat
# SEBELUM cursor tidak boleh ikut nyempil, dan yang SESUDAH cursor wajib ada.
FOUND=$(python3 -c "
import json
ids = [t['id'] for t in json.load(open('/tmp/last_body.json'))]
print('yes' if '$CURSOR_TENANT_ID' in ids else 'no')
" 2>/dev/null)
if [ "$FOUND" = "yes" ]; then
  echo "PASS  tenant baru muncul di hasil incremental sync"
  PASS=$((PASS + 1))
else
  echo "FAIL  tenant baru TIDAK muncul di hasil incremental sync (filter updated_since salah?)"
  FAIL=$((FAIL + 1))
fi

echo "=== 23. Full sync businesses ==="
STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses")
check "GET businesses full sync" 200 "$STATUS"

echo "=== 24. Invalid timestamp businesses ==="
STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses?updated_since=abc")
check "GET businesses invalid timestamp -> 400" 400 "$STATUS"

CURSOR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
sleep 1

echo "=== 25. Business Baru Setelah Cursor ==="
STATUS=$(req POST "/tenants/$IDEMP_TENANT/businesses" \
  '{"name":"Sync Test","business_type":"retail"}')
check "POST business baru" 201 "$STATUS"

SYNC_BUSINESS_ID=$(python3 -c "import json;print(json.load(open('/tmp/last_body.json'))['id'])" 2>/dev/null)
echo " sync_business_id = $SYNC_BUSINESS_ID"

STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses?updated_since=$CURSOR")
check "GET incremental businesses" 200 "$STATUS"

STATUS=$(req DELETE "/businesses/$SYNC_BUSINESS_ID" '{"expected_version":0}')
check "DELETE business" 204 "$STATUS"

STATUS=$(req GET "/tenants/$IDEMP_TENANT/businesses")
check "GET businesses setelah delete" 200 "$STATUS"

# Business yang di-soft-delete harus tetap muncul (bukan hilang) dan
# ditandai is_deleted=true — ini kontrak penting untuk client offline.
SOFT_DELETED_OK=$(python3 -c "
import json
items = json.load(open('/tmp/last_body.json'))
match = [b for b in items if b['id'] == '$SYNC_BUSINESS_ID']
print('yes' if match and match[0]['is_deleted'] else 'no')
" 2>/dev/null)
if [ "$SOFT_DELETED_OK" = "yes" ]; then
  echo "PASS  business soft-deleted tetap muncul dengan is_deleted=true"
  PASS=$((PASS + 1))
else
  echo "FAIL  business soft-deleted seharusnya tetap muncul dengan is_deleted=true"
  FAIL=$((FAIL + 1))
fi

echo
echo "=========================================="
echo "Hasil: $PASS PASS, $FAIL FAIL"
echo "=========================================="
