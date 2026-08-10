//! Repository Test — sesuai aturan Testing di Development_Rules.pdf:
//! "Repository Test jika menggunakan database".
//!
//! Dijalankan lawan Postgres ASLI (native, dari repo Debian stable),
//! bukan docker. `#[sqlx::test]` otomatis:
//! - membaca DATABASE_URL dari environment,
//! - membuat satu database sementara yang terisolasi PER TEST (jadi test
//!   tidak saling bentrok data walau dijalankan paralel),
//! - menjalankan seluruh migration di folder `migrations/`,
//! - menghapus database sementara itu setelah test selesai.
//!
//! Butuh role Postgres dengan hak CREATEDB. Lihat instruksi setup di
//! pesan pendamping.

use application::{
    BusinessRepository, CustomerRepository, InteractionRepository, RelationshipRepository,
    RepositoryError, TenantRepository, TransactionRepository,
};
use capability_workshop::{ServiceOrder, ServiceOrderDescription, ServiceOrderRepository};
use chrono::Utc;
use domain::{
    Business, BusinessName, BusinessType, Customer, CustomerName, CustomerPhone, Interaction,
    InteractionNote, InteractionType, Relationship, RelationshipType, Tenant, TenantName,
    Transaction, TransactionAmount, TransactionKind,
};
use infra_postgres::{
    PgBusinessRepository, PgCustomerRepository, PgInteractionRepository, PgRelationshipRepository,
    PgServiceOrderRepository, PgTenantRepository, PgTransactionRepository,
};
use sqlx::PgPool;

#[sqlx::test]
async fn save_and_find_tenant_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgTenantRepository::new(pool);
    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());

    repo.save(&tenant).await.unwrap();
    let fetched = repo.find_by_id(tenant.id()).await.unwrap().unwrap();

    assert_eq!(fetched.id(), tenant.id());
    assert_eq!(fetched.name().as_str(), "Tenant A");
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn find_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgTenantRepository::new(pool);
    let result = repo.find_by_id(domain::TenantId::new()).await.unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_detects_version_conflict_at_database_level(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgTenantRepository::new(pool);
    let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    repo.save(&tenant).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = tenant.clone();

    // Salah satu menang duluan.
    tenant.rename(TenantName::new("Tenant A Baru").unwrap());
    repo.save(&tenant).await.unwrap();

    // Yang telat masih berdasarkan versi 0 — harus ditolak DB, bukan
    // cuma tertimpa diam-diam.
    stale_copy.rename(TenantName::new("Tenant A Telat").unwrap());
    let result = repo.save(&stale_copy).await;

    assert!(matches!(result, Err(RepositoryError::VersionConflict)));
    Ok(())
}

#[sqlx::test]
async fn duplicate_active_business_name_per_tenant_is_rejected(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();

    // Nama sama, tenant sama, keduanya aktif — harus ditolak index unik
    // (defense-in-depth terhadap race di Application Service).
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    let result = business_repo.save(&business_b).await;

    assert!(matches!(
        result,
        Err(RepositoryError::UniqueConstraintViolation)
    ));
    Ok(())
}

#[sqlx::test]
async fn same_business_name_allowed_across_different_tenants(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool);

    let tenant_a = Tenant::new(TenantName::new("Tenant A").unwrap());
    let tenant_b = Tenant::new(TenantName::new("Tenant B").unwrap());
    tenant_repo.save(&tenant_a).await.unwrap();
    tenant_repo.save(&tenant_b).await.unwrap();

    let business_a = Business::new(
        tenant_a.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    let business_b = Business::new(
        tenant_b.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );

    business_repo.save(&business_a).await.unwrap();
    let result = business_repo.save(&business_b).await;

    assert!(result.is_ok());
    Ok(())
}

