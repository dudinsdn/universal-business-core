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

use application::{BusinessRepository, RepositoryError, TenantRepository};
use domain::{Business, BusinessName, BusinessType, Tenant, TenantName};
use infra_postgres::{PgBusinessRepository, PgTenantRepository};
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
