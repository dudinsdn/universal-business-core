use application::{RepositoryError, TenantRepository};
use chrono::{DateTime, Utc};
use domain::{Tenant, TenantId, TenantName};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgTenantRepository {
    pool: PgPool,
}

impl PgTenantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl TenantRepository for PgTenantRepository {
    async fn find_by_id(&self, id: TenantId) -> Result<Option<Tenant>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, name, created_at, updated_at, deleted_at, version
            FROM tenants
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

        // Nama sudah tervalidasi saat pertama kali disimpan; validasi ulang
        // di sini murni jaga-jaga terhadap data yang diubah manual di luar
        // aplikasi (mis. lewat psql langsung), bukan jalur normal.
        let name =
            TenantName::new(row.name).map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(Some(Tenant::from_persisted(
            TenantId::from_uuid(row.id),
            name,
            row.created_at,
            row.updated_at,
            row.deleted_at,
            row.version as u32,
        )))
    }

    async fn save(&self, tenant: &Tenant) -> Result<(), RepositoryError> {
        if tenant.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO tenants (id, name, created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                tenant.id().as_uuid(),
                tenant.name().as_str(),
                tenant.created_at(),
                tenant.updated_at(),
                tenant.deleted_at(),
                tenant.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database: hanya berhasil kalau versi
        // yang tersimpan masih sama dengan versi SEBELUM perubahan ini.
        // 0 baris ter-update berarti ada pihak lain yang menulis duluan.
        let expected_previous_version = (tenant.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE tenants
            SET name = $1, updated_at = $2, deleted_at = $3, version = $4
            WHERE id = $5 AND version = $6
            "#,
            tenant.name().as_str(),
            tenant.updated_at(),
            tenant.deleted_at(),
            tenant.version() as i32,
            tenant.id().as_uuid(),
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

    async fn find_updated_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<Tenant>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, name, created_at, updated_at, deleted_at, version
            FROM tenants
            WHERE updated_at > $1
            ORDER BY updated_at ASC
            "#,
            since
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        rows.into_iter()
            .map(|row| {
                let name = TenantName::new(row.name)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                Ok(Tenant::from_persisted(
                    TenantId::from_uuid(row.id),
                    name,
                    row.created_at,
                    row.updated_at,
                    row.deleted_at,
                    row.version as u32,
                ))
            })
            .collect()
    }
}
