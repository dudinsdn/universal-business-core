CREATE TABLE transactions (
    id UUID PRIMARY KEY,
    business_id UUID NOT NULL REFERENCES businesses (id),
    customer_id UUID REFERENCES customers (id),
    kind TEXT NOT NULL,
    amount BIGINT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Tidak ada unique constraint apa pun — Transaction tidak punya business
-- rule keunikan (beda dari ux_businesses_tenant_name_active). Validasi
-- amount > 0 sudah dijamin oleh TransactionAmount di domain layer sebelum
-- baris ini pernah ditulis, jadi tidak perlu CHECK constraint duplikat di
-- level database.
CREATE INDEX ix_transactions_business_id ON transactions (business_id);

-- customer_id opsional (banyak transaksi walk-in tanpa customer
-- teridentifikasi) — index tetap dibuat karena kemungkinan besar akan
-- dipakai untuk query "semua transaksi milik customer X" di capability
-- nanti (mis. riwayat pembelian).
CREATE INDEX ix_transactions_customer_id ON transactions (customer_id);
