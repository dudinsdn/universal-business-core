//! Entity Interaction: catatan kontak antara Business dengan satu
//! Customer (panggilan, kunjungan, chat, email, catatan follow-up, dll).
//!
//! Beda dari Relationship (struktur hubungan peer-to-peer antar dua
//! Customer setara) dan Transaction (kejadian bernilai uang), Interaction
//! berpusat pada "kontak apa yang terjadi dan kapan", searah dari sudut
//! pandang Business terhadap SATU Customer.
//!
//! Beda dari Transaction: `customer_id` di sini WAJIB (bukan opsional) —
//! Interaction secara alami selalu tentang seseorang (keputusan Din).
//!
//! Cakupan SENGAJA dibuat sempit di tahap ini: tidak ada isi pesan
//! lengkap/transkrip (hanya `note` pendek opsional, bukan data besar atau
//! sensitif), tidak ada status resolusi (ticketing), tidak ada penugasan
//! staff/agent (belum ada domain User/Staff di Core). Semua itu bisa
//! diperluas nanti kalau memang ada kebutuhan capability yang konkret.
//!
//! Sama seperti Transaction/Relationship, Interaction TIDAK bisa diubah
//! isinya setelah dibuat — hanya `soft_delete`. Catatan historis
//! dikoreksi lewat entry baru, bukan diedit.

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::business::BusinessId;
use crate::customer::CustomerId;
use crate::error::DomainError;

const MAX_TYPE_LENGTH: usize = 64;
const MAX_NOTE_LENGTH: usize = 500;

/// Identitas unik Interaction. Selalu berupa UUID v7 — pola sama seperti
/// `TransactionId`/`RelationshipId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractionId(Uuid);

impl InteractionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for InteractionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InteractionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for InteractionId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidId)
    }
}

/// Jenis kontak. Sengaja berupa string terbuka (pola sama seperti
/// `TransactionKind`/`RelationshipType`), BUKAN enum tertutup — supaya
/// capability bisa mendefinisikan jenisnya sendiri (mis. "call", "visit",
/// "chat", "email") tanpa perlu mengubah Core Domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InteractionType(String);

impl InteractionType {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let normalized = raw.into().trim().to_lowercase();
        if normalized.is_empty() {
            return Err(DomainError::EmptyInteractionType);
        }
        if normalized.chars().count() > MAX_TYPE_LENGTH {
            return Err(DomainError::InteractionTypeTooLong {
                max: MAX_TYPE_LENGTH,
            });
        }
        let is_valid = normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !is_valid {
            return Err(DomainError::InvalidInteractionType);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catatan singkat opsional tentang kontak yang terjadi. SELALU dibungkus
/// `Option<InteractionNote>` di level `Interaction` — kalau tidak ada
/// catatan, field-nya `None`, BUKAN `InteractionNote` berisi string
/// kosong (pola sama seperti `CustomerPhone`).
///
/// SENGAJA dibatasi pendek (maks 500 karakter) — ini ringkasan singkat,
/// BUKAN transkrip lengkap atau isi pesan penuh (lihat catatan modul).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InteractionNote(String);

impl InteractionNote {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyInteractionNote);
        }
        if trimmed.chars().count() > MAX_NOTE_LENGTH {
            return Err(DomainError::InteractionNoteTooLong {
                max: MAX_NOTE_LENGTH,
            });
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Data mentah untuk merekonstruksi Interaction dari penyimpanan. Sama
/// alasannya dengan `PersistedTransaction`/`PersistedRelationship`.
pub struct PersistedInteraction {
    pub id: InteractionId,
    pub business_id: BusinessId,
    pub customer_id: CustomerId,
    pub interaction_type: InteractionType,
    pub note: Option<InteractionNote>,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub version: u32,
}

/// Entity Interaction: satu catatan kontak antara Business dengan satu
/// Customer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interaction {
    id: InteractionId,
    business_id: BusinessId,
    customer_id: CustomerId,
    interaction_type: InteractionType,
    note: Option<InteractionNote>,
    occurred_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    version: u32,
}

impl Interaction {
    /// Membuat Interaction baru, dengan Id yang di-generate otomatis oleh
    /// sistem.
    ///
    /// PENTING: pengecekan lain (apakah Business masih aktif, apakah
    /// Customer benar-benar milik Business yang sama) TIDAK dilakukan di
    /// sini — itu business rule lintas-aggregate, tanggung jawab
    /// Application Service (belum diimplementasikan di tahap domain ini).
    ///
    /// `occurred_at` diterima sebagai parameter (bukan selalu
    /// `Utc::now()`) — pola sama seperti `Transaction::new`, kontak
    /// offline bisa dicatat belakangan tapi terjadi di waktu lampau.
    pub fn new(
        business_id: BusinessId,
        customer_id: CustomerId,
        interaction_type: InteractionType,
        note: Option<InteractionNote>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self::with_id(
            InteractionId::new(),
            business_id,
            customer_id,
            interaction_type,
            note,
            occurred_at,
        )
    }

    /// Membuat Interaction baru dengan Id yang SUDAH ditentukan
    /// (idempotent create) — pola sama seperti `Transaction::with_id`.
    pub fn with_id(
        id: InteractionId,
        business_id: BusinessId,
        customer_id: CustomerId,
        interaction_type: InteractionType,
        note: Option<InteractionNote>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            business_id,
            customer_id,
            interaction_type,
            note,
            occurred_at,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 0,
        }
    }

    pub fn id(&self) -> InteractionId {
        self.id
    }

