use thiserror::Error;

/// Error khusus Capability Workshop — SENGAJA terpisah dari
/// `domain::DomainError`. Core Domain tidak boleh "mengenal" konsep
/// Workshop (lihat catatan analisis: Capability dibangun di atas Core,
/// bukan bagian dari Core). Kalau nanti ada Capability lain (Laundry,
/// Klinik, dll), masing-masing akan punya error type sendiri dengan pola
/// yang sama, bukan menumpuk semua ke satu enum raksasa.
///
/// Berisi CAMPURAN error validasi Value Object (`EmptyDescription`, dst)
/// dan pelanggaran business rule (`BusinessIsDeleted`, `VersionConflict`)
/// — pola yang sama seperti `domain::DomainError` di Core, bukan
/// dipisah jadi banyak enum kecil untuk kasus sekecil ini.
///
/// Pakai `thiserror` — lihat catatan di `domain::DomainError` soal
/// pembagian tanggung jawab: `#[error("...")]` tetap Bahasa Indonesia
/// (pesan default/developer-facing), `code()` untuk terjemahan
/// multi-bahasa di lapisan API.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkshopError {
    #[error("deskripsi tidak boleh kosong")]
    EmptyDescription,
    #[error("deskripsi tidak boleh lebih dari {max} karakter")]
    DescriptionTooLong { max: usize },
    #[error("format id tidak valid")]
    InvalidId,
    /// Satu varian generik untuk SEMUA transisi status yang tidak valid
    /// (mis. `cancel()` pada order yang sudah `Completed`, `complete()`
    /// pada order yang masih `Received`). Sengaja tidak dipecah per
    /// transisi — jumlah kombinasinya kecil dan pesannya sudah cukup
    /// jelas lewat status saat ini vs status tujuan.
    #[error("tidak bisa mengubah status dari {from} ke {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
    /// ServiceOrder tidak boleh dibuat di bawah Business yang sudah
    /// di-soft-delete. SENGAJA dicek ulang di sini (bukan memanggil
    /// `domain::rules::ensure_business_is_active` yang mengembalikan
    /// `DomainError`) — supaya `WorkshopService` tidak perlu menerjemahkan
    /// error Core ke error Workshop bolak-balik untuk satu pengecekan
    /// boolean sederhana. Lihat `rules.rs`.
    #[error("tidak bisa membuat service order baru karena business sudah dihapus")]
    BusinessIsDeleted,
    /// Optimistic locking di level business rule: `expected_version` yang
    /// dikirim client tidak sama dengan `version` yang tersimpan.
    #[error("versi data tidak sesuai, kemungkinan data sudah diubah pihak lain")]
    VersionConflict,
    /// Nilai status yang tersimpan di database tidak dikenal
    /// `ServiceOrderStatus`. Seharusnya tidak pernah terjadi lewat jalur
    /// normal aplikasi — lihat komentar di `ServiceOrderStatus::from_str`.
    #[error("status service order tidak dikenal: {value}")]
    UnknownStatus { value: String },
    /// ServiceOrder tidak boleh dibuat untuk Customer yang bukan milik
    /// Business yang sama (gap #3: validasi customer_id lintas-aggregate).
    /// SENGAJA dipetakan ke pesan/status yang SAMA seperti "tidak
    /// ditemukan" (404, bukan 409) — analog
    /// `ApplicationError::CustomerNotFound` di Core — supaya client tidak
    /// bisa membedakan "customer_id salah" dari "customer_id itu milik
    /// business/tenant lain" (info-hiding, lihat diskusi desain gap #3).
    #[error("customer tidak ditemukan")]
    CustomerNotFound,
    /// ServiceOrder tidak boleh di-`complete()` dengan `transaction_id`
    /// yang bukan milik Business yang sama (pola identik dengan
    /// `CustomerNotFound` di atas, menutup celah yang sama untuk
    /// `transaction_id`). SENGAJA dipetakan ke pesan/status yang SAMA
    /// seperti "tidak ditemukan" (404, bukan 409) untuk alasan info-hiding
    /// yang sama.
    #[error("transaction tidak ditemukan")]
    TransactionNotFound,
}

