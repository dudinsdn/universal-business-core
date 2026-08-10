CREATE TABLE service_orders (
    id UUID PRIMARY KEY,
    business_id UUID NOT NULL REFERENCES businesses (id),
    customer_id UUID NOT NULL REFERENCES customers (id),
    description TEXT NOT NULL,
    status TEXT NOT NULL,
    transaction_id UUID REFERENCES transactions (id),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 0
);

-- Tidak ada CHECK constraint untuk nilai status yang valid (mis. status
-- IN ('received','in_progress','completed','cancelled')) -- validitas
-- nilai dijamin oleh ServiceOrderStatus (enum tertutup) di domain layer
-- Capability Workshop sebelum baris ini pernah ditulis, konsisten dengan
-- pola TransactionAmount > 0 yang juga tidak diduplikasi jadi CHECK
-- constraint di Core.
CREATE INDEX ix_service_orders_business_id ON service_orders (business_id);
CREATE INDEX ix_service_orders_customer_id ON service_orders (customer_id);
CREATE INDEX ix_service_orders_transaction_id ON service_orders (transaction_id);
