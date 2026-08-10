use std::fmt;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkshopError {
    EmptyDescription,
    DescriptionTooLong {
        max: usize,
    },
    InvalidId,
    /// Satu varian generik untuk SEMUA transisi status yang tidak valid
    /// (mis. `cancel()` pada order yang sudah `Completed`, `complete()`
    /// pada order yang masih `Received`). Sengaja tidak dipecah per
    /// transisi — jumlah kombinasinya kecil dan pesannya sudah cukup
    /// jelas lewat status saat ini vs status tujuan.
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
    BusinessIsDeleted,
    /// Optimistic locking di level business rule: `expected_version` yang
    /// dikirim client tidak sama dengan `version` yang tersimpan.
    VersionConflict,
    /// Nilai status yang tersimpan di database tidak dikenal
    /// `ServiceOrderStatus`. Seharusnya tidak pernah terjadi lewat jalur
    /// normal aplikasi — lihat komentar di `ServiceOrderStatus::from_str`.
    UnknownStatus {
        value: String,
    },
}

impl fmt::Display for WorkshopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkshopError::EmptyDescription => write!(f, "deskripsi tidak boleh kosong"),
            WorkshopError::DescriptionTooLong { max } => {
                write!(f, "deskripsi tidak boleh lebih dari {max} karakter")
            }
            WorkshopError::InvalidId => write!(f, "format id tidak valid"),
            WorkshopError::InvalidTransition { from, to } => {
                write!(f, "tidak bisa mengubah status dari {from} ke {to}")
            }
            WorkshopError::BusinessIsDeleted => write!(
                f,
                "tidak bisa membuat service order baru karena business sudah dihapus"
            ),
            WorkshopError::VersionConflict => write!(
                f,
                "versi data tidak sesuai, kemungkinan data sudah diubah pihak lain"
            ),
            WorkshopError::UnknownStatus { value } => {
                write!(f, "status service order tidak dikenal: {value}")
            }
        }
    }
}

impl std::error::Error for WorkshopError {}

/// Kegagalan Repository (infrastruktur) — pola sama persis seperti
/// `application::RepositoryError` di Core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Unavailable(String),
    /// Terdeteksi di level penyimpanan lewat conditional UPDATE ...
    /// WHERE version = versi_lama, 0 baris ter-update. Beda dari
    /// `WorkshopError::VersionConflict` yang mendeteksi dari input
    /// klien — sama seperti Core, keduanya disatukan lagi di
    /// `ServiceOrderError`.
    VersionConflict,
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
        }
    }
}

impl std::error::Error for RepositoryError {}

/// Error di level Application Service Workshop — gabungan pelanggaran
/// business rule (`WorkshopError`), kegagalan infrastruktur
/// (`RepositoryError`), atau entity yang tidak ditemukan. Pola sama
/// persis seperti `application::ApplicationError` di Core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceOrderError {
    Workshop(WorkshopError),
    Repository(RepositoryError),
    ServiceOrderNotFound,
}

impl fmt::Display for ServiceOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceOrderError::Workshop(err) => write!(f, "{err}"),
            ServiceOrderError::Repository(err) => write!(f, "{err}"),
            ServiceOrderError::ServiceOrderNotFound => write!(f, "service order tidak ditemukan"),
        }
    }
}

impl std::error::Error for ServiceOrderError {}

impl From<WorkshopError> for ServiceOrderError {
    fn from(err: WorkshopError) -> Self {
        ServiceOrderError::Workshop(err)
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
