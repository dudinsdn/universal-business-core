use application::{RelationshipRepository, RepositoryError};
use chrono::{DateTime, Utc};
use domain::{
    BusinessId, CustomerId, Relationship, RelationshipId, RelationshipType,
    relationship::PersistedRelationship,
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgRelationshipRepository {
    pool: PgPool,
}

impl PgRelationshipRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RelationshipRepository for PgRelationshipRepository {
    async fn find_by_id(
        &self,
        id: RelationshipId,
    ) -> Result<Option<Relationship>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, business_id, from_customer_id, to_customer_id, relationship_type,
                   created_at, updated_at, deleted_at, version
            FROM relationships
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

        let relationship_type = RelationshipType::new(row.relationship_type)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

        Ok(Some(Relationship::from_persisted(PersistedRelationship {
            id: RelationshipId::from_uuid(row.id),
            business_id: BusinessId::from_uuid(row.business_id),
            from_customer_id: CustomerId::from_uuid(row.from_customer_id),
            to_customer_id: CustomerId::from_uuid(row.to_customer_id),
            relationship_type,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn save(&self, relationship: &Relationship) -> Result<(), RepositoryError> {
        if relationship.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO relationships
                    (id, business_id, from_customer_id, to_customer_id, relationship_type,
                     created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                relationship.id().as_uuid(),
                relationship.business_id().as_uuid(),
                relationship.from_customer_id().as_uuid(),
                relationship.to_customer_id().as_uuid(),
                relationship.relationship_type().as_str(),
                relationship.created_at(),
                relationship.updated_at(),
                relationship.deleted_at(),
                relationship.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database — pola sama persis dengan
        // PgTransactionRepository::save. Kolom yang immutable secara
        // domain (business_id, from_customer_id, to_customer_id,
        // relationship_type) SENGAJA tidak ikut di-SET — Relationship
        // tidak punya method untuk mengubahnya, jadi satu-satunya alasan
        // baris ini pernah di-UPDATE adalah `soft_delete`.
        let expected_previous_version = (relationship.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE relationships
            SET updated_at = $1, deleted_at = $2, version = $3
            WHERE id = $4 AND version = $5
            "#,
            relationship.updated_at(),
            relationship.deleted_at(),
            relationship.version() as i32,
            relationship.id().as_uuid(),
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
    ) -> Result<Vec<Relationship>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, business_id, from_customer_id, to_customer_id, relationship_type,
                   created_at, updated_at, deleted_at, version
            FROM relationships
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
                let relationship_type = RelationshipType::new(row.relationship_type)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

                Ok(Relationship::from_persisted(PersistedRelationship {
                    id: RelationshipId::from_uuid(row.id),
                    business_id: BusinessId::from_uuid(row.business_id),
                    from_customer_id: CustomerId::from_uuid(row.from_customer_id),
                    to_customer_id: CustomerId::from_uuid(row.to_customer_id),
                    relationship_type,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                    version: row.version as u32,
                }))
            })
            .collect()
    }
}
