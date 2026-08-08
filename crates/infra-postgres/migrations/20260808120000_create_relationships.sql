CREATE TABLE relationships (
    id UUID PRIMARY KEY,
    business_id UUID NOT NULL REFERENCES businesses (id),
    from_customer_id UUID NOT NULL REFERENCES customers (id),
    to_customer_id UUID NOT NULL REFERENCES customers (id),
    relationship_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Tidak ada CHECK constraint (from_customer_id <> to_customer_id) di sini
-- — beda dari ux_businesses_tenant_name_active yang memang defense-in-depth
-- terhadap race condition. SelfRelationship bukan skenario race (dua id
-- datang dari satu request yang sama), jadi validasi di
-- Relationship::with_id (domain layer) sudah cukup — konsisten dengan
-- TransactionAmount > 0 yang juga tidak diduplikasi jadi CHECK constraint.
--
-- Tidak ada unique constraint untuk pasangan Customer + jenis — pencegahan
-- relationship duplikat belum diimplementasikan (lihat catatan di
-- RelationshipService).
CREATE INDEX ix_relationships_business_id ON relationships (business_id);
CREATE INDEX ix_relationships_from_customer_id ON relationships (from_customer_id);
CREATE INDEX ix_relationships_to_customer_id ON relationships (to_customer_id);
