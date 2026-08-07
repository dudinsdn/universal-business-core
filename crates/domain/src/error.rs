use std::fmt;

/// Error domain: pelanggaran validasi Value Object atau Business Rule.
///
/// Sengaja tidak pakai crate `thiserror` di tahap ini — jumlah varian masih
/// sedikit dan implementasi manual di bawah sudah cukup sederhana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyName,
    NameTooLong {
        max: usize,
    },
    EmptyBusinessType,
    InvalidBusinessType,
    DuplicateBusinessName,
    TenantHasActiveBusiness,
    TenantIsDeleted,
    /// Analog `TenantIsDeleted`, tapi untuk aggregate Business: Customer
    /// tidak boleh dibuat di bawah Business yang sudah di-soft-delete.
    BusinessIsDeleted,
    VersionConflict,
    InvalidId,
    /// Query param `updated_since` (endpoint incremental sync) tidak
    /// berupa timestamp RFC 3339 yang valid.
    InvalidTimestamp,
    /// `CustomerPhone::new` dipanggil dengan string kosong. Beda dari
    /// "tidak punya telepon" (`None`) — itu bukan error, cukup jangan
    /// panggil `CustomerPhone::new` sama sekali.
    EmptyPhone,
    PhoneTooLong {
        max: usize,
    },
    InvalidPhone,
    /// `TransactionKind::new` dipanggil dengan string kosong.
    EmptyTransactionKind,
    TransactionKindTooLong {
        max: usize,
    },
    InvalidTransactionKind,
    /// `TransactionAmount::new` dipanggil dengan nilai <= 0.
    InvalidAmount,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::EmptyName => write!(f, "nama tidak boleh kosong"),
            DomainError::NameTooLong { max } => {
                write!(f, "nama tidak boleh lebih dari {max} karakter")
            }
            DomainError::EmptyBusinessType => write!(f, "jenis usaha tidak boleh kosong"),
            DomainError::InvalidBusinessType => write!(
                f,
                "jenis usaha hanya boleh berisi huruf, angka, underscore, atau hyphen"
            ),
            DomainError::DuplicateBusinessName => {
                write!(f, "nama business sudah digunakan pada tenant ini")
            }
            DomainError::TenantHasActiveBusiness => write!(
                f,
                "tenant tidak bisa dihapus karena masih memiliki business aktif"
            ),
            DomainError::TenantIsDeleted => write!(
                f,
                "tidak bisa membuat business baru karena tenant sudah dihapus"
            ),
            DomainError::BusinessIsDeleted => write!(
                f,
                "tidak bisa membuat customer baru karena business sudah dihapus"
            ),
            DomainError::VersionConflict => write!(
                f,
                "versi data tidak sesuai, kemungkinan data sudah diubah pihak lain"
            ),
            DomainError::InvalidId => write!(f, "format id tidak valid"),
            DomainError::InvalidTimestamp => write!(
                f,
                "format waktu tidak valid, gunakan format RFC 3339 (mis. 2026-08-01T00:00:00Z)"
            ),
            DomainError::EmptyPhone => write!(f, "nomor telepon tidak boleh kosong"),
            DomainError::PhoneTooLong { max } => {
                write!(f, "nomor telepon tidak boleh lebih dari {max} karakter")
            }
            DomainError::InvalidPhone => write!(
                f,
                "nomor telepon hanya boleh berisi angka, spasi, tanda +, -, ( atau )"
            ),
            DomainError::EmptyTransactionKind => write!(f, "jenis transaksi tidak boleh kosong"),
            DomainError::TransactionKindTooLong { max } => {
                write!(f, "jenis transaksi tidak boleh lebih dari {max} karakter")
            }
            DomainError::InvalidTransactionKind => write!(
                f,
                "jenis transaksi hanya boleh berisi huruf, angka, underscore, atau hyphen"
            ),
            DomainError::InvalidAmount => write!(f, "nilai transaksi harus lebih besar dari nol"),
        }
    }
}

impl std::error::Error for DomainError {}
