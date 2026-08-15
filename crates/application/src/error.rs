use domain::DomainError;
use thiserror::Error;

/// Kegagalan Repository (infrastruktur). Masih generik karena belum ada
/// implementasi konkret (Postgres dll) — akan diperkaya nanti sesuai
/// kebutuhan nyata dari implementasi tersebut.
///
/// Pakai `thiserror` — lihat catatan di `domain::DomainError` soal
/// pembagian tanggung jawab: `#[error("...")]` di bawah tetap Bahasa
/// Indonesia (pesan default/developer-facing), `code()` yang dipakai
/// lapisan API untuk terjemahan multi-bahasa.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("penyimpanan data tidak dapat diakses: {0}")]
    Unavailable(String),
    /// Update gagal karena versi baris di penyimpanan sudah berubah sejak
    /// terakhir dibaca (terdeteksi lewat conditional UPDATE ... WHERE
    /// version = versi_lama, 0 baris ter-update). Beda dari
    /// `DomainError::VersionConflict` yang mendeteksi versi tidak cocok
    /// dari INPUT klien — ini terdeteksi di penyimpanan itu sendiri
    /// (menjaga dari race condition antar-request, bukan cuma klien basi).
    #[error("data sudah diubah pihak lain sejak terakhir dibaca (terdeteksi di penyimpanan)")]
    VersionConflict,
    /// Constraint UNIQUE di database menolak insert/update (mis. dua
    /// request nyaris bersamaan lolos pengecekan `ensure_business_name_unique`
    /// di Application Service — sama seperti VersionConflict, ini
    /// jaring pengaman di level penyimpanan, bukan pengganti pengecekan
    /// business rule.
    #[error("data duplikat ditolak oleh constraint unik di penyimpanan")]
    UniqueConstraintViolation,
}

impl RepositoryError {
    /// Identitas stabil, bahasa-independen — pola sama seperti
    /// `DomainError::code()`.
    pub fn code(&self) -> &'static str {
        match self {
            RepositoryError::Unavailable(_) => "repository_unavailable",
            RepositoryError::VersionConflict => "repository_version_conflict",
            RepositoryError::UniqueConstraintViolation => "repository_unique_constraint_violation",
        }
    }
}

/// Error di level Application Service: gabungan dari pelanggaran business
/// rule (Domain), kegagalan infrastruktur (Repository), atau entity yang
/// tidak ditemukan.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplicationError {
    /// `#[error(transparent)]` + `#[from]`: Display dan `source()` di-
    /// delegasikan langsung ke `DomainError`, dan `From<DomainError>`
    /// di-generate otomatis (menggantikan impl manual yang dulu ada di
    /// sini) — perilakunya identik, hanya dibuat oleh macro.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// Sengaja TIDAK pakai `#[from]` — konversi `RepositoryError` ->
    /// `ApplicationError` butuh logika kustom (VersionConflict/
    /// UniqueConstraintViolation dipetakan ulang ke varian Domain, lihat
    /// `impl From<RepositoryError>` di bawah), bukan pembungkusan lurus.
    #[error(transparent)]
    Repository(RepositoryError),
    #[error("tenant tidak ditemukan")]
    TenantNotFound,
    #[error("business tidak ditemukan")]
    BusinessNotFound,
    #[error("customer tidak ditemukan")]
    CustomerNotFound,
    #[error("transaction tidak ditemukan")]
    TransactionNotFound,
    #[error("relationship tidak ditemukan")]
    RelationshipNotFound,
    #[error("interaction tidak ditemukan")]
    InteractionNotFound,
}

impl ApplicationError {
    /// Identitas stabil, bahasa-independen. Untuk varian pembungkus
    /// (`Domain`/`Repository`), didelegasikan ke `code()` error di
    /// dalamnya — supaya lapisan API cukup satu kali lookup tanpa perlu
    /// tahu apakah errornya berasal dari Domain atau Repository.
    pub fn code(&self) -> &'static str {
        match self {
            ApplicationError::Domain(err) => err.code(),
            ApplicationError::Repository(err) => err.code(),
            ApplicationError::TenantNotFound => "tenant_not_found",
            ApplicationError::BusinessNotFound => "business_not_found",
            ApplicationError::CustomerNotFound => "customer_not_found",
            ApplicationError::TransactionNotFound => "transaction_not_found",
            ApplicationError::RelationshipNotFound => "relationship_not_found",
            ApplicationError::InteractionNotFound => "interaction_not_found",
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_delegates_to_inner_domain_error() {
        let err = ApplicationError::Domain(DomainError::EmptyName);
        assert_eq!(err.code(), "empty_name");
    }

    #[test]
    fn code_delegates_to_inner_repository_error() {
        let err = ApplicationError::Repository(RepositoryError::VersionConflict);
        assert_eq!(err.code(), "repository_version_conflict");
    }

    #[test]
    fn not_found_variants_have_their_own_code() {
        assert_eq!(ApplicationError::TenantNotFound.code(), "tenant_not_found");
    }

    #[test]
    fn display_still_produces_bahasa_indonesia_default_message() {
        assert_eq!(
            ApplicationError::TenantNotFound.to_string(),
            "tenant tidak ditemukan"
        );
        assert_eq!(
            ApplicationError::Domain(DomainError::EmptyName).to_string(),
            "nama tidak boleh kosong"
        );
    }
}
