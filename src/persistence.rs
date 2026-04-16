use crate::{
    api::{LibraryConfigPayload, SourceRootRulesPayload},
    model::{SourceRecord, VisualUnitRecord},
    STATE_SNAPSHOT_ROW_ID,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path as FsPath,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DurableAppStateSnapshot {
    pub(crate) version: u32,
    pub(crate) library_order: Vec<String>,
    pub(crate) libraries: BTreeMap<String, DurableLibraryRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DurableLibraryRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) config: LibraryConfigPayload,
    pub(crate) source_roots: BTreeMap<String, DurableSourceRootRecord>,
    pub(crate) source_root_order: Vec<String>,
    pub(crate) sources: BTreeMap<String, SourceRecord>,
    pub(crate) source_order: Vec<String>,
    pub(crate) visual_units: BTreeMap<String, VisualUnitRecord>,
    pub(crate) visual_unit_order: Vec<String>,
    pub(crate) active_index_lines: BTreeSet<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DurableSourceRootRecord {
    pub(crate) id: String,
    pub(crate) root_path: String,
    pub(crate) enabled: bool,
    pub(crate) rules: SourceRootRulesPayload,
}

pub(crate) fn load_durable_state_snapshot(
    path: &FsPath,
) -> Result<Option<DurableAppStateSnapshot>, io::Error> {
    if !path.exists() {
        return Ok(None);
    }

    let connection = Connection::open(path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to open durable state store {}: {error}",
                path.display()
            ),
        )
    })?;
    initialize_durable_state_store(&connection).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to initialize durable state store {}: {error}",
                path.display()
            ),
        )
    })?;

    let payload = connection
        .query_row(
            "SELECT payload_json FROM durable_state_snapshots WHERE id = ?1",
            params![STATE_SNAPSHOT_ROW_ID],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to read durable state snapshot {}: {error}",
                    path.display()
                ),
            )
        })?;

    payload
        .map(|payload| {
            serde_json::from_str::<DurableAppStateSnapshot>(&payload).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Failed to decode durable state snapshot {}: {error}",
                        path.display()
                    ),
                )
            })
        })
        .transpose()
}

pub(crate) fn write_durable_state_snapshot(
    path: &FsPath,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create durable state store directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let mut connection = Connection::open(path).map_err(|error| {
        format!(
            "Failed to open durable state store {}: {error}",
            path.display()
        )
    })?;
    initialize_durable_state_store(&connection).map_err(|error| {
        format!(
            "Failed to initialize durable state store {}: {error}",
            path.display()
        )
    })?;

    let payload = serde_json::to_string(snapshot)
        .map_err(|error| format!("Failed to encode durable state snapshot: {error}"))?;
    let updated_at_ms = i64::try_from(current_unix_ms()).unwrap_or(i64::MAX);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin durable state transaction: {error}"))?;
    transaction
        .execute("DELETE FROM durable_state_snapshots", [])
        .map_err(|error| format!("Failed to clear durable state snapshot: {error}"))?;
    transaction
        .execute(
            "INSERT INTO durable_state_snapshots (id, payload_json, updated_at_ms) VALUES (?1, ?2, ?3)",
            params![STATE_SNAPSHOT_ROW_ID, payload, updated_at_ms],
        )
        .map_err(|error| format!("Failed to write durable state snapshot: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit durable state snapshot: {error}"))?;
    Ok(())
}

pub(crate) fn initialize_durable_state_store(
    connection: &Connection,
) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS durable_state_snapshots (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            payload_json TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
