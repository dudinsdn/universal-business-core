use application::{RepositoryError, TransactionRepository};
use chrono::{DateTime, Utc};
use domain::{
    BusinessId, CustomerId, Transaction, TransactionAmount, TransactionId, TransactionKind,
    transaction::PersistedTransaction,
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgTransactionRepository {
    pool: PgPool,
}

impl PgTransactionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl TransactionRepository for PgTransactionRepository {
    async fn find_by_id(&self, id: TransactionId) -> Result<Option<Transaction>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, kind, amount, occurred_at,
                   created_at, updated_at, deleted_at, version
            FROM transactions
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

        let kind = TransactionKind::new(row.kind)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        let amount = TransactionAmount::new(row.amount)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(Some(Transaction::from_persisted(PersistedTransaction {
            id: TransactionId::from_uuid(row.id),
            business_id: BusinessId::from_uuid(row.business_id),
            customer_id: row.customer_id.map(CustomerId::from_uuid),
            kind,
            amount,
            occurred_at: row.occurred_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn save(&self, transaction: &Transaction) -> Result<(), RepositoryError> {
        if transaction.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO transactions
                    (id, business_id, customer_id, kind, amount, occurred_at,
                     created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                transaction.id().as_uuid(),
                transaction.business_id().as_uuid(),
                transaction.customer_id().map(|c| c.as_uuid()),
                transaction.kind().as_str(),
                transaction.amount().as_i64(),
                transaction.occurred_at(),
                transaction.created_at(),
                transaction.updated_at(),
                transaction.deleted_at(),
                transaction.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database — pola sama persis dengan
        // PgCustomerRepository::save. Kolom yang immutable secara domain
        // (business_id, customer_id, kind, amount, occurred_at) SENGAJA
        // tidak ikut di-SET — Transaction tidak punya method untuk
        // mengubahnya (lihat komentar di `domain::transaction`), jadi
        // satu-satunya alasan baris ini pernah di-UPDATE adalah
        // `soft_delete` (mengubah deleted_at, updated_at, version saja).
        let expected_previous_version = (transaction.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE transactions
            SET updated_at = $1, deleted_at = $2, version = $3
            WHERE id = $4 AND version = $5
            "#,
            transaction.updated_at(),
            transaction.deleted_at(),
            transaction.version() as i32,
            transaction.id().as_uuid(),
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
    ) -> Result<Vec<Transaction>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, kind, amount, occurred_at,
                   created_at, updated_at, deleted_at, version
            FROM transactions
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
                let kind = TransactionKind::new(row.kind)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                let amount = TransactionAmount::new(row.amount)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

                Ok(Transaction::from_persisted(PersistedTransaction {
                    id: TransactionId::from_uuid(row.id),
                    business_id: BusinessId::from_uuid(row.business_id),
                    customer_id: row.customer_id.map(CustomerId::from_uuid),
                    kind,
                    amount,
                    occurred_at: row.occurred_at,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                    version: row.version as u32,
                }))
            })
            .collect()
    }
}
