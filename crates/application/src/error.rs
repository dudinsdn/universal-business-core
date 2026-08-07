use std::fmt;

use domain::DomainError;

/// Kegagalan Repository (infrastruktur). Masih generik karena belum ada
/// implementasi konkret (Postgres dll) — akan diperkaya nanti sesuai
/// kebutuhan nyata dari implementasi tersebut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Unavailable(String),
    /// Update gagal karena versi baris di penyimpanan sudah berubah sejak
    /// terakhir dibaca (terdeteksi lewat conditional UPDATE ... WHERE
    /// version = versi_lama, 0 baris ter-update). Beda dari
    /// `DomainError::VersionConflict` yang mendeteksi versi tidak cocok
    /// dari INPUT klien — ini terdeteksi di penyimpanan itu sendiri
    /// (menjaga dari race condition antar-request, bukan cuma klien basi).
    VersionConflict,
    /// Constraint UNIQUE di database menolak insert/update (mis. dua
    /// request nyaris bersamaan lolos pengecekan `ensure_business_name_unique`
    /// di Application Service — sama seperti VersionConflict, ini
    /// jaring pengaman di level penyimpanan, bukan pengganti pengecekan
    /// business rule.
    UniqueConstraintViolation,
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryError::Unavailable(reason) => {
                write!(f, "penyimpanan data tidak dapat diakses: {reason}")
            }
            RepositoryError::VersionConflict => write!(
                f,
                "data sudah diubah pihak lain sejak terakhir dibaca (terdeteksi di penyimpanan)"
            ),
            RepositoryError::UniqueConstraintViolation => {
                write!(
                    f,
                    "data duplikat ditolak oleh constraint unik di penyimpanan"
                )
            }
        }
    }
}

impl std::error::Error for RepositoryError {}

/// Error di level Application Service: gabungan dari pelanggaran business
/// rule (Domain), kegagalan infrastruktur (Repository), atau entity yang
/// tidak ditemukan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    Domain(DomainError),
    Repository(RepositoryError),
    TenantNotFound,
    BusinessNotFound,
    CustomerNotFound,
    TransactionNotFound,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicationError::Domain(err) => write!(f, "{err}"),
            ApplicationError::Repository(err) => write!(f, "{err}"),
            ApplicationError::TenantNotFound => write!(f, "tenant tidak ditemukan"),
            ApplicationError::BusinessNotFound => write!(f, "business tidak ditemukan"),
            ApplicationError::CustomerNotFound => write!(f, "customer tidak ditemukan"),
            ApplicationError::TransactionNotFound => write!(f, "transaction tidak ditemukan"),
        }
    }
}

impl std::error::Error for ApplicationError {}

impl From<DomainError> for ApplicationError {
    fn from(err: DomainError) -> Self {
        ApplicationError::Domain(err)
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(err: RepositoryError) -> Self {
        match err {
            // Disatukan dengan DomainError::VersionConflict: dari sudut
            // pandang pemanggil (API dll), keduanya sama-sama berarti
            // "coba lagi dengan data terbaru" — sumbernya beda (klien basi
            // vs race condition di penyimpanan), efeknya sama.
            RepositoryError::VersionConflict => {
                ApplicationError::Domain(DomainError::VersionConflict)
            }
            RepositoryError::UniqueConstraintViolation => {
                ApplicationError::Domain(DomainError::DuplicateBusinessName)
            }
            other => ApplicationError::Repository(other),
        }
    }
}
