//! Model-independent Lychnos memory interfaces.

use std::{collections::BTreeMap, convert::Infallible};

use crate::{action::ActionId, event::EventId};

/// Current foundation-phase memory schema version.
pub const MEMORY_SCHEMA_VERSION: u32 = 1;

/// Opaque identifier for one memory record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryId(String);

impl MemoryId {
    /// Creates a memory identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryTimestamp(u64);

impl MemoryTimestamp {
    /// Creates a memory timestamp from Unix epoch milliseconds.
    #[must_use]
    pub const fn from_unix_millis(value: u64) -> Self {
        Self(value)
    }

    /// Returns the timestamp as Unix epoch milliseconds.
    #[must_use]
    pub const fn as_unix_millis(self) -> u64 {
        self.0
    }
}

/// Opaque category describing what a memory represents.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryKind(String);

impl MemoryKind {
    /// Creates a memory kind.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the memory kind.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque scope controlling where a memory logically belongs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryScope(String);

impl MemoryScope {
    /// Creates a memory scope.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the scope name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque identifier for a Lychnos device.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a device identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the device identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Privacy sensitivity of persisted Lychnos memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MemorySensitivity {
    /// Ordinary memory with standard local protections.
    #[default]
    Standard,

    /// Memory deserving additional privacy handling.
    Sensitive,

    /// Memory requiring the strongest available local handling policy.
    Restricted,
}

/// Confidence percentage associated with an inferred memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryConfidence(u8);

impl MemoryConfidence {
    /// Creates a confidence value from a percentage in the range 0..=100.
    #[must_use]
    pub const fn from_percent(percent: u8) -> Option<Self> {
        if percent <= 100 {
            Some(Self(percent))
        } else {
            None
        }
    }

    /// Returns the confidence percentage.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

/// Scalar value stored in structured memory content.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryValue {
    Text(String),
    Integer(i64),
    Unsigned(u64),
    Boolean(bool),
}

/// Structured content owned by a memory record.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MemoryContent {
    fields: BTreeMap<String, MemoryValue>,
}

impl MemoryContent {
    /// Creates empty memory content.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Adds a structured field.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: MemoryValue) -> Self {
        self.fields.insert(key.into(), value);
        self
    }

    /// Returns one field by name.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&MemoryValue> {
        self.fields.get(key)
    }

    /// Returns the number of fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether the content contains no fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// Describes where a memory came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProvenance {
    pub source: String,
    pub event_id: Option<EventId>,
    pub action_id: Option<ActionId>,
}

impl MemoryProvenance {
    /// Creates provenance with a caller-provided source name.
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            event_id: None,
            action_id: None,
        }
    }

    /// Associates provenance with a Lychnos event.
    #[must_use]
    pub fn with_event(mut self, event_id: EventId) -> Self {
        self.event_id = Some(event_id);
        self
    }

    /// Associates provenance with a Lychnos action.
    #[must_use]
    pub fn with_action(mut self, action_id: ActionId) -> Self {
        self.action_id = Some(action_id);
        self
    }
}

/// Metadata reserved for future multi-device synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemorySyncMetadata {
    pub revision: u64,
    pub tombstone: bool,
}

/// One Lychnos-owned memory record.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRecord {
    pub id: MemoryId,
    pub schema_version: u32,
    pub kind: MemoryKind,
    pub content: MemoryContent,
    pub created_at: MemoryTimestamp,
    pub updated_at: MemoryTimestamp,
    pub provenance: MemoryProvenance,
    pub device_id: Option<DeviceId>,
    pub scope: MemoryScope,
    pub sensitivity: MemorySensitivity,
    pub confidence: Option<MemoryConfidence>,
    pub sync: MemorySyncMetadata,
}

impl MemoryRecord {
    /// Creates a new Lychnos memory record using the current schema version.
    #[must_use]
    pub fn new(
        id: MemoryId,
        kind: MemoryKind,
        created_at: MemoryTimestamp,
        provenance: MemoryProvenance,
        scope: MemoryScope,
        sensitivity: MemorySensitivity,
    ) -> Self {
        Self {
            id,
            schema_version: MEMORY_SCHEMA_VERSION,
            kind,
            content: MemoryContent::new(),
            created_at,
            updated_at: created_at,
            provenance,
            device_id: None,
            scope,
            sensitivity,
            confidence: None,
            sync: MemorySyncMetadata::default(),
        }
    }

    /// Sets structured memory content.
    #[must_use]
    pub fn with_content(mut self, content: MemoryContent) -> Self {
        self.content = content;
        self
    }

    /// Associates the memory with an originating Lychnos device.
    #[must_use]
    pub fn with_device(mut self, device_id: DeviceId) -> Self {
        self.device_id = Some(device_id);
        self
    }

    /// Adds an optional confidence value.
    #[must_use]
    pub fn with_confidence(mut self, confidence: MemoryConfidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Updates synchronization metadata.
    #[must_use]
    pub fn with_sync_metadata(mut self, sync: MemorySyncMetadata) -> Self {
        self.sync = sync;
        self
    }

    /// Updates the modification timestamp.
    #[must_use]
    pub const fn with_updated_at(mut self, updated_at: MemoryTimestamp) -> Self {
        self.updated_at = updated_at;
        self
    }
}

/// Persistence boundary for Lychnos-owned memory.
pub trait MemoryStore {
    type Error;

