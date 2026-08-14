//! Business rule Workshop yang melibatkan lebih dari satu entity/aggregate,
//! atau input dari luar (mis. `expected_version` dari client).
//!
//! Pola sama persis seperti `domain::rules` di Core: fungsi murni, tidak
//! ada akses database, bisa di-unit-test tanpa infrastruktur apa pun.

use domain::BusinessId;

use crate::error::WorkshopError;

/// Business rule: ServiceOrder hanya boleh dibuat di bawah Business yang
/// masih aktif (belum di-soft-delete).
///
/// SENGAJA duplikat kecil dari `domain::rules::ensure_business_is_active`
/// — dipertimbangkan lagi kalau nanti ternyata banyak Capability butuh
/// pengecekan yang sama persis (baru jadi alasan kuat untuk dipindah ke
/// Core atau dibagikan lewat cara lain). Untuk satu pengecekan boolean
/// sesederhana ini, depend ke tipe error Core hanya untuk satu fungsi
/// tidak sepadan dengan kerumitan konversi error yang ditimbulkannya.
pub fn ensure_business_is_active(business_is_deleted: bool) -> Result<(), WorkshopError> {
    if business_is_deleted {
        return Err(WorkshopError::BusinessIsDeleted);
    }
    Ok(())
}

/// Business rule: Optimistic Locking. Update ditolak jika version yang
/// dikirim client tidak sama dengan version yang tersimpan.
pub fn ensure_version_matches(expected: u32, actual: u32) -> Result<(), WorkshopError> {
    if expected != actual {
        return Err(WorkshopError::VersionConflict);
    }
    Ok(())
}

/// Business rule: ServiceOrder hanya boleh di-`complete()` dengan
/// `transaction_id` yang benar-benar milik Business yang sama.
///
/// Pola sama persis seperti `domain::rules::customer_belongs_to_business`
/// — predikat murni, keputusan mau dipetakan ke error APA
/// (`WorkshopError::TransactionNotFound`, demi info-hiding) ada di
/// pemanggil (`ServiceOrderService`), bukan di sini.
pub fn transaction_belongs_to_business(
    transaction_business_id: BusinessId,
    business_id: BusinessId,
) -> bool {
    transaction_business_id == business_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_creation_when_business_is_deleted() {
        assert_eq!(
            ensure_business_is_active(true),
            Err(WorkshopError::BusinessIsDeleted)
        );
    }

    #[test]
    fn allows_creation_when_business_is_active() {
        assert_eq!(ensure_business_is_active(false), Ok(()));
    }

    #[test]
    fn rejects_update_when_version_mismatch() {
        assert_eq!(
            ensure_version_matches(1, 2),
            Err(WorkshopError::VersionConflict)
        );
    }

    #[test]
    fn allows_update_when_version_matches() {
        assert_eq!(ensure_version_matches(2, 2), Ok(()));
    }

    #[test]
    fn transaction_belongs_to_business_true_when_same_business() {
        let business_id = BusinessId::new();
        assert!(transaction_belongs_to_business(business_id, business_id));
    }

    #[test]
    fn transaction_belongs_to_business_false_when_different_business() {
        assert!(!transaction_belongs_to_business(
            BusinessId::new(),
            BusinessId::new()
        ));
    }
}
