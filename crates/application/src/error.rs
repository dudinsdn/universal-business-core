use std::fmt;

use domain::DomainError;

/// Kegagalan Repository (infrastruktur). Masih generik karena belum ada
/// implementasi konkret (Postgres dll) — akan diperkaya nanti sesuai
/// kebutuhan nyata dari implementasi tersebut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Unavailable(String),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryError::Unavailable(reason) => {
                write!(f, "penyimpanan data tidak dapat diakses: {reason}")
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
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicationError::Domain(err) => write!(f, "{err}"),
            ApplicationError::Repository(err) => write!(f, "{err}"),
            ApplicationError::TenantNotFound => write!(f, "tenant tidak ditemukan"),
            ApplicationError::BusinessNotFound => write!(f, "business tidak ditemukan"),
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
        ApplicationError::Repository(err)
    }
}