    /// Inserts a new record or replaces the record with the same identifier.
    fn upsert(&mut self, record: MemoryRecord) -> Result<(), Self::Error>;

    /// Returns one record by identifier.
    fn get(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, Self::Error>;

    /// Returns all currently stored records.
    fn list(&self) -> Result<Vec<MemoryRecord>, Self::Error>;
}

/// Machine-independent memory store used during the foundation phase.
#[derive(Debug, Default)]
pub struct InMemoryMemoryStore {
    records: BTreeMap<MemoryId, MemoryRecord>,
}

impl InMemoryMemoryStore {
    /// Creates an empty memory store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Returns the number of stored records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns whether the store contains no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl MemoryStore for InMemoryMemoryStore {
    type Error = Infallible;

    fn upsert(&mut self, record: MemoryRecord) -> Result<(), Self::Error> {
        self.records.insert(record.id.clone(), record);
        Ok(())
    }

    fn get(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, Self::Error> {
        Ok(self.records.get(id).cloned())
    }

    fn list(&self) -> Result<Vec<MemoryRecord>, Self::Error> {
        Ok(self.records.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(id: &str, value: &str) -> MemoryRecord {
        MemoryRecord::new(
            MemoryId::new(id),
            MemoryKind::new("preference"),
            MemoryTimestamp::from_unix_millis(1_800_000_000_000),
            MemoryProvenance::new("test").with_event(EventId::new("event-001")),
            MemoryScope::new("identity"),
            MemorySensitivity::Standard,
        )
        .with_content(MemoryContent::new().with_field("value", MemoryValue::Text(value.into())))
        .with_device(DeviceId::new("desktop-001"))
        .with_confidence(
            MemoryConfidence::from_percent(90).expect("90 percent is valid confidence"),
        )
    }

    #[test]
    fn memory_record_preserves_lychnos_owned_metadata() {
        let record = memory("memory-001", "dark-mode");

        assert_eq!(record.id.as_str(), "memory-001");
        assert_eq!(record.schema_version, MEMORY_SCHEMA_VERSION);
        assert_eq!(record.kind.as_str(), "preference");
        assert_eq!(record.scope.as_str(), "identity");
        assert_eq!(
            record.device_id.as_ref().map(DeviceId::as_str),
            Some("desktop-001")
        );
        assert_eq!(record.confidence.map(MemoryConfidence::percent), Some(90));
        assert_eq!(
            record.content.get("value"),
            Some(&MemoryValue::Text("dark-mode".into()))
        );
        assert_eq!(
            record.provenance.event_id.as_ref().map(EventId::as_str),
            Some("event-001")
        );
    }

    #[test]
    fn confidence_rejects_values_above_one_hundred() {
        assert_eq!(
            MemoryConfidence::from_percent(100).map(MemoryConfidence::percent),
            Some(100)
        );
        assert_eq!(MemoryConfidence::from_percent(101), None);
    }

    #[test]
    fn in_memory_store_round_trips_records() {
        let mut store = InMemoryMemoryStore::new();
        let record = memory("memory-001", "dark-mode");

        store
            .upsert(record.clone())
            .expect("in-memory upsert cannot fail");

        assert_eq!(
            store
                .get(&MemoryId::new("memory-001"))
                .expect("in-memory read cannot fail"),
            Some(record)
        );

        assert_eq!(store.len(), 1);
    }

    #[test]
    fn upsert_replaces_existing_memory_with_same_id() {
        let mut store = InMemoryMemoryStore::new();

        store
            .upsert(memory("memory-001", "first"))
            .expect("in-memory upsert cannot fail");

        store
            .upsert(memory("memory-001", "second"))
            .expect("in-memory upsert cannot fail");

        let stored = store
            .get(&MemoryId::new("memory-001"))
            .expect("in-memory read cannot fail")
            .expect("record should exist");

        assert_eq!(
            stored.content.get("value"),
            Some(&MemoryValue::Text("second".into()))
        );

        assert_eq!(store.len(), 1);
    }

    #[test]
    fn sync_tombstone_metadata_survives_storage() {
        let mut store = InMemoryMemoryStore::new();

        let record = memory("memory-001", "obsolete")
            .with_updated_at(MemoryTimestamp::from_unix_millis(1_800_000_000_100))
            .with_sync_metadata(MemorySyncMetadata {
                revision: 2,
                tombstone: true,
            });

        store.upsert(record).expect("in-memory upsert cannot fail");

        let stored = store
            .get(&MemoryId::new("memory-001"))
            .expect("in-memory read cannot fail")
            .expect("record should exist");

        assert_eq!(stored.sync.revision, 2);
        assert!(stored.sync.tombstone);
    }

    #[test]
    fn new_memory_store_is_empty() {
        let store = InMemoryMemoryStore::new();

        assert!(store.is_empty());
        assert_eq!(
            store.list().expect("in-memory list cannot fail"),
            Vec::<MemoryRecord>::new()
        );
    }
}
