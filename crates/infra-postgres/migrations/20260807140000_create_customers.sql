CREATE TABLE customers (
    id UUID PRIMARY KEY,
    business_id UUID NOT NULL REFERENCES businesses (id),
    name TEXT NOT NULL,
    phone TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Tidak ada unique index seperti ux_businesses_tenant_name_active — nama
-- Customer memang sengaja tidak unik (keputusan domain, lihat
-- domain::customer), jadi tidak ada constraint keunikan yang perlu
-- dijaga di level database.
CREATE INDEX ix_customers_business_id ON customers (business_id);
