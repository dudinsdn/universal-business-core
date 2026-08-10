use capability_workshop::{
    PersistedServiceOrder, RepositoryError, ServiceOrder, ServiceOrderDescription, ServiceOrderId,
    ServiceOrderRepository, ServiceOrderStatus, WorkshopError,
};
use chrono::{DateTime, Utc};
use domain::{BusinessId, CustomerId, TransactionId};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgServiceOrderRepository {
    pool: PgPool,
}

impl PgServiceOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Merekonstruksi `ServiceOrderStatus` dari kolom TEXT. Lihat komentar di
/// `ServiceOrderStatus::from_str` (capability-workshop) — jaga-jaga
/// terhadap data yang diubah manual di luar aplikasi, pola sama seperti
/// `TenantName::new(row.name)` di `PgTenantRepository`.
fn parse_status(raw: String) -> Result<ServiceOrderStatus, RepositoryError> {
    raw.parse::<ServiceOrderStatus>()
        .map_err(|e: WorkshopError| RepositoryError::Unavailable(e.to_string()))
}

impl ServiceOrderRepository for PgServiceOrderRepository {
    async fn find_by_id(
        &self,
        id: ServiceOrderId,
    ) -> Result<Option<ServiceOrder>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, description, status, transaction_id,
                   created_at, updated_at, deleted_at, version
            FROM service_orders
            WHERE id = $1
            "#,
            id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let description = ServiceOrderDescription::new(row.description)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        let status = parse_status(row.status)?;

        Ok(Some(ServiceOrder::from_persisted(PersistedServiceOrder {
            id: ServiceOrderId::from_uuid(row.id),
            business_id: BusinessId::from_uuid(row.business_id),
            customer_id: CustomerId::from_uuid(row.customer_id),
            description,
            status,
            transaction_id: row.transaction_id.map(TransactionId::from_uuid),
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn save(&self, order: &ServiceOrder) -> Result<(), RepositoryError> {
        if order.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO service_orders
                    (id, business_id, customer_id, description, status, transaction_id,
                     created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                order.id().as_uuid(),
                order.business_id().as_uuid(),
                order.customer_id().as_uuid(),
                order.description().as_str(),
                order.status().as_str(),
                order.transaction_id().map(|t| t.as_uuid()),
                order.created_at(),
                order.updated_at(),
                order.deleted_at(),
                order.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database — pola sama seperti
        // repository lain di Core. `status` dan `transaction_id` IKUT
        // di-SET (beda dari Transaction/Relationship yang immutable
        // penuh) karena keduanya berubah lewat start()/complete()/
        // cancel(). `description` TIDAK ikut di-SET — tidak ada method
        // untuk mengubahnya setelah dibuat (lihat domain::ServiceOrder).
        let expected_previous_version = (order.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE service_orders
            SET status = $1, transaction_id = $2, updated_at = $3, deleted_at = $4, version = $5
            WHERE id = $6 AND version = $7
            "#,
            order.status().as_str(),
            order.transaction_id().map(|t| t.as_uuid()),
            order.updated_at(),
            order.deleted_at(),
            order.version() as i32,
            order.id().as_uuid(),
            expected_previous_version,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::VersionConflict);
        }

        Ok(())
    }

    async fn find_updated_since_by_business(
        &self,
        business_id: BusinessId,
        since: DateTime<Utc>,
    ) -> Result<Vec<ServiceOrder>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, description, status, transaction_id,
                   created_at, updated_at, deleted_at, version
            FROM service_orders
            WHERE business_id = $1 AND updated_at > $2
            ORDER BY updated_at ASC
            "#,
            business_id.as_uuid(),
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        rows.into_iter()
            .map(|row| {
                let description = ServiceOrderDescription::new(row.description)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                let status = parse_status(row.status)?;

                Ok(ServiceOrder::from_persisted(PersistedServiceOrder {
                    id: ServiceOrderId::from_uuid(row.id),
                    business_id: BusinessId::from_uuid(row.business_id),
                    customer_id: CustomerId::from_uuid(row.customer_id),
                    description,
                    status,
                    transaction_id: row.transaction_id.map(TransactionId::from_uuid),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                    version: row.version as u32,
                }))
            })
            .collect()
    }
}
