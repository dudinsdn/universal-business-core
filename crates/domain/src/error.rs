use thiserror::Error;

/// Error domain: pelanggaran validasi Value Object atau Business Rule.
///
/// Menggunakan `thiserror` untuk generate `Display` dan `std::error::Error`
/// otomatis dari atribut `#[error("...")]` di setiap varian. Pesan di
/// atribut tersebut TETAP Bahasa Indonesia — perannya sekarang adalah
/// pesan default/developer-facing (untuk log, atau fallback kalau lapisan
/// di atas belum menerjemahkan), BUKAN satu-satunya representasi error.
///
/// Untuk dukungan multi-bahasa: setiap varian juga punya identitas stabil
/// via `code()` (lihat di bawah). Lapisan HTTP (`api`/`capability-workshop`)
/// yang nanti menerjemahkan memakai `code()` + field terkait (mis. `max`)
/// untuk mencari template pesan sesuai bahasa yang diminta client — BUKAN
/// dengan mem-parsing string `Display` di sini. Domain sengaja tidak tahu
/// dan tidak peduli soal bahasa; itu tanggung jawab lapisan API.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("nama tidak boleh kosong")]
    EmptyName,
    #[error("nama tidak boleh lebih dari {max} karakter")]
    NameTooLong { max: usize },
    #[error("jenis usaha tidak boleh kosong")]
    EmptyBusinessType,
    #[error("jenis usaha hanya boleh berisi huruf, angka, underscore, atau hyphen")]
    InvalidBusinessType,
    #[error("nama business sudah digunakan pada tenant ini")]
    DuplicateBusinessName,
    #[error("tenant tidak bisa dihapus karena masih memiliki business aktif")]
    TenantHasActiveBusiness,
    #[error("tidak bisa membuat business baru karena tenant sudah dihapus")]
    TenantIsDeleted,
    /// Analog `TenantIsDeleted`, tapi untuk aggregate Business: Customer
    /// tidak boleh dibuat di bawah Business yang sudah di-soft-delete.
    #[error("tidak bisa membuat customer baru karena business sudah dihapus")]
    BusinessIsDeleted,
    #[error("versi data tidak sesuai, kemungkinan data sudah diubah pihak lain")]
    VersionConflict,
    #[error("format id tidak valid")]
    InvalidId,
    /// Query param `updated_since` (endpoint incremental sync) tidak
    /// berupa timestamp RFC 3339 yang valid.
    #[error("format waktu tidak valid, gunakan format RFC 3339 (mis. 2026-08-01T00:00:00Z)")]
    InvalidTimestamp,
    /// `CustomerPhone::new` dipanggil dengan string kosong. Beda dari
    /// "tidak punya telepon" (`None`) — itu bukan error, cukup jangan
    /// panggil `CustomerPhone::new` sama sekali.
    #[error("nomor telepon tidak boleh kosong")]
    EmptyPhone,
    #[error("nomor telepon tidak boleh lebih dari {max} karakter")]
    PhoneTooLong { max: usize },
    #[error("nomor telepon hanya boleh berisi angka, spasi, tanda +, -, ( atau )")]
    InvalidPhone,
    /// `TransactionKind::new` dipanggil dengan string kosong.
    #[error("jenis transaksi tidak boleh kosong")]
    EmptyTransactionKind,
    #[error("jenis transaksi tidak boleh lebih dari {max} karakter")]
    TransactionKindTooLong { max: usize },
    #[error("jenis transaksi hanya boleh berisi huruf, angka, underscore, atau hyphen")]
    InvalidTransactionKind,
    /// `TransactionAmount::new` dipanggil dengan nilai <= 0.
    #[error("nilai transaksi harus lebih besar dari nol")]
    InvalidAmount,
    /// `RelationshipType::new` dipanggil dengan string kosong.
    #[error("jenis hubungan tidak boleh kosong")]
    EmptyRelationshipType,
    #[error("jenis hubungan tidak boleh lebih dari {max} karakter")]
    RelationshipTypeTooLong { max: usize },
    #[error("jenis hubungan hanya boleh berisi huruf, angka, underscore, atau hyphen")]
    InvalidRelationshipType,
    /// `Relationship::new`/`with_id` dipanggil dengan `from_customer_id`
    /// sama dengan `to_customer_id` — Customer tidak bisa berelasi dengan
    /// dirinya sendiri.
    #[error("customer tidak bisa berelasi dengan dirinya sendiri")]
    SelfRelationship,
    /// `InteractionType::new` dipanggil dengan string kosong.
    #[error("jenis interaksi tidak boleh kosong")]
    EmptyInteractionType,
    #[error("jenis interaksi tidak boleh lebih dari {max} karakter")]
    InteractionTypeTooLong { max: usize },
    #[error("jenis interaksi hanya boleh berisi huruf, angka, underscore, atau hyphen")]
    InvalidInteractionType,
    /// `InteractionNote::new` dipanggil dengan string kosong. Beda dari
    /// "tidak ada catatan" (`None`) — itu bukan error, cukup jangan
    /// panggil `InteractionNote::new` sama sekali.
    #[error("catatan tidak boleh kosong")]
    EmptyInteractionNote,
    #[error("catatan tidak boleh lebih dari {max} karakter")]
    InteractionNoteTooLong { max: usize },
}

