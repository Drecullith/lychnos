use std::{
    collections::BTreeMap,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use lychnos_core::{
    action::ActionId,
    event::EventId,
    memory::{
        DeviceId, MEMORY_SCHEMA_VERSION, MemoryConfidence, MemoryContent, MemoryId, MemoryKind,
        MemoryProvenance, MemoryRecord, MemoryScope, MemorySensitivity, MemoryStore,
        MemorySyncMetadata, MemoryTimestamp, MemoryValue,
    },
};
use serde::{Deserialize, Serialize};

const MEMORY_FILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct JsonFileMemoryStore {
    path: PathBuf,
    records: BTreeMap<MemoryId, MemoryRecord>,
}

impl JsonFileMemoryStore {
    pub fn open_default() -> Result<Self, String> {
        Self::open(default_memory_path())
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();

        if !path.exists() {
            return Ok(Self {
                path,
                records: BTreeMap::new(),
            });
        }

        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let snapshot: PersistedMemorySnapshot = serde_json::from_slice(&bytes)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;

        if snapshot.schema_version != MEMORY_FILE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported memory file schema {} in {}; supported schema is {}",
                snapshot.schema_version,
                path.display(),
                MEMORY_FILE_SCHEMA_VERSION
            ));
        }

        let mut records = BTreeMap::new();
        for persisted in snapshot.records {
            let record = persisted.into_record()?;
            if records.insert(record.id.clone(), record).is_some() {
                return Err(format!(
                    "duplicate memory id found while loading {}",
                    path.display()
                ));
            }
        }

        Ok(Self { path, records })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    fn persist(&self) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| format!("memory path has no parent: {}", self.path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

        let snapshot = PersistedMemorySnapshot {
            schema_version: MEMORY_FILE_SCHEMA_VERSION,
            records: self
                .records
                .values()
                .map(PersistedMemoryRecord::from_record)
                .collect(),
        };
        let payload = serde_json::to_vec_pretty(&snapshot)
            .map_err(|error| format!("failed to serialize memory snapshot: {error}"))?;

        let temporary = self
            .path
            .with_extension(format!("tmp-{}", std::process::id()));

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|error| {
                    format!(
                        "failed to restrict permissions on {}: {error}",
                        temporary.display()
                    )
                })?;
        }

        file.write_all(&payload)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        file.write_all(b"\n")
            .map_err(|error| format!("failed to finish {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;
        drop(file);

        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "failed to publish memory snapshot {}: {error}",
                self.path.display()
            ));
        }

        Ok(())
    }
}

impl MemoryStore for JsonFileMemoryStore {
    type Error = String;

    fn upsert(&mut self, record: MemoryRecord) -> Result<(), Self::Error> {
        if record.schema_version != MEMORY_SCHEMA_VERSION {
            return Err(format!(
                "refusing memory {} with unsupported record schema {}",
                record.id.as_str(),
                record.schema_version
            ));
        }

        let id = record.id.clone();
        let previous = self.records.insert(id.clone(), record);

        if let Err(error) = self.persist() {
            match previous {
                Some(previous) => {
                    self.records.insert(id, previous);
                }
                None => {
                    self.records.remove(&id);
                }
            }
            return Err(error);
        }

        Ok(())
    }

