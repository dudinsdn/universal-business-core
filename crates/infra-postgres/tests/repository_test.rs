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

use application::{BusinessRepository, CustomerRepository, RepositoryError, TenantRepository};
use chrono::Utc;
use domain::{
    Business, BusinessName, BusinessType, Customer, CustomerName, CustomerPhone, Tenant, TenantName,
};
use infra_postgres::{PgBusinessRepository, PgCustomerRepository, PgTenantRepository};
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
