#!/usr/bin/env bash
# Smoke test khusus Capability Workshop (ServiceOrder).
#
# Terpisah dari smoke_test_core.sh (yang khusus Core) — konsisten dengan
# prinsip proyek: Capability terpisah dari Core, jadi test-nya pun
# terpisah. Kalau nanti ada Capability lain (Laundry, Klinik, dst),
# masing-masing dapat file sendiri dengan pola yang sama.
#
# Jalankan server dulu di terminal lain:
#   DATABASE_URL="postgres://..." cargo run -p api
#
# Lalu:
#   bash workshop.sh
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
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$ROOT/lib/header.sh"

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

STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"Ganti oli dan servis rem"}')
check "POST /businesses/{id}/service-orders valid" 201 "$STATUS"
check_json_field "status" "received" "status awal 'received'"
check_json_field "transaction_id" "None" "transaction_id kosong di awal"
SERVICE_ORDER_ID=$(json_value id)
echo "  service_order_id = $SERVICE_ORDER_ID"

STATUS=$(req POST "/businesses/$BUSINESS_ID/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"   "}')
check "POST service-orders deskripsi kosong -> 400" 400 "$STATUS"
check_contains "error" "pesan error deskripsi kosong"

STATUS=$(req POST \
  "/businesses/00000000-0000-0000-0000-000000000000/service-orders" \
  '{"customer_id":"'"$CUSTOMER_ID"'","description":"X"}')
check "POST service-orders business tidak ada -> 404" 404 "$STATUS"

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

STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/complete" \
  '{"expected_version":0}')
check "PATCH complete dari 'received' -> 409" 409 "$STATUS"
check_contains "error" "tidak bisa mengubah status" \
  "pesan error transisi tidak valid"

STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/start" \
  '{"expected_version":99}')
check "PATCH start versi basi -> 409" 409 "$STATUS"

STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/start" \
  '{"expected_version":0}')
check "PATCH start versi benar -> 200" 200 "$STATUS"
check_json_field "status" "in_progress" "status berubah jadi 'in_progress'"

STATUS=$(req POST "/businesses/$BUSINESS_ID/transactions" \
  '{"kind":"service","amount":150000}')
check "POST transaction untuk penagihan servis" 201 "$STATUS"
TRANSACTION_ID=$(json_value id)

STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/complete" \
  '{"expected_version":1,"transaction_id":"'"$TRANSACTION_ID"'"}')
check "PATCH complete dengan transaction_id -> 200" 200 "$STATUS"
check_json_field "status" "completed" "status berubah jadi 'completed'"
check_json_field "transaction_id" "$TRANSACTION_ID" \
  "transaction_id tertaut ke service order"

STATUS=$(req PATCH "/service-orders/$SERVICE_ORDER_ID/cancel" \
  '{"expected_version":2}')
check "PATCH cancel dari 'completed' -> 409" 409 "$STATUS"

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

source "$ROOT/lib/footer.sh"