    /// Merekonstruksi Interaction dari data yang SUDAH tersimpan. Dipakai
    /// HANYA oleh implementasi Repository konkret.
    pub fn from_persisted(data: PersistedInteraction) -> Self {
        Self {
            id: data.id,
            business_id: data.business_id,
            customer_id: data.customer_id,
            interaction_type: data.interaction_type,
            note: data.note,
            occurred_at: data.occurred_at,
            created_at: data.created_at,
            updated_at: data.updated_at,
            deleted_at: data.deleted_at,
            version: data.version,
        }
    }

    pub fn business_id(&self) -> BusinessId {
        self.business_id
    }

    pub fn customer_id(&self) -> CustomerId {
        self.customer_id
    }

    pub fn interaction_type(&self) -> &InteractionType {
        &self.interaction_type
    }

    pub fn note(&self) -> Option<&InteractionNote> {
        self.note.as_ref()
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    pub fn soft_delete(&mut self) {
        self.deleted_at = Some(Utc::now());
        self.touch();
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
        self.version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_business_id() -> BusinessId {
        BusinessId::new()
    }

    fn sample_type() -> InteractionType {
        InteractionType::new("call").unwrap()
    }

    #[test]
    fn interaction_id_roundtrips_through_uuid() {
        let id = InteractionId::new();
        let rebuilt = InteractionId::from_uuid(id.as_uuid());
        assert_eq!(id, rebuilt);
    }

    #[test]
    fn interaction_id_roundtrips_through_string() {
        let id = InteractionId::new();
        let parsed: InteractionId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn interaction_id_rejects_invalid_string() {
        assert_eq!(
            "not-a-uuid".parse::<InteractionId>(),
            Err(DomainError::InvalidId)
        );
    }

    #[test]
    fn interaction_type_normalizes_to_lowercase_and_trims() {
        let it = InteractionType::new("  Call  ").unwrap();
        assert_eq!(it.as_str(), "call");
    }

    #[test]
    fn interaction_type_rejects_empty() {
        assert_eq!(
            InteractionType::new("   "),
            Err(DomainError::EmptyInteractionType)
        );
    }

    #[test]
    fn interaction_type_rejects_invalid_characters() {
        assert_eq!(
            InteractionType::new("phone call!"),
            Err(DomainError::InvalidInteractionType)
        );
    }

    #[test]
    fn interaction_type_allows_underscore_and_hyphen() {
        assert!(InteractionType::new("follow_up").is_ok());
        assert!(InteractionType::new("follow-up").is_ok());
    }

    #[test]
    fn interaction_note_rejects_empty_string() {
        assert_eq!(
            InteractionNote::new(""),
            Err(DomainError::EmptyInteractionNote)
        );
        assert_eq!(
            InteractionNote::new("   "),
            Err(DomainError::EmptyInteractionNote)
        );
    }

    #[test]
    fn interaction_note_trims_whitespace() {
        let note = InteractionNote::new("  Follow up minggu depan  ").unwrap();
        assert_eq!(note.as_str(), "Follow up minggu depan");
    }

    #[test]
    fn interaction_note_rejects_too_long() {
        let long_note = "a".repeat(MAX_NOTE_LENGTH + 1);
        assert_eq!(
            InteractionNote::new(long_note),
            Err(DomainError::InteractionNoteTooLong {
                max: MAX_NOTE_LENGTH
            })
        );
    }

    #[test]
    fn new_interaction_is_linked_to_given_business_and_customer() {
        let business_id = sample_business_id();
        let customer_id = CustomerId::new();
        let interaction =
            Interaction::new(business_id, customer_id, sample_type(), None, Utc::now());

        assert_eq!(interaction.business_id(), business_id);
        assert_eq!(interaction.customer_id(), customer_id);
        assert_eq!(interaction.version(), 0);
        assert!(!interaction.is_deleted());
        assert!(interaction.note().is_none());
    }

    #[test]
    fn new_interaction_can_have_a_note() {
        let note = InteractionNote::new("Follow up minggu depan").unwrap();
        let interaction = Interaction::new(
            sample_business_id(),
            CustomerId::new(),
            sample_type(),
            Some(note.clone()),
            Utc::now(),
        );

        assert_eq!(interaction.note(), Some(&note));
    }

    #[test]
    fn with_id_uses_the_given_id() {
        let id = InteractionId::new();
        let interaction = Interaction::with_id(
            id,
            sample_business_id(),
            CustomerId::new(),
            sample_type(),
            None,
            Utc::now(),
        );
        assert_eq!(interaction.id(), id);
        assert_eq!(interaction.version(), 0);
    }

    #[test]
    fn soft_delete_marks_deleted_and_increments_version() {
        let mut interaction = Interaction::new(
            sample_business_id(),
            CustomerId::new(),
            sample_type(),
            None,
            Utc::now(),
        );
        interaction.soft_delete();
        assert!(interaction.is_deleted());
        assert_eq!(interaction.version(), 1);
    }

    #[test]
    fn from_persisted_reconstructs_exact_state() {
        let id = InteractionId::new();
        let business_id = sample_business_id();
        let customer_id = CustomerId::new();
        let interaction_type = sample_type();
        let occurred_at = Utc::now();

        let interaction = Interaction::from_persisted(PersistedInteraction {
            id,
            business_id,
            customer_id,
            interaction_type: interaction_type.clone(),
            note: None,
            occurred_at,
            created_at: occurred_at,
            updated_at: occurred_at,
            deleted_at: None,
            version: 4,
        });

        assert_eq!(interaction.id(), id);
        assert_eq!(interaction.business_id(), business_id);
        assert_eq!(interaction.customer_id(), customer_id);
        assert_eq!(interaction.interaction_type(), &interaction_type);
        assert_eq!(interaction.version(), 4);
    }
}