impl WorkshopError {
    /// Identitas stabil, bahasa-independen — pola sama seperti
    /// `domain::DomainError::code()`.
    pub fn code(&self) -> &'static str {
        match self {
            WorkshopError::EmptyDescription => "empty_description",
            WorkshopError::DescriptionTooLong { .. } => "description_too_long",
            WorkshopError::InvalidId => "invalid_id",
            WorkshopError::InvalidTransition { .. } => "invalid_transition",
            WorkshopError::BusinessIsDeleted => "business_is_deleted",
            WorkshopError::VersionConflict => "version_conflict",
            WorkshopError::UnknownStatus { .. } => "unknown_status",
            WorkshopError::CustomerNotFound => "customer_not_found",
            WorkshopError::TransactionNotFound => "transaction_not_found",
        }
    }
}

/// Kegagalan Repository (infrastruktur) — pola sama persis seperti
/// `application::RepositoryError` di Core.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("penyimpanan data tidak dapat diakses: {0}")]
    Unavailable(String),
    /// Terdeteksi di level penyimpanan lewat conditional UPDATE ...
    /// WHERE version = versi_lama, 0 baris ter-update. Beda dari
    /// `WorkshopError::VersionConflict` yang mendeteksi dari input
    /// klien — sama seperti Core, keduanya disatukan lagi di
    /// `ServiceOrderError`.
    #[error("data sudah diubah pihak lain sejak terakhir dibaca (terdeteksi di penyimpanan)")]
    VersionConflict,
}

impl RepositoryError {
    /// Identitas stabil, bahasa-independen — pola sama seperti
    /// `application::RepositoryError::code()`.
    pub fn code(&self) -> &'static str {
        match self {
            RepositoryError::Unavailable(_) => "repository_unavailable",
            RepositoryError::VersionConflict => "repository_version_conflict",
        }
    }
}

/// Error di level Application Service Workshop — gabungan pelanggaran
/// business rule (`WorkshopError`), kegagalan infrastruktur
/// (`RepositoryError`), atau entity yang tidak ditemukan. Pola sama
/// persis seperti `application::ApplicationError` di Core.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceOrderError {
    /// `#[from]`: `From<WorkshopError>` di-generate otomatis, pembungkusan
    /// lurus (bukan pemetaan ulang) — pola sama seperti
    /// `ApplicationError::Domain` di Core.
    #[error(transparent)]
    Workshop(#[from] WorkshopError),
    /// Sengaja TIDAK pakai `#[from]` — konversi `RepositoryError` butuh
    /// logika kustom (`VersionConflict` dipetakan ulang ke varian
    /// Workshop, lihat `impl From<RepositoryError>` di bawah).
    #[error(transparent)]
    Repository(RepositoryError),
    #[error("service order tidak ditemukan")]
    ServiceOrderNotFound,
}

impl ServiceOrderError {
    /// Identitas stabil, bahasa-independen — didelegasikan ke error di
    /// dalamnya untuk varian pembungkus, pola sama seperti
    /// `application::ApplicationError::code()`.
    pub fn code(&self) -> &'static str {
        match self {
            ServiceOrderError::Workshop(err) => err.code(),
            ServiceOrderError::Repository(err) => err.code(),
            ServiceOrderError::ServiceOrderNotFound => "service_order_not_found",
        }
    }
}

impl From<RepositoryError> for ServiceOrderError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::VersionConflict => {
                ServiceOrderError::Workshop(WorkshopError::VersionConflict)
            }
            other => ServiceOrderError::Repository(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_delegates_to_inner_workshop_error() {
        let err = ServiceOrderError::Workshop(WorkshopError::EmptyDescription);
        assert_eq!(err.code(), "empty_description");
    }

    #[test]
    fn code_delegates_to_inner_repository_error() {
        let err = ServiceOrderError::Repository(RepositoryError::VersionConflict);
        assert_eq!(err.code(), "repository_version_conflict");
    }

    #[test]
    fn display_still_produces_bahasa_indonesia_default_message() {
        assert_eq!(
            ServiceOrderError::ServiceOrderNotFound.to_string(),
            "service order tidak ditemukan"
        );
        assert_eq!(
            ServiceOrderError::Workshop(WorkshopError::EmptyDescription).to_string(),
            "deskripsi tidak boleh kosong"
        );
    }
}
