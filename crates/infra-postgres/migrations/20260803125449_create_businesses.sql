CREATE TABLE businesses (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants (id),
    name TEXT NOT NULL,
    business_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Defense-in-depth: pengecekan keunikan nama di Application Service
-- (rules::ensure_business_name_unique) punya celah TOCTOU pada beban
-- concurrent (dua request baca "belum ada nama sama" sebelum salah satu
-- selesai insert). Index ini mencegah dua baris aktif dengan nama sama
-- pada tenant yang sama lolos sampai level database. Perbandingan
-- case-sensitive, konsisten dengan BusinessName di domain (tidak
-- di-lowercase).
CREATE UNIQUE INDEX ux_businesses_tenant_name_active
    ON businesses (tenant_id, name)
    WHERE deleted_at IS NULL;

CREATE INDEX ix_businesses_tenant_id ON businesses (tenant_id);
