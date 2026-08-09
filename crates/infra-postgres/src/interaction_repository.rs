use application::{InteractionRepository, RepositoryError};
use chrono::{DateTime, Utc};
use domain::{
    BusinessId, CustomerId, Interaction, InteractionId, InteractionNote, InteractionType,
    interaction::PersistedInteraction,
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgInteractionRepository {
    pool: PgPool,
}

impl PgInteractionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Membangun `Option<InteractionNote>` dari kolom `note` yang nullable.
/// Kolom `NULL` -> `None` (tidak ada catatan). Kolom berisi string ->
/// divalidasi ulang lewat `InteractionNote::new` — jaga-jaga terhadap
/// data yang diubah manual di luar aplikasi, konsisten dengan pola
/// `TenantName::new`/`BusinessType::new` di repository lain.
fn parse_note(raw: Option<String>) -> Result<Option<InteractionNote>, RepositoryError> {
    raw.map(InteractionNote::new)
        .transpose()
        .map_err(|e| RepositoryError::Unavailable(e.to_string()))
}

impl InteractionRepository for PgInteractionRepository {
    async fn find_by_id(&self, id: InteractionId) -> Result<Option<Interaction>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, interaction_type, note, occurred_at,
                   created_at, updated_at, deleted_at, version
            FROM interactions
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

        let interaction_type = InteractionType::new(row.interaction_type)
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
        let note = parse_note(row.note)?;

        Ok(Some(Interaction::from_persisted(PersistedInteraction {
            id: InteractionId::from_uuid(row.id),
            business_id: BusinessId::from_uuid(row.business_id),
            customer_id: CustomerId::from_uuid(row.customer_id),
            interaction_type,
            note,
            occurred_at: row.occurred_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
            version: row.version as u32,
        })))
    }

    async fn save(&self, interaction: &Interaction) -> Result<(), RepositoryError> {
        if interaction.version() == 0 {
            sqlx::query!(
                r#"
                INSERT INTO interactions
                    (id, business_id, customer_id, interaction_type, note, occurred_at,
                     created_at, updated_at, deleted_at, version)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                interaction.id().as_uuid(),
                interaction.business_id().as_uuid(),
                interaction.customer_id().as_uuid(),
                interaction.interaction_type().as_str(),
                interaction.note().map(|n| n.as_str()),
                interaction.occurred_at(),
                interaction.created_at(),
                interaction.updated_at(),
                interaction.deleted_at(),
                interaction.version() as i32,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;

            return Ok(());
        }

        // Optimistic locking di level database — pola sama persis dengan
        // PgTransactionRepository/PgRelationshipRepository::save. Kolom
        // yang immutable secara domain (business_id, customer_id,
        // interaction_type, note, occurred_at) SENGAJA tidak ikut di-SET
        // — Interaction tidak punya method untuk mengubahnya, jadi
        // satu-satunya alasan baris ini pernah di-UPDATE adalah
        // `soft_delete`.
        let expected_previous_version = (interaction.version() - 1) as i32;
        let result = sqlx::query!(
            r#"
            UPDATE interactions
            SET updated_at = $1, deleted_at = $2, version = $3
            WHERE id = $4 AND version = $5
            "#,
            interaction.updated_at(),
            interaction.deleted_at(),
            interaction.version() as i32,
            interaction.id().as_uuid(),
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
    ) -> Result<Vec<Interaction>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, business_id, customer_id, interaction_type, note, occurred_at,
                   created_at, updated_at, deleted_at, version
            FROM interactions
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
                let interaction_type = InteractionType::new(row.interaction_type)
                    .map_err(|e| RepositoryError::Unavailable(e.to_string()))?;
                let note = parse_note(row.note)?;

                Ok(Interaction::from_persisted(PersistedInteraction {
                    id: InteractionId::from_uuid(row.id),
                    business_id: BusinessId::from_uuid(row.business_id),
                    customer_id: CustomerId::from_uuid(row.customer_id),
                    interaction_type,
                    note,
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