#[sqlx::test]
async fn count_active_by_tenant_excludes_soft_deleted(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let mut business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    assert_eq!(
        business_repo
            .count_active_by_tenant(tenant.id())
            .await
            .unwrap(),
        1
    );

    business.soft_delete();
    business_repo.save(&business).await.unwrap();

    assert_eq!(
        business_repo
            .count_active_by_tenant(tenant.id())
            .await
            .unwrap(),
        0
    );
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_only_returns_tenants_changed_after_cursor(
    pool: PgPool,
) -> sqlx::Result<()> {
    let repo = PgTenantRepository::new(pool);

    let old_tenant = Tenant::new(TenantName::new("Tenant Lama").unwrap());
    repo.save(&old_tenant).await.unwrap();

    let cursor = Utc::now();

    let new_tenant = Tenant::new(TenantName::new("Tenant Baru").unwrap());
    repo.save(&new_tenant).await.unwrap();

    let changed = repo.find_updated_since(cursor).await.unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), new_tenant.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_includes_soft_deleted_tenants(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgTenantRepository::new(pool);

    let mut tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    repo.save(&tenant).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga, bukan cuma
    // perubahan pada entity yang masih aktif.
    tenant.soft_delete();
    repo.save(&tenant).await.unwrap();

    let changed = repo.find_updated_since(cursor).await.unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}

#[sqlx::test]
async fn save_and_find_interaction_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let interaction_repo = PgInteractionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let interaction = Interaction::new(
        business.id(),
        customer.id(),
        InteractionType::new("call").unwrap(),
        Some(InteractionNote::new("Follow up jadwal kontrol").unwrap()),
        Utc::now(),
    );
    interaction_repo.save(&interaction).await.unwrap();

    let fetched = interaction_repo
        .find_by_id(interaction.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.id(), interaction.id());
    assert_eq!(fetched.business_id(), business.id());
    assert_eq!(fetched.customer_id(), customer.id());
    assert_eq!(fetched.interaction_type().as_str(), "call");
    assert_eq!(
        fetched.note().map(|n| n.as_str()),
        Some("Follow up jadwal kontrol")
    );
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn save_and_find_interaction_without_note_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let interaction_repo = PgInteractionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let interaction = Interaction::new(
        business.id(),
        customer.id(),
        InteractionType::new("visit").unwrap(),
        None,
        Utc::now(),
    );
    interaction_repo.save(&interaction).await.unwrap();

    let fetched = interaction_repo
        .find_by_id(interaction.id())
        .await
        .unwrap()
        .unwrap();

    assert!(fetched.note().is_none());
    Ok(())
}

