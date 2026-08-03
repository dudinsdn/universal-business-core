use std::fmt;

/// Error domain: pelanggaran validasi Value Object atau Business Rule.
///
/// Sengaja tidak pakai crate `thiserror` di tahap ini — jumlah varian masih
/// sedikit dan implementasi manual di bawah sudah cukup sederhana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    EmptyName,
    NameTooLong { max: usize },
    EmptyBusinessType,
    InvalidBusinessType,
    DuplicateBusinessName,
    TenantHasActiveBusiness,
    TenantIsDeleted,
    VersionConflict,
    InvalidId,
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
            DomainError::VersionConflict => write!(
                f,
                "versi data tidak sesuai, kemungkinan data sudah diubah pihak lain"
            ),
            DomainError::InvalidId => write!(f, "format id tidak valid"),
        }
    }
}

impl std::error::Error for DomainError {}
