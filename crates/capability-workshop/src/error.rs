use std::fmt;

/// Error khusus Capability Workshop — SENGAJA terpisah dari
/// `domain::DomainError`. Core Domain tidak boleh "mengenal" konsep
/// Workshop (lihat catatan analisis: Capability dibangun di atas Core,
/// bukan bagian dari Core). Kalau nanti ada Capability lain (Laundry,
/// Klinik, dll), masing-masing akan punya error type sendiri dengan pola
/// yang sama, bukan menumpuk semua ke satu enum raksasa.
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
        }
    }
}

impl std::error::Error for WorkshopError {}
