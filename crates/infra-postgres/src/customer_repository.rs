use application::{CustomerRepository, RepositoryError};
use chrono::{DateTime, Utc};
use domain::{
    BusinessId, Customer, CustomerId, CustomerName, CustomerPhone, customer::PersistedCustomer,
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgCustomerRepository {
    pool: PgPool,
}

impl PgCustomerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl CustomerRepository for PgCustomerRepository {
    async fn find_by_id(&self, id: CustomerId) -> Result<Option<Customer>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, business_id, name, phone, created_at, updated_at, deleted_at, version
            FROM customers
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

        let name =
            CustomerName::new(row.name).map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        let phone = row
            .phone
            .map(CustomerPhone::new)
            .transpose()
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(Some(Customer::from_persisted(PersistedCustomer {
            id: CustomerId::from_uuid(row.id),
            business_id: BusinessId::from_uuid(row.business_id),
            name,
            phone,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn save(&self, customer: &Customer) -> Result<(), RepositoryError> {
        if customer.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO customers
                    (id, business_id, name, phone, created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                customer.id().as_uuid(),
                customer.business_id().as_uuid(),
                customer.name().as_str(),
                customer.phone().map(|p| p.as_str()),
                customer.created_at(),
                customer.updated_at(),
                customer.deleted_at(),
                customer.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database — pola sama persis dengan
        // PgBusinessRepository::save. Tidak ada pengecekan unique
        // constraint di sini karena nama Customer memang tidak unik.
        let expected_previous_version = (customer.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE customers
            SET name = $1, phone = $2, updated_at = $3, deleted_at = $4, version = $5
            WHERE id = $6 AND version = $7
            "#,
            customer.name().as_str(),
            customer.phone().map(|p| p.as_str()),
            customer.updated_at(),
            customer.deleted_at(),
            customer.version() as i32,
            customer.id().as_uuid(),
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
    ) -> Result<Vec<Customer>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, business_id, name, phone, created_at, updated_at, deleted_at, version
            FROM customers
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
                let name = CustomerName::new(row.name)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                let phone = row
                    .phone
                    .map(CustomerPhone::new)
                    .transpose()
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

                Ok(Customer::from_persisted(PersistedCustomer {
                    id: CustomerId::from_uuid(row.id),
                    business_id: BusinessId::from_uuid(row.business_id),
                    name,
                    phone,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                    version: row.version as u32,
                }))
            })
            .collect()
    }
}
