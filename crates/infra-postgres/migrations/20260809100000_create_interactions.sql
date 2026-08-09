CREATE TABLE interactions (
    id UUID PRIMARY KEY,
    business_id UUID NOT NULL REFERENCES businesses (id),
    customer_id UUID NOT NULL REFERENCES customers (id),
    interaction_type TEXT NOT NULL,
    note TEXT,
    occurred_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Beda dari transactions.customer_id (nullable) — customer_id di sini
-- NOT NULL karena Interaction secara alami selalu tentang seseorang
-- (keputusan Din, lihat domain::interaction). Tidak ada CHECK panjang
-- `note` di sini — MAX_NOTE_LENGTH sudah dijamin InteractionNote di
-- domain layer sebelum baris ini pernah ditulis, konsisten dengan alasan
-- TransactionAmount > 0 tidak diduplikasi jadi CHECK constraint.
CREATE INDEX ix_interactions_business_id ON interactions (business_id);
CREATE INDEX ix_interactions_customer_id ON interactions (customer_id);
