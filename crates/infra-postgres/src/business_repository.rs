use application::{BusinessRepository, RepositoryError};
use chrono::{DateTime, Utc};
use domain::{Business, BusinessId, BusinessName, TenantId, business::PersistedBusiness};
use sqlx::PgPool;

/// Memetakan error sqlx generik ke `RepositoryError`. Khusus mendeteksi
/// pelanggaran `ux_businesses_tenant_name_active` (nama duplikat aktif per
/// tenant, lolos dari pengecekan di Application Service karena race
/// condition) supaya HTTP layer bisa membalasnya sebagai 409 Conflict,
/// bukan 500 generik.
fn map_sqlx_error(err: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.constraint() == Some("ux_businesses_tenant_name_active") {
            return RepositoryError::UniqueConstraintViolation;
        }
    }
    RepositoryError::Unavailable(err.to_string())
}

#[derive(Debug, Clone)]
pub struct PgBusinessRepository {
    pool: PgPool,
}

impl PgBusinessRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl BusinessRepository for PgBusinessRepository {
    async fn find_by_id(&self, id: BusinessId) -> Result<Option<Business>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, tenant_id, name, business_type, created_at, updated_at, deleted_at, version
            FROM businesses
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
            BusinessName::new(row.name).map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        let business_type = domain::BusinessType::new(row.business_type)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(Some(Business::from_persisted(PersistedBusiness {
            id: BusinessId::from_uuid(row.id),
            tenant_id: TenantId::from_uuid(row.tenant_id),
            name,
            business_type,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn find_active_names_by_tenant(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<BusinessName>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT name
            FROM businesses
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
            tenant_id.as_uuid()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        rows.into_iter()
            .map(|row| {
                BusinessName::new(row.name).map_err(|e| RepositoryError::Unavailable(e.to_string()))
            })
            .collect()
    }

    async fn count_active_by_tenant(&self, tenant_id: TenantId) -> Result<usize, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*) AS count
            FROM businesses
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
            tenant_id.as_uuid()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(row.count.unwrap_or(0) as usize)
    }

    async fn save(&self, business: &Business) -> Result<(), RepositoryError> {
        if business.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO businesses
                    (id, tenant_id, name, business_type, created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                business.id().as_uuid(),
                business.tenant_id().as_uuid(),
                business.name().as_str(),
                business.business_type().as_str(),
                business.created_at(),
                business.updated_at(),
                business.deleted_at(),
                business.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

            return Ok(());
        }

        // Sama seperti PgTenantRepository: optimistic locking di level
        // database lewat conditional UPDATE. tenant_id sengaja tidak ikut
        // di-SET — itu identitas relasi yang tidak pernah berubah setelah
        // dibuat.
        let expected_previous_version = (business.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE businesses
            SET name = $1, business_type = $2, updated_at = $3, deleted_at = $4, version = $5
            WHERE id = $6 AND version = $7
            "#,
            business.name().as_str(),
            business.business_type().as_str(),
            business.updated_at(),
            business.deleted_at(),
            business.version() as i32,
            business.id().as_uuid(),
            expected_previous_version,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::VersionConflict);
        }

        Ok(())
    }

    async fn find_updated_since_by_tenant(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> Result<Vec<Business>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, tenant_id, name, business_type, created_at, updated_at, deleted_at, version
            FROM businesses
            WHERE tenant_id = $1 AND updated_at > $2
            ORDER BY updated_at ASC
            "#,
            tenant_id.as_uuid(),
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        rows.into_iter()
            .map(|row| {
                let name = BusinessName::new(row.name)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                let business_type = domain::BusinessType::new(row.business_type)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

                Ok(Business::from_persisted(PersistedBusiness {
                    id: BusinessId::from_uuid(row.id),
                    tenant_id: TenantId::from_uuid(row.tenant_id),
                    name,
                    business_type,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                    version: row.version as u32,
                }))
            })
            .collect()
    }
}