impl DomainError {
    /// Identitas stabil, bahasa-independen untuk tiap varian — kunci yang
    /// dipakai lapisan API untuk mencari terjemahan pesan (mis. lookup ke
    /// tabel `{code, locale} -> template`), bukan nama varian Rust
    /// langsung, supaya identitas ini tidak diam-diam berubah kalau nama
    /// varian di-refactor.
    ///
    /// Field seperti `max` pada `NameTooLong` dsb. tetap bisa diambil
    /// lewat pattern matching pada `DomainError` itu sendiri (sudah
    /// public) — `code()` cuma menjawab "pesan mana yang harus dicari",
    /// bukan "apa isi parameternya".
    pub fn code(&self) -> &'static str {
        match self {
            DomainError::EmptyName => "empty_name",
            DomainError::NameTooLong { .. } => "name_too_long",
            DomainError::EmptyBusinessType => "empty_business_type",
            DomainError::InvalidBusinessType => "invalid_business_type",
            DomainError::DuplicateBusinessName => "duplicate_business_name",
            DomainError::TenantHasActiveBusiness => "tenant_has_active_business",
            DomainError::TenantIsDeleted => "tenant_is_deleted",
            DomainError::BusinessIsDeleted => "business_is_deleted",
            DomainError::VersionConflict => "version_conflict",
            DomainError::InvalidId => "invalid_id",
            DomainError::InvalidTimestamp => "invalid_timestamp",
            DomainError::EmptyPhone => "empty_phone",
            DomainError::PhoneTooLong { .. } => "phone_too_long",
            DomainError::InvalidPhone => "invalid_phone",
            DomainError::EmptyTransactionKind => "empty_transaction_kind",
            DomainError::TransactionKindTooLong { .. } => "transaction_kind_too_long",
            DomainError::InvalidTransactionKind => "invalid_transaction_kind",
            DomainError::InvalidAmount => "invalid_amount",
            DomainError::EmptyRelationshipType => "empty_relationship_type",
            DomainError::RelationshipTypeTooLong { .. } => "relationship_type_too_long",
            DomainError::InvalidRelationshipType => "invalid_relationship_type",
            DomainError::SelfRelationship => "self_relationship",
            DomainError::EmptyInteractionType => "empty_interaction_type",
            DomainError::InteractionTypeTooLong { .. } => "interaction_type_too_long",
            DomainError::InvalidInteractionType => "invalid_interaction_type",
            DomainError::EmptyInteractionNote => "empty_interaction_note",
            DomainError::InteractionNoteTooLong { .. } => "interaction_note_too_long",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_stable_and_language_independent_for_representative_variants() {
        assert_eq!(DomainError::EmptyName.code(), "empty_name");
        assert_eq!(
            DomainError::NameTooLong { max: 255 }.code(),
            "name_too_long"
        );
        assert_eq!(DomainError::VersionConflict.code(), "version_conflict");
        assert_eq!(DomainError::SelfRelationship.code(), "self_relationship");
    }

    #[test]
    fn display_still_produces_bahasa_indonesia_default_message() {
        assert_eq!(
            DomainError::EmptyName.to_string(),
            "nama tidak boleh kosong"
        );
        assert_eq!(
            DomainError::NameTooLong { max: 255 }.to_string(),
            "nama tidak boleh lebih dari 255 karakter"
        );
    }
}