#[sqlx::test]
async fn find_interaction_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgInteractionRepository::new(pool);
    let result = repo.find_by_id(domain::InteractionId::new()).await.unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_interaction_detects_version_conflict_at_database_level(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let interaction_repo = PgInteractionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let mut interaction = Interaction::new(
        business.id(),
        customer.id(),
        InteractionType::new("call").unwrap(),
        None,
        Utc::now(),
    );
    interaction_repo.save(&interaction).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = interaction.clone();

    // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
    // disimpan.
    interaction.soft_delete();
    interaction_repo.save(&interaction).await.unwrap();

    // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
    // soft delete juga, tapi versi 0 di penyimpanan sudah tidak ada lagi —
    // harus ditolak DB.
    stale_copy.soft_delete();
    let result = interaction_repo.save(&stale_copy).await;

    assert!(matches!(result, Err(RepositoryError::VersionConflict)));
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_excludes_other_business_for_interaction(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let interaction_repo = PgInteractionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Klinik B").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let customer_a = Customer::new(business_a.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business_b.id(), CustomerName::new("Siti").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let interaction_a = Interaction::new(
        business_a.id(),
        customer_a.id(),
        InteractionType::new("call").unwrap(),
        None,
        Utc::now(),
    );
    let interaction_b = Interaction::new(
        business_b.id(),
        customer_b.id(),
        InteractionType::new("call").unwrap(),
        None,
        Utc::now(),
    );
    interaction_repo.save(&interaction_a).await.unwrap();
    interaction_repo.save(&interaction_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = interaction_repo
        .find_updated_since_by_business(business_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), interaction_a.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_includes_soft_deleted_interactions(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let interaction_repo = PgInteractionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let mut interaction = Interaction::new(
        business.id(),
        customer.id(),
        InteractionType::new("call").unwrap(),
        None,
        Utc::now(),
    );
    interaction_repo.save(&interaction).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga.
    interaction.soft_delete();
    interaction_repo.save(&interaction).await.unwrap();

    let changed = interaction_repo
        .find_updated_since_by_business(business.id(), cursor)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}

#[sqlx::test]
async fn save_and_find_relationship_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let relationship_repo = PgRelationshipRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer_a = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business.id(), CustomerName::new("Ani").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let relationship = Relationship::new(
        business.id(),
        customer_a.id(),
        customer_b.id(),
        RelationshipType::new("sibling").unwrap(),
    )
    .unwrap();
    relationship_repo.save(&relationship).await.unwrap();

    let fetched = relationship_repo
        .find_by_id(relationship.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.id(), relationship.id());
    assert_eq!(fetched.business_id(), business.id());
    assert_eq!(fetched.from_customer_id(), customer_a.id());
    assert_eq!(fetched.to_customer_id(), customer_b.id());
    assert_eq!(fetched.relationship_type().as_str(), "sibling");
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn find_relationship_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgRelationshipRepository::new(pool);
    let result = repo
        .find_by_id(domain::RelationshipId::new())
        .await
        .unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_relationship_detects_version_conflict_at_database_level(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let relationship_repo = PgRelationshipRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer_a = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business.id(), CustomerName::new("Ani").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let mut relationship = Relationship::new(
        business.id(),
        customer_a.id(),
        customer_b.id(),
        RelationshipType::new("sibling").unwrap(),
    )
    .unwrap();
    relationship_repo.save(&relationship).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = relationship.clone();

    // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
    // disimpan.
    relationship.soft_delete();
    relationship_repo.save(&relationship).await.unwrap();

    // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
    // soft delete juga, tapi versi 0 di penyimpanan sudah tidak ada lagi —
    // harus ditolak DB.
    stale_copy.soft_delete();
    let result = relationship_repo.save(&stale_copy).await;

    assert!(matches!(result, Err(RepositoryError::VersionConflict)));
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_excludes_other_business_for_relationship(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let relationship_repo = PgRelationshipRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Klinik B").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let customer_a1 = Customer::new(business_a.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_a2 = Customer::new(business_a.id(), CustomerName::new("Ani").unwrap(), None);
    let customer_b1 = Customer::new(business_b.id(), CustomerName::new("Siti").unwrap(), None);
    let customer_b2 = Customer::new(business_b.id(), CustomerName::new("Joko").unwrap(), None);
    customer_repo.save(&customer_a1).await.unwrap();
    customer_repo.save(&customer_a2).await.unwrap();
    customer_repo.save(&customer_b1).await.unwrap();
    customer_repo.save(&customer_b2).await.unwrap();

    let relationship_a = Relationship::new(
        business_a.id(),
        customer_a1.id(),
        customer_a2.id(),
        RelationshipType::new("sibling").unwrap(),
    )
    .unwrap();
    let relationship_b = Relationship::new(
        business_b.id(),
        customer_b1.id(),
        customer_b2.id(),
        RelationshipType::new("sibling").unwrap(),
    )
    .unwrap();
    relationship_repo.save(&relationship_a).await.unwrap();
    relationship_repo.save(&relationship_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = relationship_repo
        .find_updated_since_by_business(business_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), relationship_a.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_includes_soft_deleted_relationships(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let relationship_repo = PgRelationshipRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Klinik A").unwrap(),
        BusinessType::new("clinic").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer_a = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business.id(), CustomerName::new("Ani").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let mut relationship = Relationship::new(
        business.id(),
        customer_a.id(),
        customer_b.id(),
        RelationshipType::new("sibling").unwrap(),
    )
    .unwrap();
    relationship_repo.save(&relationship).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga.
    relationship.soft_delete();
    relationship_repo.save(&relationship).await.unwrap();

    let changed = relationship_repo
        .find_updated_since_by_business(business.id(), cursor)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}

#[sqlx::test]
async fn save_and_find_transaction_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let transaction = Transaction::new(
        business.id(),
        None,
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(50_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction).await.unwrap();

    let fetched = transaction_repo
        .find_by_id(transaction.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.id(), transaction.id());
    assert_eq!(fetched.business_id(), business.id());
    assert!(fetched.customer_id().is_none());
    assert_eq!(fetched.kind().as_str(), "sale");
    assert_eq!(fetched.amount().as_i64(), 50_000);
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn save_and_find_transaction_with_customer_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let transaction = Transaction::new(
        business.id(),
        Some(customer.id()),
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(25_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction).await.unwrap();

    let fetched = transaction_repo
        .find_by_id(transaction.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.customer_id(), Some(customer.id()));
    Ok(())
}

#[sqlx::test]
async fn find_transaction_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgTransactionRepository::new(pool);
    let result = repo.find_by_id(domain::TransactionId::new()).await.unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_transaction_detects_version_conflict_at_database_level(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let mut transaction = Transaction::new(
        business.id(),
        None,
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(10_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = transaction.clone();

    // Salah satu menang duluan: soft delete, jadi versi 1, berhasil
    // disimpan.
    transaction.soft_delete();
    transaction_repo.save(&transaction).await.unwrap();

    // Yang telat masih berdasarkan versi 0 saat mulai mutasi — mencoba
    // soft delete juga (jadi versi 1 di sisinya sendiri), tapi versi 0 di
    // penyimpanan sudah tidak ada lagi — harus ditolak DB, bukan cuma
    // tertimpa diam-diam.
    stale_copy.soft_delete();
    let result = transaction_repo.save(&stale_copy).await;

    assert!(matches!(result, Err(RepositoryError::VersionConflict)));
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_excludes_other_business_for_transaction(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Toko A").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Toko B").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let transaction_a = Transaction::new(
        business_a.id(),
        None,
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(10_000).unwrap(),
        Utc::now(),
    );
    let transaction_b = Transaction::new(
        business_b.id(),
        None,
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(10_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction_a).await.unwrap();
    transaction_repo.save(&transaction_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = transaction_repo
        .find_updated_since_by_business(business_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), transaction_a.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_includes_soft_deleted_transactions(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let mut transaction = Transaction::new(
        business.id(),
        None,
        TransactionKind::new("sale").unwrap(),
        TransactionAmount::new(10_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga.
    transaction.soft_delete();
    transaction_repo.save(&transaction).await.unwrap();

    let changed = transaction_repo
        .find_updated_since_by_business(business.id(), cursor)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_tenant_excludes_other_tenants(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool);

    let tenant_a = Tenant::new(TenantName::new("Tenant A").unwrap());
    let tenant_b = Tenant::new(TenantName::new("Tenant B").unwrap());
    tenant_repo.save(&tenant_a).await.unwrap();
    tenant_repo.save(&tenant_b).await.unwrap();

    let business_a = Business::new(
        tenant_a.id(),
        BusinessName::new("Toko A").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    let business_b = Business::new(
        tenant_b.id(),
        BusinessName::new("Toko B").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = business_repo
        .find_updated_since_by_tenant(tenant_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), business_a.id());
    Ok(())
}

#[sqlx::test]
async fn save_and_find_customer_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let customer = Customer::new(
        business.id(),
        CustomerName::new("Budi").unwrap(),
        Some(CustomerPhone::new("081234567890").unwrap()),
    );
    customer_repo.save(&customer).await.unwrap();

    let fetched = customer_repo
        .find_by_id(customer.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.id(), customer.id());
    assert_eq!(fetched.name().as_str(), "Budi");
    assert_eq!(fetched.phone().unwrap().as_str(), "081234567890");
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn find_customer_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgCustomerRepository::new(pool);
    let result = repo.find_by_id(domain::CustomerId::new()).await.unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_customer_detects_version_conflict_at_database_level(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let mut customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = customer.clone();

    customer.rename(CustomerName::new("Budi Santoso").unwrap());
    customer_repo.save(&customer).await.unwrap();

    // Yang telat masih berdasarkan versi 0 — harus ditolak DB.
    stale_copy.rename(CustomerName::new("Budi Telat").unwrap());
    let result = customer_repo.save(&stale_copy).await;

    assert!(matches!(result, Err(RepositoryError::VersionConflict)));
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_excludes_other_business(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Toko A").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Toko B").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let customer_a = Customer::new(business_a.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business_b.id(), CustomerName::new("Siti").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = customer_repo
        .find_updated_since_by_business(business_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), customer_a.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_includes_soft_deleted_customers(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Toko Baju").unwrap(),
        BusinessType::new("retail").unwrap(),
    );
    business_repo.save(&business).await.unwrap();

    let mut customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga, bukan cuma
    // perubahan pada entity yang masih aktif.
    customer.soft_delete();
    customer_repo.save(&customer).await.unwrap();

    let changed = customer_repo
        .find_updated_since_by_business(business.id(), cursor)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}

#[sqlx::test]
async fn save_and_find_service_order_roundtrips(pool: PgPool) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let service_order_repo = PgServiceOrderRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel Jaya").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let order = ServiceOrder::new(
        business.id(),
        customer.id(),
        ServiceOrderDescription::new("Ganti oli dan servis rem").unwrap(),
    );
    service_order_repo.save(&order).await.unwrap();

    let fetched = service_order_repo
        .find_by_id(order.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched.id(), order.id());
    assert_eq!(fetched.business_id(), business.id());
    assert_eq!(fetched.customer_id(), customer.id());
    assert_eq!(fetched.description().as_str(), "Ganti oli dan servis rem");
    assert_eq!(
        fetched.status(),
        capability_workshop::ServiceOrderStatus::Received
    );
    assert!(fetched.transaction_id().is_none());
    assert_eq!(fetched.version(), 0);
    Ok(())
}

#[sqlx::test]
async fn find_service_order_by_id_returns_none_when_not_found(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgServiceOrderRepository::new(pool);
    let result = repo
        .find_by_id(capability_workshop::ServiceOrderId::new())
        .await
        .unwrap();
    assert!(result.is_none());
    Ok(())
}

#[sqlx::test]
async fn save_service_order_persists_status_and_transaction_link_after_transitions(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let transaction_repo = PgTransactionRepository::new(pool.clone());
    let service_order_repo = PgServiceOrderRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel Jaya").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();
    let transaction = Transaction::new(
        business.id(),
        Some(customer.id()),
        TransactionKind::new("service").unwrap(),
        TransactionAmount::new(150_000).unwrap(),
        Utc::now(),
    );
    transaction_repo.save(&transaction).await.unwrap();

    let mut order = ServiceOrder::new(
        business.id(),
        customer.id(),
        ServiceOrderDescription::new("Ganti kampas rem").unwrap(),
    );
    service_order_repo.save(&order).await.unwrap();

    order.start().unwrap();
    service_order_repo.save(&order).await.unwrap();

    order.complete(Some(transaction.id())).unwrap();
    service_order_repo.save(&order).await.unwrap();

    let fetched = service_order_repo
        .find_by_id(order.id())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        fetched.status(),
        capability_workshop::ServiceOrderStatus::Completed
    );
    assert_eq!(fetched.transaction_id(), Some(transaction.id()));
    assert_eq!(fetched.version(), 2);
    Ok(())
}

#[sqlx::test]
async fn save_service_order_detects_version_conflict_at_database_level(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let service_order_repo = PgServiceOrderRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel Jaya").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let mut order = ServiceOrder::new(
        business.id(),
        customer.id(),
        ServiceOrderDescription::new("Ganti oli").unwrap(),
    );
    service_order_repo.save(&order).await.unwrap();

    // Dua "pembaca" sama-sama mulai dari data versi 0.
    let mut stale_copy = order.clone();

    // Salah satu menang duluan: start(), jadi versi 1, berhasil disimpan.
    order.start().unwrap();
    service_order_repo.save(&order).await.unwrap();

    // Yang telat masih berdasarkan versi 0 — mencoba cancel() juga, tapi
    // versi 0 di penyimpanan sudah tidak ada lagi — harus ditolak DB.
    stale_copy.cancel().unwrap();
    let result = service_order_repo.save(&stale_copy).await;

    assert!(matches!(
        result,
        Err(capability_workshop::RepositoryError::VersionConflict)
    ));
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_excludes_other_business_for_service_order(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let service_order_repo = PgServiceOrderRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();

    let business_a = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel A").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    let business_b = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel B").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    business_repo.save(&business_a).await.unwrap();
    business_repo.save(&business_b).await.unwrap();

    let customer_a = Customer::new(business_a.id(), CustomerName::new("Budi").unwrap(), None);
    let customer_b = Customer::new(business_b.id(), CustomerName::new("Siti").unwrap(), None);
    customer_repo.save(&customer_a).await.unwrap();
    customer_repo.save(&customer_b).await.unwrap();

    let order_a = ServiceOrder::new(
        business_a.id(),
        customer_a.id(),
        ServiceOrderDescription::new("Ganti oli").unwrap(),
    );
    let order_b = ServiceOrder::new(
        business_b.id(),
        customer_b.id(),
        ServiceOrderDescription::new("Ganti oli").unwrap(),
    );
    service_order_repo.save(&order_a).await.unwrap();
    service_order_repo.save(&order_b).await.unwrap();

    let epoch = chrono::DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    let changed = service_order_repo
        .find_updated_since_by_business(business_a.id(), epoch)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id(), order_a.id());
    Ok(())
}

#[sqlx::test]
async fn find_updated_since_by_business_includes_soft_deleted_service_orders(
    pool: PgPool,
) -> sqlx::Result<()> {
    let tenant_repo = PgTenantRepository::new(pool.clone());
    let business_repo = PgBusinessRepository::new(pool.clone());
    let customer_repo = PgCustomerRepository::new(pool.clone());
    let service_order_repo = PgServiceOrderRepository::new(pool);

    let tenant = Tenant::new(TenantName::new("Tenant A").unwrap());
    tenant_repo.save(&tenant).await.unwrap();
    let business = Business::new(
        tenant.id(),
        BusinessName::new("Bengkel Jaya").unwrap(),
        BusinessType::new("workshop").unwrap(),
    );
    business_repo.save(&business).await.unwrap();
    let customer = Customer::new(business.id(), CustomerName::new("Budi").unwrap(), None);
    customer_repo.save(&customer).await.unwrap();

    let mut order = ServiceOrder::new(
        business.id(),
        customer.id(),
        ServiceOrderDescription::new("Ganti oli").unwrap(),
    );
    service_order_repo.save(&order).await.unwrap();

    let cursor = Utc::now();

    // Client offline harus tahu soal penghapusan juga.
    order.soft_delete();
    service_order_repo.save(&order).await.unwrap();

    let changed = service_order_repo
        .find_updated_since_by_business(business.id(), cursor)
        .await
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert!(changed[0].is_deleted());
    Ok(())
}