    fn get(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, Self::Error> {
        Ok(self.records.get(id).cloned())
    }

    fn list(&self) -> Result<Vec<MemoryRecord>, Self::Error> {
        Ok(self.records.values().cloned().collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedMemorySnapshot {
    schema_version: u32,
    records: Vec<PersistedMemoryRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedMemoryRecord {
    id: String,
    schema_version: u32,
    kind: String,
    content: BTreeMap<String, PersistedMemoryValue>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    provenance: PersistedMemoryProvenance,
    device_id: Option<String>,
    scope: String,
    sensitivity: PersistedMemorySensitivity,
    confidence_percent: Option<u8>,
    sync_revision: u64,
    tombstone: bool,
}

impl PersistedMemoryRecord {
    fn from_record(record: &MemoryRecord) -> Self {
        let content = record
            .content
            .iter()
            .map(|(key, value)| (key.clone(), PersistedMemoryValue::from(value)))
            .collect();

        Self {
            id: record.id.as_str().to_string(),
            schema_version: record.schema_version,
            kind: record.kind.as_str().to_string(),
            content,
            created_at_unix_ms: record.created_at.as_unix_millis(),
            updated_at_unix_ms: record.updated_at.as_unix_millis(),
            provenance: PersistedMemoryProvenance {
                source: record.provenance.source.clone(),
                event_id: record
                    .provenance
                    .event_id
                    .as_ref()
                    .map(|id| id.as_str().to_string()),
                action_id: record
                    .provenance
                    .action_id
                    .as_ref()
                    .map(|id| id.as_str().to_string()),
            },
            device_id: record.device_id.as_ref().map(|id| id.as_str().to_string()),
            scope: record.scope.as_str().to_string(),
            sensitivity: PersistedMemorySensitivity::from(record.sensitivity),
            confidence_percent: record.confidence.map(MemoryConfidence::percent),
            sync_revision: record.sync.revision,
            tombstone: record.sync.tombstone,
        }
    }

    fn into_record(self) -> Result<MemoryRecord, String> {
        if self.schema_version != MEMORY_SCHEMA_VERSION {
            return Err(format!(
                "memory {} uses unsupported record schema {}; supported schema is {}",
                self.id, self.schema_version, MEMORY_SCHEMA_VERSION
            ));
        }

        let mut provenance = MemoryProvenance::new(self.provenance.source);
        if let Some(event_id) = self.provenance.event_id {
            provenance = provenance.with_event(EventId::new(event_id));
        }
        if let Some(action_id) = self.provenance.action_id {
            provenance = provenance.with_action(ActionId::new(action_id));
        }

        let mut content = MemoryContent::new();
        for (key, value) in self.content {
            content = content.with_field(key, value.into_memory_value());
        }

        let created_at = MemoryTimestamp::from_unix_millis(self.created_at_unix_ms);
        let mut record = MemoryRecord::new(
            MemoryId::new(self.id),
            MemoryKind::new(self.kind),
            created_at,
            provenance,
            MemoryScope::new(self.scope),
            self.sensitivity.into_memory_sensitivity(),
        )
        .with_content(content)
        .with_updated_at(MemoryTimestamp::from_unix_millis(self.updated_at_unix_ms))
        .with_sync_metadata(MemorySyncMetadata {
            revision: self.sync_revision,
            tombstone: self.tombstone,
        });

        if let Some(device_id) = self.device_id {
            record = record.with_device(DeviceId::new(device_id));
        }

        if let Some(percent) = self.confidence_percent {
            let confidence = MemoryConfidence::from_percent(percent).ok_or_else(|| {
                format!(
                    "memory {} contains invalid confidence percentage {percent}",
                    record.id.as_str()
                )
            })?;
            record = record.with_confidence(confidence);
        }

        Ok(record)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum PersistedMemoryValue {
    Text(String),
    Integer(i64),
    Unsigned(u64),
    Boolean(bool),
}

impl From<&MemoryValue> for PersistedMemoryValue {
    fn from(value: &MemoryValue) -> Self {
        match value {
            MemoryValue::Text(value) => Self::Text(value.clone()),
            MemoryValue::Integer(value) => Self::Integer(*value),
            MemoryValue::Unsigned(value) => Self::Unsigned(*value),
            MemoryValue::Boolean(value) => Self::Boolean(*value),
        }
    }
}

impl PersistedMemoryValue {
    fn into_memory_value(self) -> MemoryValue {
        match self {
            Self::Text(value) => MemoryValue::Text(value),
            Self::Integer(value) => MemoryValue::Integer(value),
            Self::Unsigned(value) => MemoryValue::Unsigned(value),
            Self::Boolean(value) => MemoryValue::Boolean(value),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedMemoryProvenance {
    source: String,
    event_id: Option<String>,
    action_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedMemorySensitivity {
    Standard,
    Sensitive,
    Restricted,
}

impl From<MemorySensitivity> for PersistedMemorySensitivity {
    fn from(value: MemorySensitivity) -> Self {
        match value {
            MemorySensitivity::Standard => Self::Standard,
            MemorySensitivity::Sensitive => Self::Sensitive,
            MemorySensitivity::Restricted => Self::Restricted,
        }
    }
}

impl PersistedMemorySensitivity {
    fn into_memory_sensitivity(self) -> MemorySensitivity {
        match self {
            Self::Standard => MemorySensitivity::Standard,
            Self::Sensitive => MemorySensitivity::Sensitive,
            Self::Restricted => MemorySensitivity::Restricted,
        }
    }
}

fn default_memory_path() -> PathBuf {
    resolve_memory_path(
        env::var_os("LYCHNOS_MEMORY_PATH"),
        env::var_os("XDG_DATA_HOME"),
        env::var_os("HOME"),
    )
}

fn resolve_memory_path(
    explicit_path: Option<std::ffi::OsString>,
    data_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(path) = explicit_path {
        return PathBuf::from(path);
    }

    data_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(env::temp_dir)
        .join("lychnos/memory-v1.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "lychnos-memory-test-{}-{}-{name}.json",
            std::process::id(),
            unique_nanos()
        ))
    }

    fn memory(id: &str, value: &str) -> MemoryRecord {
        MemoryRecord::new(
            MemoryId::new(id),
            MemoryKind::new("preference"),
            MemoryTimestamp::from_unix_millis(1_800_000_000_000),
            MemoryProvenance::new("test").with_event(EventId::new("event-1")),
            MemoryScope::new("identity"),
            MemorySensitivity::Standard,
        )
        .with_content(
            MemoryContent::new()
                .with_field("value", MemoryValue::Text(value.to_string()))
                .with_field("enabled", MemoryValue::Boolean(true)),
        )
        .with_device(DeviceId::new("device-1"))
        .with_confidence(MemoryConfidence::from_percent(95).expect("95 percent should be valid"))
        .with_sync_metadata(MemorySyncMetadata {
            revision: 3,
            tombstone: false,
        })
    }

    #[test]
    fn explicit_memory_path_overrides_data_home_and_home() {
        let resolved = resolve_memory_path(
            Some("/tmp/explicit-memory.json".into()),
            Some("/tmp/data-home".into()),
            Some("/tmp/home".into()),
        );

        assert_eq!(resolved, PathBuf::from("/tmp/explicit-memory.json"));
    }

    #[test]
    fn file_store_survives_reopen() {
        let path = test_path("reopen");

        {
            let mut store = JsonFileMemoryStore::open(&path).expect("store should open");
            store
                .upsert(memory("memory-1", "dark-mode"))
                .expect("memory should persist");
            assert_eq!(store.len(), 1);
        }

        let reopened = JsonFileMemoryStore::open(&path).expect("store should reopen");
        let record = reopened
            .get(&MemoryId::new("memory-1"))
            .expect("lookup should succeed")
            .expect("memory should exist after reopen");

        assert_eq!(
            record.content.get("value"),
            Some(&MemoryValue::Text("dark-mode".into()))
        );
        assert_eq!(record.sync.revision, 3);
        assert_eq!(
            record.device_id.as_ref().map(DeviceId::as_str),
            Some("device-1")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn unsupported_snapshot_schema_fails_closed() {
        let path = test_path("schema");
        fs::write(&path, r#"{"schema_version":999,"records":[]}"#).expect("fixture should write");

        let error = JsonFileMemoryStore::open(&path).expect_err("schema should fail closed");
        assert!(error.contains("unsupported memory file schema"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_confidence_fails_closed() {
        let path = test_path("confidence");
        let fixture = r#"{
          "schema_version": 1,
          "records": [{
            "id": "memory-1",
            "schema_version": 1,
            "kind": "preference",
            "content": {},
            "created_at_unix_ms": 1,
            "updated_at_unix_ms": 1,
            "provenance": {"source":"test","event_id":null,"action_id":null},
            "device_id": null,
            "scope": "identity",
            "sensitivity": "standard",
            "confidence_percent": 101,
            "sync_revision": 0,
            "tombstone": false
          }]
        }"#;
        fs::write(&path, fixture).expect("fixture should write");

        let error = JsonFileMemoryStore::open(&path).expect_err("confidence should fail closed");
        assert!(error.contains("invalid confidence"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn upsert_replaces_persisted_record() {
        let path = test_path("replace");
        let mut store = JsonFileMemoryStore::open(&path).expect("store should open");

        store
            .upsert(memory("memory-1", "first"))
            .expect("first memory should persist");
        store
            .upsert(memory("memory-1", "second"))
            .expect("replacement should persist");

        let reopened = JsonFileMemoryStore::open(&path).expect("store should reopen");
        let record = reopened
            .get(&MemoryId::new("memory-1"))
            .expect("lookup should succeed")
            .expect("memory should exist");

        assert_eq!(
            record.content.get("value"),
            Some(&MemoryValue::Text("second".into()))
        );
        assert_eq!(reopened.len(), 1);

        let _ = fs::remove_file(path);
    }

    fn unique_nanos() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};

        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos()
    }
}
