//! Business rule yang melibatkan lebih dari satu entity/aggregate.
//!
//! Semua fungsi di sini murni (pure function): menerima data yang sudah
//! diambil oleh Application Service dari Repository, lalu mengembalikan
//! keputusan. Tidak ada akses database di sini, sehingga bisa di-unit-test
//! tanpa infrastruktur apa pun.

use crate::business::{BusinessId, BusinessName};
use crate::error::DomainError;

/// Business rule: nama Business harus unik dalam satu Tenant.
///
/// `existing_names` adalah nama-nama Business aktif yang sudah ada pada
/// Tenant yang sama (bukan lintas tenant — pemanggil bertanggung jawab
/// memfilter berdasarkan tenant_id sebelum memanggil fungsi ini).
pub fn ensure_business_name_unique(
    existing_names: &[BusinessName],
    candidate: &BusinessName,
) -> Result<(), DomainError> {
    if existing_names.iter().any(|existing| existing == candidate) {
        return Err(DomainError::DuplicateBusinessName);
    }
    Ok(())
}

/// Business rule: Tenant tidak boleh dihapus (soft delete) selama masih
/// memiliki Business aktif (belum di-soft-delete) di bawahnya.
pub fn ensure_tenant_can_be_deleted(active_business_count: usize) -> Result<(), DomainError> {
    if active_business_count > 0 {
        return Err(DomainError::TenantHasActiveBusiness);
    }
    Ok(())
}

/// Business rule: Business hanya boleh dibuat di bawah Tenant yang masih
/// aktif (belum di-soft-delete).
pub fn ensure_tenant_is_active(tenant_is_deleted: bool) -> Result<(), DomainError> {
    if tenant_is_deleted {
        return Err(DomainError::TenantIsDeleted);
    }
    Ok(())
}

/// Business rule: Optimistic Locking. Update ditolak jika version yang
/// dikirim client tidak sama dengan version yang tersimpan di database.
pub fn ensure_version_matches(expected: u32, actual: u32) -> Result<(), DomainError> {
    if expected != actual {
        return Err(DomainError::VersionConflict);
    }
    Ok(())
}

/// Business rule: Customer hanya boleh dibuat di bawah Business yang
/// masih aktif (belum di-soft-delete). Analog `ensure_tenant_is_active`,
/// tapi untuk aggregate boundary Business -> Customer.
pub fn ensure_business_is_active(business_is_deleted: bool) -> Result<(), DomainError> {
    if business_is_deleted {
        return Err(DomainError::BusinessIsDeleted);
    }
    Ok(())
}

/// Predikat murni: apakah sebuah Customer benar-benar milik Business yang
/// sedang diproses. Dipakai Application Service (`TransactionService`,
/// `RelationshipService`, `InteractionService`, dan
/// `capability_workshop::ServiceOrderService`) SEBELUM membuat entity
/// yang mereferensikan `customer_id` — mencegah client mengirim
/// `customer_id` milik Business/Tenant lain yang lolos begitu saja.
///
/// SENGAJA mengembalikan `bool`, BUKAN `Result<(), DomainError>` seperti
/// rule lain di modul ini — keputusan mau dipetakan ke error APA
/// (`ApplicationError::CustomerNotFound`, demi tidak membocorkan
/// keberadaan data lintas-tenant lewat pesan error — lihat diskusi
/// desain) adalah keputusan Application layer, bukan Domain. Domain di
/// sini cukup menyatakan fakta, bukan menentukan konsekuensi HTTP-nya.
pub fn customer_belongs_to_business(
    customer_business_id: BusinessId,
    business_id: BusinessId,
) -> bool {
    customer_business_id == business_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenant::TenantId;

    #[test]
    fn rejects_duplicate_business_name_in_same_tenant() {
        let existing = vec![BusinessName::new("Toko Baju").unwrap()];
        let candidate = BusinessName::new("Toko Baju").unwrap();

        assert_eq!(
            ensure_business_name_unique(&existing, &candidate),
            Err(DomainError::DuplicateBusinessName)
        );
    }

    #[test]
    fn allows_business_name_when_not_duplicate_in_tenant() {
        let existing = vec![BusinessName::new("Toko Baju").unwrap()];
        let candidate = BusinessName::new("Toko Sepatu").unwrap();

        assert_eq!(ensure_business_name_unique(&existing, &candidate), Ok(()));
    }

    #[test]
    fn same_name_is_allowed_across_different_tenants() {
        // Pemanggil (Application Service) hanya mengambil existing_names
        // dari Tenant yang sama, sehingga nama yang sama di Tenant lain
        // tidak pernah masuk ke `existing_names`. Test ini mensimulasikan
        // itu dengan daftar kosong dari Tenant B.
        let _tenant_a = TenantId::new();
        let tenant_b_existing_names: Vec<BusinessName> = vec![];
        let candidate = BusinessName::new("Toko Baju").unwrap();

        assert_eq!(
            ensure_business_name_unique(&tenant_b_existing_names, &candidate),
            Ok(())
        );
    }

    #[test]
    fn rejects_tenant_deletion_when_active_business_exists() {
        assert_eq!(
            ensure_tenant_can_be_deleted(1),
            Err(DomainError::TenantHasActiveBusiness)
        );
    }

    #[test]
    fn allows_tenant_deletion_when_no_active_business() {
        assert_eq!(ensure_tenant_can_be_deleted(0), Ok(()));
    }

    #[test]
    fn rejects_business_creation_when_tenant_is_deleted() {
        assert_eq!(
            ensure_tenant_is_active(true),
            Err(DomainError::TenantIsDeleted)
        );
    }

    #[test]
    fn allows_business_creation_when_tenant_is_active() {
        assert_eq!(ensure_tenant_is_active(false), Ok(()));
    }

    #[test]
    fn rejects_update_when_version_mismatch() {
        assert_eq!(
            ensure_version_matches(1, 2),
            Err(DomainError::VersionConflict)
        );
    }

    #[test]
    fn allows_update_when_version_matches() {
        assert_eq!(ensure_version_matches(2, 2), Ok(()));
    }

    #[test]
    fn rejects_customer_creation_when_business_is_deleted() {
        assert_eq!(
            ensure_business_is_active(true),
            Err(DomainError::BusinessIsDeleted)
        );
    }

    #[test]
    fn allows_customer_creation_when_business_is_active() {
        assert_eq!(ensure_business_is_active(false), Ok(()));
    }

    #[test]
    fn customer_belongs_to_business_true_when_same_business() {
        let business_id = BusinessId::new();
        assert!(customer_belongs_to_business(business_id, business_id));
    }

    #[test]
    fn customer_belongs_to_business_false_when_different_business() {
        assert!(!customer_belongs_to_business(
            BusinessId::new(),
            BusinessId::new()
        ));
    }
}
