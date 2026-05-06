use crate::{
    api::SourceRootRulesPayload,
    model::{
        AssetRecord, ContentE2eIndexStateRecord, ContentRecord, QueryHistoryRecord,
        SourceAssetLocationRecord, SourceRecord, TempQueryAssetRecord, UnitIndexRecord, UnitRecord,
        VectorSpaceRecord,
    },
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs, io,
    path::Path as FsPath,
    time::{SystemTime, UNIX_EPOCH},
};

const STRUCTURED_STATE_SCHEMA_VERSION: i64 = 7;

#[derive(Debug)]
pub(crate) struct LoadedDurableStateSnapshot {
    pub(crate) snapshot: DurableAppStateSnapshot,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DurableAppStateSnapshot {
    pub(crate) version: u32,
    pub(crate) library_order: Vec<String>,
    pub(crate) libraries: BTreeMap<String, DurableLibraryRecord>,
    pub(crate) query_assets: BTreeMap<String, TempQueryAssetRecord>,
    pub(crate) query_history: BTreeMap<String, QueryHistoryRecord>,
    pub(crate) query_history_order: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DurableLibraryRecord {
    pub(crate) id: String,
    pub(crate) display_name: String,
    #[serde(default = "default_library_lifecycle_state")]
    pub(crate) lifecycle_state: String,
    #[serde(default)]
    pub(crate) archived_at_ms: Option<u128>,
    pub(crate) source_roots: BTreeMap<String, DurableSourceRootRecord>,
    pub(crate) source_root_order: Vec<String>,
    pub(crate) contents: BTreeMap<String, ContentRecord>,
    pub(crate) sources: BTreeMap<String, SourceRecord>,
    pub(crate) source_order: Vec<String>,
    pub(crate) source_asset_locations: BTreeMap<String, SourceAssetLocationRecord>,
    pub(crate) source_asset_location_order: Vec<String>,
    pub(crate) assets: BTreeMap<String, AssetRecord>,
    pub(crate) asset_order: Vec<String>,
    pub(crate) units: BTreeMap<String, UnitRecord>,
    pub(crate) unit_order: Vec<String>,
    pub(crate) vector_spaces: BTreeMap<String, VectorSpaceRecord>,
    pub(crate) unit_indexes: BTreeMap<String, UnitIndexRecord>,
    pub(crate) content_e2e_index_states: BTreeMap<String, ContentE2eIndexStateRecord>,
}

fn default_library_lifecycle_state() -> String {
    "active".to_string()
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
) -> Result<Option<LoadedDurableStateSnapshot>, io::Error> {
    if !path.exists() {
        return Ok(None);
    }

    let connection = open_connection(path)?;
    reject_legacy_snapshot_store(path, &connection)?;
    initialize_durable_state_store(&connection).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to initialize durable state store {}: {error}",
                path.display()
            ),
        )
    })?;

    let Some(schema_version) = read_schema_version(path, &connection)? else {
        return Ok(None);
    };
    if schema_version != STRUCTURED_STATE_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unsupported durable state schema version {schema_version} in {}; expected {STRUCTURED_STATE_SCHEMA_VERSION}. Reset or cut over the runtime before starting this version.",
                path.display()
            ),
        ));
    }

    load_structured_state_snapshot(path, &connection).map(Some)
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
    reject_legacy_snapshot_store(path, &connection).map_err(|error| error.to_string())?;
    initialize_durable_state_store(&connection).map_err(|error| {
        format!(
            "Failed to initialize durable state store {}: {error}",
            path.display()
        )
    })?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin durable state transaction: {error}"))?;
    write_structured_state_snapshot(&transaction, snapshot)?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit durable state snapshot: {error}"))?;
    Ok(())
}

pub(crate) fn initialize_durable_state_store(
    connection: &Connection,
) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS state_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            schema_version INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS libraries (
            library_id TEXT PRIMARY KEY,
            position INTEGER NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            lifecycle_state TEXT NOT NULL,
            archived_at_ms INTEGER NULL
        );
        CREATE TABLE IF NOT EXISTS source_roots (
            library_id TEXT NOT NULL,
            source_root_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            root_path TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            rules_json TEXT NOT NULL,
            PRIMARY KEY (library_id, source_root_id),
            UNIQUE (library_id, position)
        );
        CREATE TABLE IF NOT EXISTS sources (
            library_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            source_content_id TEXT NOT NULL,
            source_root_id TEXT NULL,
            source_root_path TEXT NULL,
            source_uri TEXT NOT NULL,
            relative_path TEXT NULL,
            source_type TEXT NOT NULL,
            media_type TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            status_reason TEXT NULL,
            page_count INTEGER NULL,
            duration_ms INTEGER NULL,
            observed_size_bytes INTEGER NULL,
            observed_modified_at_ms INTEGER NULL,
            PRIMARY KEY (library_id, source_id),
            UNIQUE (library_id, position)
        );
        CREATE TABLE IF NOT EXISTS contents (
            content_id TEXT NOT NULL,
            size_bytes INTEGER NULL,
            fast_fingerprint TEXT NULL,
            sha256 TEXT NULL,
            created_at_ms INTEGER NOT NULL,
            PRIMARY KEY (content_id)
        );
        CREATE TABLE IF NOT EXISTS source_asset_locations (
            library_id TEXT NOT NULL,
            location_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            source_id TEXT NOT NULL,
            asset_id TEXT NOT NULL,
            locator_json TEXT NOT NULL,
            visibility TEXT NOT NULL,
            PRIMARY KEY (library_id, location_id),
            UNIQUE (library_id, position),
            UNIQUE (library_id, source_id, asset_id)
        );
        CREATE TABLE IF NOT EXISTS assets (
            asset_id TEXT NOT NULL,
            position INTEGER NOT NULL UNIQUE,
            source_content_id TEXT NOT NULL,
            asset_type TEXT NOT NULL,
            locator_json TEXT NOT NULL,
            derivation_signature TEXT NOT NULL,
            neighbor_context_json TEXT NOT NULL,
            PRIMARY KEY (asset_id)
        );
        CREATE TABLE IF NOT EXISTS units (
            unit_id TEXT NOT NULL,
            position INTEGER NOT NULL UNIQUE,
            asset_id TEXT NOT NULL,
            asset_position INTEGER NOT NULL,
            point_id INTEGER NOT NULL,
            unit_type TEXT NOT NULL,
            derivation_signature TEXT NOT NULL,
            locator_json TEXT NOT NULL,
            neighbor_context_json TEXT NOT NULL,
            PRIMARY KEY (unit_id),
            UNIQUE (asset_id, asset_position)
        );
        CREATE TABLE IF NOT EXISTS vector_spaces (
            vector_space_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            model_version TEXT NOT NULL,
            model_revision TEXT NULL,
            vector_type TEXT NOT NULL,
            PRIMARY KEY (vector_space_id)
        );
        CREATE TABLE IF NOT EXISTS unit_indexes (
            unit_id TEXT NOT NULL,
            vector_space_id TEXT NOT NULL,
            status TEXT NOT NULL,
            visibility TEXT NOT NULL,
            vector_ref_json TEXT NULL,
            job_id TEXT NULL,
            error_summary TEXT NULL,
            PRIMARY KEY (unit_id, vector_space_id)
        );
        CREATE TABLE IF NOT EXISTS content_e2e_index_states (
            content_id TEXT NOT NULL,
            pipe_signature TEXT NOT NULL,
            vector_space_id TEXT NOT NULL,
            indexed_at_ms INTEGER NOT NULL,
            PRIMARY KEY (content_id, pipe_signature, vector_space_id)
        );
        CREATE TABLE IF NOT EXISTS query_assets (
            query_asset_id TEXT PRIMARY KEY,
            owner_scope TEXT NOT NULL,
            library_id TEXT NULL,
            source_type TEXT NOT NULL,
            content_type TEXT NOT NULL,
            path TEXT NOT NULL,
            original_filename TEXT NULL,
            page_count INTEGER NULL,
            duration_ms INTEGER NULL,
            size_bytes INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL,
            expires_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS query_history (
            query_id TEXT PRIMARY KEY,
            position INTEGER NOT NULL UNIQUE,
            created_at_ms INTEGER NOT NULL,
            source TEXT NOT NULL,
            query_kind TEXT NOT NULL,
            input_kind TEXT NOT NULL,
            input_summary TEXT NOT NULL,
            input_json TEXT NOT NULL,
            search_scope_json TEXT NOT NULL,
            filters_json TEXT NULL,
            target_content_types_json TEXT NULL,
            top_k INTEGER NULL,
            status TEXT NOT NULL,
            result_count INTEGER NULL,
            error_code TEXT NULL,
            error_message TEXT NULL,
            duration_ms INTEGER NOT NULL
        );
        ",
    )?;
    Ok(())
}

fn open_connection(path: &FsPath) -> Result<Connection, io::Error> {
    Connection::open(path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to open durable state store {}: {error}",
                path.display()
            ),
        )
    })
}

fn reject_legacy_snapshot_store(path: &FsPath, connection: &Connection) -> Result<(), io::Error> {
    if table_exists(connection, "durable_state_snapshots").map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to inspect durable state store {}: {error}",
                path.display()
            ),
        )
    })? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unsupported legacy durable snapshot store {}; reset or cut over the runtime before starting this version.",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool, rusqlite::Error> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
}

fn read_schema_version(path: &FsPath, connection: &Connection) -> Result<Option<i64>, io::Error> {
    connection
        .query_row(
            "SELECT schema_version FROM state_meta WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to read durable state schema version {}: {error}",
                    path.display()
                ),
            )
        })
}

fn load_structured_state_snapshot(
    path: &FsPath,
    connection: &Connection,
) -> Result<LoadedDurableStateSnapshot, io::Error> {
    let mut snapshot = DurableAppStateSnapshot {
        version: STRUCTURED_STATE_SCHEMA_VERSION as u32,
        library_order: Vec::new(),
        libraries: BTreeMap::new(),
        query_assets: BTreeMap::new(),
        query_history: BTreeMap::new(),
        query_history_order: Vec::new(),
    };

    load_libraries(path, connection, &mut snapshot)?;
    load_source_roots(path, connection, &mut snapshot)?;
    load_contents(path, connection, &mut snapshot)?;
    load_sources(path, connection, &mut snapshot)?;
    load_source_asset_locations(path, connection, &mut snapshot)?;
    load_assets(path, connection, &mut snapshot)?;
    load_units(path, connection, &mut snapshot)?;
    load_vector_spaces(path, connection, &mut snapshot)?;
    load_unit_indexes(path, connection, &mut snapshot)?;
    load_content_e2e_index_states(path, connection, &mut snapshot)?;
    load_query_assets(path, connection, &mut snapshot)?;
    load_query_history(path, connection, &mut snapshot)?;
    hydrate_location_compat_fields(&mut snapshot);

    Ok(LoadedDurableStateSnapshot { snapshot })
}

fn hydrate_location_compat_fields(snapshot: &mut DurableAppStateSnapshot) {
    for library in snapshot.libraries.values_mut() {
        let locations = library
            .source_asset_location_order
            .iter()
            .filter_map(|location_id| library.source_asset_locations.get(location_id).cloned())
            .collect::<Vec<_>>();
        for location in locations {
            let Some(source) = library.sources.get(&location.source_id).cloned() else {
                continue;
            };
            let unit_ids = if let Some(asset) = library.assets.get_mut(&location.asset_id) {
                asset.source_id = source.id.clone();
                asset.source_path = source.source_path.clone();
                asset.source_type = source.source_type.clone();
                asset.unit_ids.clone()
            } else {
                Vec::new()
            };
            for unit_id in unit_ids {
                if let Some(unit) = library.units.get_mut(&unit_id) {
                    unit.source_id = source.id.clone();
                    unit.source_path = source.source_path.clone();
                    unit.source_type = source.source_type.clone();
                    unit.asset_type = library
                        .assets
                        .get(&location.asset_id)
                        .map(|asset| asset.asset_type.clone())
                        .unwrap_or_default();
                }
            }
        }
    }
}

fn load_libraries(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, display_name, lifecycle_state, archived_at_ms
             FROM libraries
             ORDER BY position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare libraries query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query libraries", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read library row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read library_id", error))?;
        let display_name: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read library display_name", error))?;
        let lifecycle_state: String = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read library lifecycle_state", error))?;
        let archived_at_ms = optional_i64_to_u128(
            path,
            row.get(3)
                .map_err(|error| sqlite_io(path, "read library archived_at_ms", error))?,
            "libraries.archived_at_ms",
        )?;

        snapshot.library_order.push(library_id.clone());
        snapshot.libraries.insert(
            library_id.clone(),
            DurableLibraryRecord {
                id: library_id,
                display_name,
                lifecycle_state,
                archived_at_ms,
                source_roots: BTreeMap::new(),
                source_root_order: Vec::new(),
                contents: BTreeMap::new(),
                sources: BTreeMap::new(),
                source_order: Vec::new(),
                source_asset_locations: BTreeMap::new(),
                source_asset_location_order: Vec::new(),
                assets: BTreeMap::new(),
                asset_order: Vec::new(),
                units: BTreeMap::new(),
                unit_order: Vec::new(),
                vector_spaces: BTreeMap::new(),
                unit_indexes: BTreeMap::new(),
                content_e2e_index_states: BTreeMap::new(),
            },
        );
    }
    Ok(())
}

fn load_source_roots(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, source_root_id, root_path, enabled, rules_json
             FROM source_roots
             ORDER BY library_id ASC, position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare source_roots query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query source_roots", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read source_root row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read source_root library_id", error))?;
        let source_root_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read source_root_id", error))?;
        let root_path: String = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read source_root root_path", error))?;
        let enabled = i64_to_bool(
            path,
            row.get(3)
                .map_err(|error| sqlite_io(path, "read source_root enabled", error))?,
            "source_roots.enabled",
        )?;
        let rules_json: String = row
            .get(4)
            .map_err(|error| sqlite_io(path, "read source_root rules_json", error))?;
        let rules =
            parse_json::<SourceRootRulesPayload>(path, &rules_json, "source_roots.rules_json")?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("source_root {source_root_id} references missing library {library_id}"),
            )
        })?;

        library.source_root_order.push(source_root_id.clone());
        library.source_roots.insert(
            source_root_id.clone(),
            DurableSourceRootRecord {
                id: source_root_id,
                root_path,
                enabled,
                rules,
            },
        );
    }
    Ok(())
}

fn load_sources(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, source_id, source_content_id, source_root_id, source_root_path, source_uri,
                    relative_path, source_type, media_type, kind, status, status_reason, page_count,
                    duration_ms, observed_size_bytes, observed_modified_at_ms
             FROM sources
             ORDER BY library_id ASC, position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare sources query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query sources", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read source row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read source library_id", error))?;
        let source_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read source_id", error))?;
        let source_content_id: String = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read source source_content_id", error))?;
        let source_root_id: Option<String> = row
            .get(3)
            .map_err(|error| sqlite_io(path, "read source source_root_id", error))?;
        let source_root_path: Option<String> = row
            .get(4)
            .map_err(|error| sqlite_io(path, "read source source_root_path", error))?;
        let source_uri: String = row
            .get(5)
            .map_err(|error| sqlite_io(path, "read source_uri", error))?;
        let relative_path: Option<String> = row
            .get(6)
            .map_err(|error| sqlite_io(path, "read relative_path", error))?;
        let source_type: String = row
            .get(7)
            .map_err(|error| sqlite_io(path, "read source_type", error))?;
        let media_type: String = row
            .get(8)
            .map_err(|error| sqlite_io(path, "read source media_type", error))?;
        let kind: String = row
            .get(9)
            .map_err(|error| sqlite_io(path, "read source kind", error))?;
        let status: String = row
            .get(10)
            .map_err(|error| sqlite_io(path, "read source status", error))?;
        let status_reason: Option<String> = row
            .get(11)
            .map_err(|error| sqlite_io(path, "read source status_reason", error))?;
        let page_count = optional_i64_to_usize(
            path,
            row.get(12)
                .map_err(|error| sqlite_io(path, "read source page_count", error))?,
            "sources.page_count",
        )?;
        let duration_ms = optional_i64_to_u64(
            path,
            row.get(13)
                .map_err(|error| sqlite_io(path, "read source duration_ms", error))?,
            "sources.duration_ms",
        )?;
        let observed_size_bytes = optional_i64_to_u64(
            path,
            row.get(14)
                .map_err(|error| sqlite_io(path, "read source observed_size_bytes", error))?,
            "sources.observed_size_bytes",
        )?;
        let observed_modified_at_ms = optional_i64_to_u128(
            path,
            row.get(15)
                .map_err(|error| sqlite_io(path, "read source observed_modified_at_ms", error))?,
            "sources.observed_modified_at_ms",
        )?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("source {source_id} references missing library {library_id}"),
            )
        })?;

        library.source_order.push(source_id.clone());
        let source_path = source_uri
            .strip_prefix("file://")
            .unwrap_or(&source_uri)
            .to_string();
        library.sources.insert(
            source_id,
            SourceRecord {
                id: row
                    .get(1)
                    .map_err(|error| sqlite_io(path, "read source_id", error))?,
                source_root_id,
                source_root_path,
                source_path,
                source_uri,
                relative_path,
                source_type,
                media_type,
                kind,
                status,
                status_reason,
                page_count,
                duration_ms,
                observed_size_bytes,
                observed_modified_at_ms,
                source_content_id,
                asset_ids: Vec::new(),
            },
        );
    }
    Ok(())
}

fn load_contents(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT content_id, size_bytes, fast_fingerprint, sha256, created_at_ms
             FROM contents
             ORDER BY content_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare contents query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query contents", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read content row", error))?
    {
        let content_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read content_id", error))?;
        let size_bytes = optional_i64_to_u64(
            path,
            row.get(1)
                .map_err(|error| sqlite_io(path, "read content size_bytes", error))?,
            "contents.size_bytes",
        )?;
        let created_at_ms = i64_to_u128(
            path,
            row.get(4)
                .map_err(|error| sqlite_io(path, "read content created_at_ms", error))?,
            "contents.created_at_ms",
        )?;
        let record = ContentRecord {
            id: content_id.clone(),
            size_bytes,
            fast_fingerprint: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read content fast_fingerprint", error))?,
            sha256: row
                .get(3)
                .map_err(|error| sqlite_io(path, "read content sha256", error))?,
            created_at_ms,
        };
        for library in snapshot.libraries.values_mut() {
            library.contents.insert(content_id.clone(), record.clone());
        }
    }
    Ok(())
}

fn load_source_asset_locations(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, location_id, source_id, asset_id, locator_json, visibility
             FROM source_asset_locations
             ORDER BY library_id ASC, position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare source_asset_locations query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query source_asset_locations", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read source_asset_location row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read source_asset_location library_id", error))?;
        let location_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read source_asset_location id", error))?;
        let source_id: String = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read source_asset_location source_id", error))?;
        let asset_id: String = row
            .get(3)
            .map_err(|error| sqlite_io(path, "read source_asset_location asset_id", error))?;
        let locator_json: String = row
            .get(4)
            .map_err(|error| sqlite_io(path, "read source_asset_location locator_json", error))?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!(
                    "source_asset_location {location_id} references missing library {library_id}"
                ),
            )
        })?;
        if let Some(source) = library.sources.get_mut(&source_id) {
            source.asset_ids.push(asset_id.clone());
        }
        library
            .source_asset_location_order
            .push(location_id.clone());
        library.source_asset_locations.insert(
            location_id.clone(),
            SourceAssetLocationRecord {
                id: location_id,
                source_id,
                asset_id,
                locator: parse_json::<Value>(
                    path,
                    &locator_json,
                    "source_asset_locations.locator_json",
                )?,
                visibility: row.get(5).map_err(|error| {
                    sqlite_io(path, "read source_asset_location visibility", error)
                })?,
            },
        );
    }
    Ok(())
}

fn load_assets(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT asset_id, source_content_id, asset_type, locator_json,
                    derivation_signature, neighbor_context_json
             FROM assets
             ORDER BY position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare assets query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query assets", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read asset row", error))?
    {
        let asset_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read asset_id", error))?;
        let locator_json: String = row
            .get(3)
            .map_err(|error| sqlite_io(path, "read asset locator_json", error))?;
        let neighbor_context_json: String = row
            .get(5)
            .map_err(|error| sqlite_io(path, "read asset neighbor_context_json", error))?;
        let record = AssetRecord {
            id: asset_id.clone(),
            source_id: String::new(),
            content_id: String::new(),
            source_path: String::new(),
            source_type: String::new(),
            source_content_id: row
                .get(1)
                .map_err(|error| sqlite_io(path, "read asset source_content_id", error))?,
            asset_type: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read asset asset_type", error))?,
            locator: parse_json::<Value>(path, &locator_json, "assets.locator_json")?,
            derivation_signature: row
                .get(4)
                .map_err(|error| sqlite_io(path, "read asset derivation_signature", error))?,
            neighbor_context: parse_json::<Value>(
                path,
                &neighbor_context_json,
                "assets.neighbor_context_json",
            )?,
            unit_ids: Vec::new(),
        };
        for library in snapshot.libraries.values_mut() {
            library.asset_order.push(asset_id.clone());
            library.assets.insert(asset_id.clone(), record.clone());
        }
    }
    Ok(())
}

fn load_units(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut asset_unit_ids = BTreeMap::<String, Vec<(i64, String)>>::new();
    let mut statement = connection
        .prepare(
            "SELECT unit_id, asset_id, asset_position, point_id, unit_type,
                    derivation_signature, locator_json, neighbor_context_json
             FROM units
             ORDER BY position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare units query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query units", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read unit row", error))?
    {
        let unit_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read unit_id", error))?;
        let asset_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read unit asset_id", error))?;
        let asset_position: i64 = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read unit asset_position", error))?;
        let point_id = i64_to_u64(
            path,
            row.get(3)
                .map_err(|error| sqlite_io(path, "read unit point_id", error))?,
            "units.point_id",
        )?;
        let locator_json: String = row
            .get(6)
            .map_err(|error| sqlite_io(path, "read unit locator_json", error))?;
        let neighbor_context_json: String = row
            .get(7)
            .map_err(|error| sqlite_io(path, "read unit neighbor_context_json", error))?;
        let record = UnitRecord {
            id: unit_id.clone(),
            asset_id: asset_id.clone(),
            content_id: String::new(),
            point_id,
            source_id: String::new(),
            source_path: String::new(),
            source_type: String::new(),
            asset_type: String::new(),
            unit_type: row
                .get(4)
                .map_err(|error| sqlite_io(path, "read unit unit_type", error))?,
            derivation_signature: row
                .get(5)
                .map_err(|error| sqlite_io(path, "read unit derivation_signature", error))?,
            locator: parse_json::<Value>(path, &locator_json, "units.locator_json")?,
            neighbor_context: parse_json::<Value>(
                path,
                &neighbor_context_json,
                "units.neighbor_context_json",
            )?,
        };
        for library in snapshot.libraries.values_mut() {
            library.unit_order.push(unit_id.clone());
            library.units.insert(unit_id.clone(), record.clone());
        }
        asset_unit_ids
            .entry(asset_id)
            .or_default()
            .push((asset_position, unit_id));
    }
    for (asset_id, mut unit_ids) in asset_unit_ids {
        unit_ids.sort_by_key(|(position, _)| *position);
        let unit_ids = unit_ids
            .into_iter()
            .map(|(_, unit_id)| unit_id)
            .collect::<Vec<_>>();
        for library in snapshot.libraries.values_mut() {
            let asset = library.assets.get_mut(&asset_id).ok_or_else(|| {
                invalid_data(
                    path,
                    format!("missing asset {asset_id} while assigning units"),
                )
            })?;
            asset.unit_ids = unit_ids.clone();
        }
    }
    Ok(())
}

fn load_vector_spaces(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT vector_space_id, provider_id, model_id, model_version,
                    model_revision, vector_type
             FROM vector_spaces
             ORDER BY vector_space_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare vector_spaces query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query vector_spaces", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read vector_space row", error))?
    {
        let vector_space_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read vector_space_id", error))?;
        let record = VectorSpaceRecord {
            id: vector_space_id.clone(),
            provider_id: row
                .get(1)
                .map_err(|error| sqlite_io(path, "read vector_space provider_id", error))?,
            model_id: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read vector_space model_id", error))?,
            model_version: row
                .get(3)
                .map_err(|error| sqlite_io(path, "read vector_space model_version", error))?,
            model_revision: row
                .get(4)
                .map_err(|error| sqlite_io(path, "read vector_space model_revision", error))?,
            vector_type: row
                .get(5)
                .map_err(|error| sqlite_io(path, "read vector_space vector_type", error))?,
        };
        for library in snapshot.libraries.values_mut() {
            library
                .vector_spaces
                .insert(vector_space_id.clone(), record.clone());
        }
    }
    Ok(())
}

fn load_unit_indexes(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT unit_id, vector_space_id, status, visibility,
                    vector_ref_json, job_id, error_summary
             FROM unit_indexes
             ORDER BY unit_id ASC, vector_space_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare unit_indexes query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query unit_indexes", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read unit_index row", error))?
    {
        let unit_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read unit_index unit_id", error))?;
        let vector_space_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read unit_index vector_space_id", error))?;
        let vector_ref_json: Option<String> = row
            .get(4)
            .map_err(|error| sqlite_io(path, "read unit_index vector_ref_json", error))?;
        let vector_ref = vector_ref_json
            .as_deref()
            .map(|raw| parse_json::<Value>(path, raw, "unit_indexes.vector_ref_json"))
            .transpose()?;
        let record = UnitIndexRecord {
            unit_id: unit_id.clone(),
            vector_space_id: vector_space_id.clone(),
            status: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read unit_index status", error))?,
            visibility: row
                .get(3)
                .map_err(|error| sqlite_io(path, "read unit_index visibility", error))?,
            vector_ref,
            job_id: row
                .get(5)
                .map_err(|error| sqlite_io(path, "read unit_index job_id", error))?,
            error_summary: row
                .get(6)
                .map_err(|error| sqlite_io(path, "read unit_index error_summary", error))?,
        };
        let key = UnitIndexRecord::key(&unit_id, &vector_space_id);
        for library in snapshot.libraries.values_mut() {
            library.unit_indexes.insert(key.clone(), record.clone());
        }
    }
    Ok(())
}

fn load_content_e2e_index_states(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT content_id, pipe_signature, vector_space_id, indexed_at_ms
             FROM content_e2e_index_states
             ORDER BY content_id ASC, pipe_signature ASC, vector_space_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare content_e2e_index_states query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query content_e2e_index_states", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read content_e2e_index_state row", error))?
    {
        let content_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read content_e2e_index_state content_id", error))?;
        let pipe_signature: String = row.get(1).map_err(|error| {
            sqlite_io(path, "read content_e2e_index_state pipe_signature", error)
        })?;
        let vector_space_id: String = row.get(2).map_err(|error| {
            sqlite_io(path, "read content_e2e_index_state vector_space_id", error)
        })?;
        let indexed_at_ms = i64_to_u128(
            path,
            row.get(3).map_err(|error| {
                sqlite_io(path, "read content_e2e_index_state indexed_at_ms", error)
            })?,
            "content_e2e_index_states.indexed_at_ms",
        )?;
        let record = ContentE2eIndexStateRecord {
            content_id: content_id.clone(),
            pipe_signature: pipe_signature.clone(),
            vector_space_id: vector_space_id.clone(),
            indexed_at_ms,
        };
        let key = ContentE2eIndexStateRecord::key(&content_id, &pipe_signature, &vector_space_id);
        for library in snapshot.libraries.values_mut() {
            library
                .content_e2e_index_states
                .insert(key.clone(), record.clone());
        }
    }
    Ok(())
}

fn load_query_assets(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT query_asset_id, owner_scope, library_id, source_type, content_type,
                    path, original_filename, page_count, duration_ms, size_bytes,
                    created_at_ms, expires_at_ms
             FROM query_assets
             ORDER BY query_asset_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare query_assets query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query query_assets", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read query_asset row", error))?
    {
        let id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read query_asset_id", error))?;
        let size_bytes = usize::try_from(
            row.get::<_, i64>(9)
                .map_err(|error| sqlite_io(path, "read query_asset size_bytes", error))?,
        )
        .map_err(|_| invalid_data(path, "query_assets.size_bytes must be non-negative"))?;
        let record = TempQueryAssetRecord {
            id: id.clone(),
            owner_scope: row
                .get(1)
                .map_err(|error| sqlite_io(path, "read query_asset owner_scope", error))?,
            library_id: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read query_asset library_id", error))?,
            source_type: row
                .get(3)
                .map_err(|error| sqlite_io(path, "read query_asset source_type", error))?,
            content_type: row
                .get(4)
                .map_err(|error| sqlite_io(path, "read query_asset content_type", error))?,
            path: row
                .get(5)
                .map_err(|error| sqlite_io(path, "read query_asset path", error))?,
            original_filename: row
                .get(6)
                .map_err(|error| sqlite_io(path, "read query_asset original_filename", error))?,
            page_count: optional_i64_to_usize(
                path,
                row.get(7)
                    .map_err(|error| sqlite_io(path, "read query_asset page_count", error))?,
                "query_assets.page_count",
            )?,
            duration_ms: optional_i64_to_u64(
                path,
                row.get(8)
                    .map_err(|error| sqlite_io(path, "read query_asset duration_ms", error))?,
                "query_assets.duration_ms",
            )?,
            size_bytes,
            created_at_ms: i64_to_u128(
                path,
                row.get(10)
                    .map_err(|error| sqlite_io(path, "read query_asset created_at_ms", error))?,
                "query_assets.created_at_ms",
            )?,
            expires_at_ms: i64_to_u128(
                path,
                row.get(11)
                    .map_err(|error| sqlite_io(path, "read query_asset expires_at_ms", error))?,
                "query_assets.expires_at_ms",
            )?,
        };
        snapshot.query_assets.insert(id, record);
    }
    Ok(())
}

fn load_query_history(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT query_id, created_at_ms, source, query_kind, input_kind,
                    input_summary, input_json, search_scope_json, filters_json,
                    target_content_types_json, top_k, status, result_count,
                    error_code, error_message, duration_ms
             FROM query_history
             ORDER BY position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare query_history query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query query_history", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read query_history row", error))?
    {
        let id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read query_id", error))?;
        let filters_json = row
            .get::<_, Option<String>>(8)
            .map_err(|error| sqlite_io(path, "read query_history filters_json", error))?
            .map(|raw| parse_json::<Value>(path, &raw, "query_history.filters_json"))
            .transpose()?;
        let target_content_types_json = row
            .get::<_, Option<String>>(9)
            .map_err(|error| {
                sqlite_io(path, "read query_history target_content_types_json", error)
            })?
            .map(|raw| parse_json::<Value>(path, &raw, "query_history.target_content_types_json"))
            .transpose()?;
        let record = QueryHistoryRecord {
            id: id.clone(),
            created_at_ms: i64_to_u128(
                path,
                row.get(1)
                    .map_err(|error| sqlite_io(path, "read query_history created_at_ms", error))?,
                "query_history.created_at_ms",
            )?,
            source: row
                .get(2)
                .map_err(|error| sqlite_io(path, "read query_history source", error))?,
            query_kind: row
                .get(3)
                .map_err(|error| sqlite_io(path, "read query_history query_kind", error))?,
            input_kind: row
                .get(4)
                .map_err(|error| sqlite_io(path, "read query_history input_kind", error))?,
            input_summary: row
                .get(5)
                .map_err(|error| sqlite_io(path, "read query_history input_summary", error))?,
            input_json: parse_json(
                path,
                &row.get::<_, String>(6)
                    .map_err(|error| sqlite_io(path, "read query_history input_json", error))?,
                "query_history.input_json",
            )?,
            search_scope_json: parse_json(
                path,
                &row.get::<_, String>(7).map_err(|error| {
                    sqlite_io(path, "read query_history search_scope_json", error)
                })?,
                "query_history.search_scope_json",
            )?,
            filters_json,
            target_content_types_json,
            top_k: optional_i64_to_usize(
                path,
                row.get(10)
                    .map_err(|error| sqlite_io(path, "read query_history top_k", error))?,
                "query_history.top_k",
            )?,
            status: row
                .get(11)
                .map_err(|error| sqlite_io(path, "read query_history status", error))?,
            result_count: optional_i64_to_usize(
                path,
                row.get(12)
                    .map_err(|error| sqlite_io(path, "read query_history result_count", error))?,
                "query_history.result_count",
            )?,
            error_code: row
                .get(13)
                .map_err(|error| sqlite_io(path, "read query_history error_code", error))?,
            error_message: row
                .get(14)
                .map_err(|error| sqlite_io(path, "read query_history error_message", error))?,
            duration_ms: i64_to_u128(
                path,
                row.get(15)
                    .map_err(|error| sqlite_io(path, "read query_history duration_ms", error))?,
                "query_history.duration_ms",
            )?,
        };
        snapshot.query_history_order.push(id.clone());
        snapshot.query_history.insert(id, record);
    }
    Ok(())
}

fn write_structured_state_snapshot(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    clear_structured_tables(transaction)?;
    transaction
        .execute(
            "INSERT INTO state_meta (id, schema_version, updated_at_ms) VALUES (1, ?1, ?2)",
            params![
                STRUCTURED_STATE_SCHEMA_VERSION,
                u128_to_i64(current_unix_ms(), "state_meta.updated_at_ms")?
            ],
        )
        .map_err(|error| format!("Failed to write state_meta: {error}"))?;

    for (library_position, library_id) in snapshot.library_order.iter().enumerate() {
        let library = snapshot
            .libraries
            .get(library_id)
            .ok_or_else(|| format!("library_order references missing library `{library_id}`"))?;
        transaction
            .execute(
                "INSERT INTO libraries (
                    library_id, position, display_name, lifecycle_state, archived_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    library.id,
                    usize_to_i64(library_position, "libraries.position")?,
                    library.display_name,
                    library.lifecycle_state,
                    optional_u128_to_i64(library.archived_at_ms, "libraries.archived_at_ms")?,
                ],
            )
            .map_err(|error| format!("Failed to write library {}: {error}", library.id))?;

        write_source_roots(transaction, &library.id, library)?;
        write_sources(transaction, &library.id, library)?;
        write_source_asset_locations(transaction, &library.id, library)?;
    }

    write_contents(transaction, snapshot)?;
    write_assets(transaction, snapshot)?;
    write_units(transaction, snapshot)?;
    write_vector_spaces(transaction, snapshot)?;
    write_unit_indexes(transaction, snapshot)?;
    write_content_e2e_index_states(transaction, snapshot)?;
    write_query_assets(transaction, snapshot)?;
    write_query_history(transaction, snapshot)?;

    Ok(())
}

fn clear_structured_tables(transaction: &Transaction<'_>) -> Result<(), String> {
    for table in [
        "query_history",
        "query_assets",
        "content_e2e_index_states",
        "unit_indexes",
        "vector_spaces",
        "units",
        "assets",
        "source_asset_locations",
        "sources",
        "contents",
        "source_roots",
        "libraries",
        "state_meta",
    ] {
        transaction
            .execute(&format!("DELETE FROM {table}"), [])
            .map_err(|error| format!("Failed to clear {table}: {error}"))?;
    }
    Ok(())
}

fn write_source_roots(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    for (position, source_root_id) in library.source_root_order.iter().enumerate() {
        let root = library.source_roots.get(source_root_id).ok_or_else(|| {
            format!(
                "source_root_order for library `{library_id}` references missing source root `{source_root_id}`"
            )
        })?;
        let rules_json = serde_json::to_string(&root.rules)
            .map_err(|error| format!("Failed to encode source root rules: {error}"))?;
        transaction
            .execute(
                "INSERT INTO source_roots (
                    library_id, source_root_id, position, root_path, enabled, rules_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    library_id,
                    root.id,
                    usize_to_i64(position, "source_roots.position")?,
                    root.root_path,
                    bool_to_i64(root.enabled),
                    rules_json,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write source root {} for library {library_id}: {error}",
                    root.id
                )
            })?;
    }
    Ok(())
}

fn write_sources(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    for (position, source_id) in library.source_order.iter().enumerate() {
        let source = library.sources.get(source_id).ok_or_else(|| {
            format!(
                "source_order for library `{library_id}` references missing source `{source_id}`"
            )
        })?;
        transaction
            .execute(
                "INSERT INTO sources (
                    library_id, source_id, position, source_content_id, source_root_id, source_root_path,
                    source_uri, relative_path, source_type, media_type, kind, status, status_reason,
                    page_count, duration_ms, observed_size_bytes, observed_modified_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    library_id,
                    source.id,
                    usize_to_i64(position, "sources.position")?,
                    source.source_content_id,
                    source.source_root_id,
                    source.source_root_path,
                    source.source_uri,
                    source.relative_path,
                    source.source_type,
                    source.media_type,
                    source.kind,
                    source.status,
                    source.status_reason,
                    optional_usize_to_i64(source.page_count, "sources.page_count")?,
                    optional_u64_to_i64(source.duration_ms, "sources.duration_ms")?,
                    optional_u64_to_i64(source.observed_size_bytes, "sources.observed_size_bytes")?,
                    optional_u128_to_i64(
                        source.observed_modified_at_ms,
                        "sources.observed_modified_at_ms",
                    )?,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write source {} for library {library_id}: {error}",
                    source.id
                )
            })?;
    }
    Ok(())
}

fn write_contents(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut contents = BTreeMap::<String, ContentRecord>::new();
    for library in snapshot.libraries.values() {
        contents.extend(
            library
                .contents
                .iter()
                .map(|(content_id, content)| (content_id.clone(), content.clone())),
        );
    }
    for content in contents.values() {
        transaction
            .execute(
                "INSERT INTO contents (
                    content_id, size_bytes, fast_fingerprint, sha256, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    content.id,
                    optional_u64_to_i64(content.size_bytes, "contents.size_bytes")?,
                    content.fast_fingerprint,
                    content.sha256,
                    u128_to_i64(content.created_at_ms, "contents.created_at_ms")?,
                ],
            )
            .map_err(|error| format!("Failed to write content {}: {error}", content.id))?;
    }
    Ok(())
}

fn write_source_asset_locations(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    for (position, location_id) in library.source_asset_location_order.iter().enumerate() {
        let location = library.source_asset_locations.get(location_id).ok_or_else(|| {
            format!(
                "source_asset_location_order for library `{library_id}` references missing location `{location_id}`"
            )
        })?;
        let locator_json = serde_json::to_string(&location.locator)
            .map_err(|error| format!("Failed to encode source asset location locator: {error}"))?;
        transaction
            .execute(
                "INSERT INTO source_asset_locations (
                    library_id, location_id, position, source_id, asset_id, locator_json, visibility
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    library_id,
                    location.id,
                    usize_to_i64(position, "source_asset_locations.position")?,
                    location.source_id,
                    location.asset_id,
                    locator_json,
                    location.visibility,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write source asset location {} for library {library_id}: {error}",
                    location.id
                )
            })?;
    }
    Ok(())
}

fn write_assets(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut assets = BTreeMap::<String, AssetRecord>::new();
    for library in snapshot.libraries.values() {
        assets.extend(
            library
                .assets
                .iter()
                .map(|(asset_id, asset)| (asset_id.clone(), asset.clone())),
        );
    }
    for (position, asset) in assets.values().enumerate() {
        let locator_json = serde_json::to_string(&asset.locator)
            .map_err(|error| format!("Failed to encode asset locator: {error}"))?;
        let neighbor_context_json = serde_json::to_string(&asset.neighbor_context)
            .map_err(|error| format!("Failed to encode asset neighbor context: {error}"))?;
        transaction
            .execute(
                "INSERT INTO assets (
                    asset_id, position, source_content_id, asset_type,
                    locator_json, derivation_signature, neighbor_context_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    asset.id,
                    usize_to_i64(position, "assets.position")?,
                    asset.source_content_id,
                    asset.asset_type,
                    locator_json,
                    asset.derivation_signature,
                    neighbor_context_json,
                ],
            )
            .map_err(|error| format!("Failed to write asset {}: {error}", asset.id))?;
    }
    Ok(())
}

fn write_units(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut asset_positions = BTreeMap::<String, BTreeMap<String, usize>>::new();
    let mut units = BTreeMap::<String, UnitRecord>::new();
    for library in snapshot.libraries.values() {
        for asset in library.assets.values() {
            asset_positions.entry(asset.id.clone()).or_insert_with(|| {
                asset
                    .unit_ids
                    .iter()
                    .enumerate()
                    .map(|(position, unit_id)| (unit_id.clone(), position))
                    .collect()
            });
        }
        units.extend(
            library
                .units
                .iter()
                .map(|(unit_id, unit)| (unit_id.clone(), unit.clone())),
        );
    }

    for (position, unit) in units.values().enumerate() {
        let asset_position = asset_positions
            .get(&unit.asset_id)
            .and_then(|positions| positions.get(&unit.id))
            .copied()
            .ok_or_else(|| {
                format!(
                    "unit `{}` is missing from asset `{}` unit_ids",
                    unit.id, unit.asset_id
                )
            })?;
        let locator_json = serde_json::to_string(&unit.locator)
            .map_err(|error| format!("Failed to encode unit locator: {error}"))?;
        let neighbor_context_json = serde_json::to_string(&unit.neighbor_context)
            .map_err(|error| format!("Failed to encode unit neighbor context: {error}"))?;
        transaction
            .execute(
                "INSERT INTO units (
                    unit_id, position, asset_id, asset_position, point_id, unit_type,
                    derivation_signature, locator_json, neighbor_context_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    unit.id,
                    usize_to_i64(position, "units.position")?,
                    unit.asset_id,
                    usize_to_i64(asset_position, "units.asset_position")?,
                    u64_to_i64(unit.point_id, "units.point_id")?,
                    unit.unit_type,
                    unit.derivation_signature,
                    locator_json,
                    neighbor_context_json,
                ],
            )
            .map_err(|error| format!("Failed to write unit {}: {error}", unit.id))?;
    }
    Ok(())
}

fn write_vector_spaces(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut vector_spaces = BTreeMap::<String, VectorSpaceRecord>::new();
    for library in snapshot.libraries.values() {
        vector_spaces.extend(
            library
                .vector_spaces
                .iter()
                .map(|(id, vector_space)| (id.clone(), vector_space.clone())),
        );
    }
    for vector_space in vector_spaces.values() {
        transaction
            .execute(
                "INSERT INTO vector_spaces (
                    vector_space_id, provider_id, model_id, model_version,
                    model_revision, vector_type
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    vector_space.id,
                    vector_space.provider_id,
                    vector_space.model_id,
                    vector_space.model_version,
                    vector_space.model_revision,
                    vector_space.vector_type,
                ],
            )
            .map_err(|error| {
                format!("Failed to write vector space {}: {error}", vector_space.id)
            })?;
    }
    Ok(())
}

fn write_unit_indexes(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut indexes = BTreeMap::<String, UnitIndexRecord>::new();
    for library in snapshot.libraries.values() {
        indexes.extend(
            library
                .unit_indexes
                .iter()
                .map(|(key, index)| (key.clone(), index.clone())),
        );
    }
    for index in indexes.values() {
        let vector_ref_json = index
            .vector_ref
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| format!("Failed to encode unit index vector_ref: {error}"))?;
        transaction
            .execute(
                "INSERT INTO unit_indexes (
                    unit_id, vector_space_id, status, visibility, vector_ref_json,
                    job_id, error_summary
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    index.unit_id,
                    index.vector_space_id,
                    index.status,
                    index.visibility,
                    vector_ref_json,
                    index.job_id,
                    index.error_summary,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write unit index {}/{}: {error}",
                    index.unit_id, index.vector_space_id
                )
            })?;
    }
    Ok(())
}

fn write_content_e2e_index_states(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    let mut states = BTreeMap::<String, ContentE2eIndexStateRecord>::new();
    for library in snapshot.libraries.values() {
        states.extend(
            library
                .content_e2e_index_states
                .iter()
                .map(|(key, state)| (key.clone(), state.clone())),
        );
    }
    for state in states.values() {
        transaction
            .execute(
                "INSERT INTO content_e2e_index_states (
                    content_id, pipe_signature, vector_space_id, indexed_at_ms
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    state.content_id,
                    state.pipe_signature,
                    state.vector_space_id,
                    u128_to_i64(
                        state.indexed_at_ms,
                        "content_e2e_index_states.indexed_at_ms"
                    )?,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write content e2e index state {}/{}/{}: {error}",
                    state.content_id, state.pipe_signature, state.vector_space_id
                )
            })?;
    }
    Ok(())
}

fn write_query_assets(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    for asset in snapshot.query_assets.values() {
        transaction
            .execute(
                "INSERT INTO query_assets (
                    query_asset_id, owner_scope, library_id, source_type, content_type,
                    path, original_filename, page_count, duration_ms, size_bytes,
                    created_at_ms, expires_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    asset.id,
                    asset.owner_scope,
                    asset.library_id,
                    asset.source_type,
                    asset.content_type,
                    asset.path,
                    asset.original_filename,
                    optional_usize_to_i64(asset.page_count, "query_assets.page_count")?,
                    optional_u64_to_i64(asset.duration_ms, "query_assets.duration_ms")?,
                    usize_to_i64(asset.size_bytes, "query_assets.size_bytes")?,
                    u128_to_i64(asset.created_at_ms, "query_assets.created_at_ms")?,
                    u128_to_i64(asset.expires_at_ms, "query_assets.expires_at_ms")?,
                ],
            )
            .map_err(|error| format!("Failed to write query asset {}: {error}", asset.id))?;
    }
    Ok(())
}

fn write_query_history(
    transaction: &Transaction<'_>,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    for (position, query_id) in snapshot.query_history_order.iter().enumerate() {
        let record = snapshot
            .query_history
            .get(query_id)
            .ok_or_else(|| format!("query_history_order references missing query `{query_id}`"))?;
        let input_json = serde_json::to_string(&record.input_json)
            .map_err(|error| format!("Failed to encode query_history input_json: {error}"))?;
        let search_scope_json =
            serde_json::to_string(&record.search_scope_json).map_err(|error| {
                format!("Failed to encode query_history search_scope_json: {error}")
            })?;
        let filters_json = record
            .filters_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| format!("Failed to encode query_history filters_json: {error}"))?;
        let target_content_types_json = record
            .target_content_types_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                format!("Failed to encode query_history target_content_types_json: {error}")
            })?;
        transaction
            .execute(
                "INSERT INTO query_history (
                    query_id, position, created_at_ms, source, query_kind, input_kind,
                    input_summary, input_json, search_scope_json, filters_json,
                    target_content_types_json, top_k, status, result_count,
                    error_code, error_message, duration_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    record.id,
                    usize_to_i64(position, "query_history.position")?,
                    u128_to_i64(record.created_at_ms, "query_history.created_at_ms")?,
                    record.source,
                    record.query_kind,
                    record.input_kind,
                    record.input_summary,
                    input_json,
                    search_scope_json,
                    filters_json,
                    target_content_types_json,
                    optional_usize_to_i64(record.top_k, "query_history.top_k")?,
                    record.status,
                    optional_usize_to_i64(record.result_count, "query_history.result_count")?,
                    record.error_code,
                    record.error_message,
                    u128_to_i64(record.duration_ms, "query_history.duration_ms")?,
                ],
            )
            .map_err(|error| format!("Failed to write query history {}: {error}", record.id))?;
    }
    Ok(())
}

fn sqlite_io(path: &FsPath, action: &str, error: rusqlite::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!(
            "Failed to {action} in durable state store {}: {error}",
            path.display()
        ),
    )
}

fn invalid_data(path: &FsPath, message: impl Into<String>) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "Invalid durable state store {}: {}",
            path.display(),
            message.into()
        ),
    )
}

fn parse_json<T>(path: &FsPath, payload: &str, field: &str) -> Result<T, io::Error>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(payload).map_err(|error| {
        invalid_data(
            path,
            format!("failed to decode {field} JSON payload: {error}"),
        )
    })
}

fn bool_to_i64(value: bool) -> i64 {
    i64::from(value)
}

fn i64_to_bool(path: &FsPath, value: i64, field: &str) -> Result<bool, io::Error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_data(path, format!("{field} must be 0 or 1"))),
    }
}

fn i64_to_u64(path: &FsPath, value: i64, field: &str) -> Result<u64, io::Error> {
    u64::try_from(value).map_err(|_| invalid_data(path, format!("{field} must be non-negative")))
}

fn i64_to_u128(path: &FsPath, value: i64, field: &str) -> Result<u128, io::Error> {
    u128::try_from(value).map_err(|_| invalid_data(path, format!("{field} must be non-negative")))
}

fn optional_i64_to_u64(
    path: &FsPath,
    value: Option<i64>,
    field: &str,
) -> Result<Option<u64>, io::Error> {
    value
        .map(|value| i64_to_u64(path, value, field))
        .transpose()
}

fn optional_i64_to_u128(
    path: &FsPath,
    value: Option<i64>,
    field: &str,
) -> Result<Option<u128>, io::Error> {
    value
        .map(|value| i64_to_u128(path, value, field))
        .transpose()
}

fn optional_i64_to_usize(
    path: &FsPath,
    value: Option<i64>,
    field: &str,
) -> Result<Option<usize>, io::Error> {
    value
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| invalid_data(path, format!("{field} must be non-negative")))
        })
        .transpose()
}

fn u64_to_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

fn u128_to_i64(value: u128, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

fn usize_to_i64(value: usize, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

fn optional_u64_to_i64(value: Option<u64>, field: &str) -> Result<Option<i64>, String> {
    value.map(|value| u64_to_i64(value, field)).transpose()
}

fn optional_u128_to_i64(value: Option<u128>, field: &str) -> Result<Option<i64>, String> {
    value.map(|value| u128_to_i64(value, field)).transpose()
}

fn optional_usize_to_i64(value: Option<usize>, field: &str) -> Result<Option<i64>, String> {
    value.map(|value| usize_to_i64(value, field)).transpose()
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn structured_store_roundtrip_restores_durable_records() {
        let path = unique_test_file_path("structured-roundtrip.sqlite");
        let mut snapshot = DurableAppStateSnapshot {
            version: STRUCTURED_STATE_SCHEMA_VERSION as u32,
            library_order: vec!["alpha".to_string()],
            libraries: BTreeMap::new(),
            query_assets: BTreeMap::new(),
            query_history: BTreeMap::new(),
            query_history_order: Vec::new(),
        };
        snapshot.libraries.insert(
            "alpha".to_string(),
            DurableLibraryRecord {
                id: "alpha".to_string(),
                display_name: "Alpha".to_string(),
                lifecycle_state: "archived".to_string(),
                archived_at_ms: Some(1234),
                source_roots: BTreeMap::from([(
                    "root_1".to_string(),
                    DurableSourceRootRecord {
                        id: "root_1".to_string(),
                        root_path: "/tmp/root".to_string(),
                        enabled: true,
                        rules: SourceRootRulesPayload {
                            include_globs: vec!["**/*.pdf".to_string()],
                            exclude_globs: vec!["tmp/**".to_string()],
                            include_extensions: vec!["pdf".to_string()],
                        },
                    },
                )]),
                source_root_order: vec!["root_1".to_string()],
                sources: BTreeMap::from([(
                    "src_1".to_string(),
                    SourceRecord {
                        id: "src_1".to_string(),
                        source_root_id: Some("root_1".to_string()),
                        source_root_path: Some("/tmp/root".to_string()),
                        source_path: "/tmp/root/report.pdf".to_string(),
                        source_uri: "file:///tmp/root/report.pdf".to_string(),
                        relative_path: Some("report.pdf".to_string()),
                        source_type: "document".to_string(),
                        media_type: "application/pdf".to_string(),
                        kind: "document_page".to_string(),
                        status: "active".to_string(),
                        status_reason: None,
                        page_count: Some(2),
                        duration_ms: None,
                        observed_size_bytes: Some(99),
                        observed_modified_at_ms: Some(5678),
                        source_content_id: "content_source".to_string(),
                        asset_ids: vec!["asset_2".to_string(), "asset_1".to_string()],
                    },
                )]),
                source_order: vec!["src_1".to_string()],
                contents: BTreeMap::from([
                    (
                        "content_source".to_string(),
                        ContentRecord {
                            id: "content_source".to_string(),
                            size_bytes: Some(99),
                            fast_fingerprint: Some("fast-source".to_string()),
                            sha256: Some("sha256-source".to_string()),
                            created_at_ms: 1,
                        },
                    ),
                    (
                        "content_unit_1".to_string(),
                        ContentRecord {
                            id: "content_unit_1".to_string(),
                            size_bytes: None,
                            fast_fingerprint: None,
                            sha256: None,
                            created_at_ms: 1,
                        },
                    ),
                ]),
                source_asset_locations: BTreeMap::from([
                    (
                        SourceAssetLocationRecord::key("src_1", "asset_2"),
                        SourceAssetLocationRecord {
                            id: SourceAssetLocationRecord::key("src_1", "asset_2"),
                            source_id: "src_1".to_string(),
                            asset_id: "asset_2".to_string(),
                            locator: json!({ "page": 2 }),
                            visibility: crate::ACTIVE_INDEX_VISIBILITY.to_string(),
                        },
                    ),
                    (
                        SourceAssetLocationRecord::key("src_1", "asset_1"),
                        SourceAssetLocationRecord {
                            id: SourceAssetLocationRecord::key("src_1", "asset_1"),
                            source_id: "src_1".to_string(),
                            asset_id: "asset_1".to_string(),
                            locator: json!({ "page": 1 }),
                            visibility: crate::ACTIVE_INDEX_VISIBILITY.to_string(),
                        },
                    ),
                ]),
                source_asset_location_order: vec![
                    SourceAssetLocationRecord::key("src_1", "asset_2"),
                    SourceAssetLocationRecord::key("src_1", "asset_1"),
                ],
                assets: BTreeMap::from([
                    (
                        "asset_1".to_string(),
                        AssetRecord {
                            id: "asset_1".to_string(),
                            source_id: "src_1".to_string(),
                            content_id: "content_unit_1".to_string(),
                            source_path: "/tmp/root/report.pdf".to_string(),
                            source_type: "document".to_string(),
                            source_content_id: "content_source".to_string(),
                            asset_type: "document_page".to_string(),
                            locator: json!({ "page": 1 }),
                            derivation_signature: "pdf-page:v1:1".to_string(),
                            neighbor_context: json!({ "next": "asset_2" }),
                            unit_ids: vec!["unit_1".to_string()],
                        },
                    ),
                    (
                        "asset_2".to_string(),
                        AssetRecord {
                            id: "asset_2".to_string(),
                            source_id: "src_1".to_string(),
                            content_id: "content_unit_1".to_string(),
                            source_path: "/tmp/root/report.pdf".to_string(),
                            source_type: "document".to_string(),
                            source_content_id: "content_source".to_string(),
                            asset_type: "document_page".to_string(),
                            locator: json!({ "page": 2 }),
                            derivation_signature: "pdf-page:v1:2".to_string(),
                            neighbor_context: json!({ "previous": "asset_1" }),
                            unit_ids: Vec::new(),
                        },
                    ),
                ]),
                asset_order: vec!["asset_1".to_string(), "asset_2".to_string()],
                units: BTreeMap::from([(
                    "unit_1".to_string(),
                    UnitRecord {
                        id: "unit_1".to_string(),
                        asset_id: "asset_1".to_string(),
                        content_id: "content_unit_1".to_string(),
                        point_id: 11,
                        source_id: "src_1".to_string(),
                        source_path: "/tmp/root/report.pdf".to_string(),
                        source_type: "document".to_string(),
                        asset_type: "document_page".to_string(),
                        unit_type: "page_image".to_string(),
                        derivation_signature: "pdf-page-image:v1:1".to_string(),
                        locator: json!({ "page": 1 }),
                        neighbor_context: json!({ "next": "asset_2" }),
                    },
                )]),
                unit_order: vec!["unit_1".to_string()],
                vector_spaces: BTreeMap::from([(
                    "vs_active".to_string(),
                    VectorSpaceRecord {
                        id: "vs_active".to_string(),
                        provider_id: "local_sidecar".to_string(),
                        model_id: "model".to_string(),
                        model_version: "main".to_string(),
                        model_revision: None,
                        vector_type: "multi_vector".to_string(),
                    },
                )]),
                unit_indexes: BTreeMap::from([(
                    UnitIndexRecord::key("unit_1", "vs_active"),
                    UnitIndexRecord {
                        unit_id: "unit_1".to_string(),
                        vector_space_id: "vs_active".to_string(),
                        status: "ready".to_string(),
                        visibility: crate::ACTIVE_INDEX_VISIBILITY.to_string(),
                        vector_ref: Some(json!({ "point_id": 11 })),
                        job_id: Some("job_1".to_string()),
                        error_summary: None,
                    },
                )]),
                content_e2e_index_states: BTreeMap::from([(
                    ContentE2eIndexStateRecord::key("content_source", "document:v1", "vs_active"),
                    ContentE2eIndexStateRecord {
                        content_id: "content_source".to_string(),
                        pipe_signature: "document:v1".to_string(),
                        vector_space_id: "vs_active".to_string(),
                        indexed_at_ms: 2,
                    },
                )]),
            },
        );

        write_durable_state_snapshot(&path, &snapshot).unwrap();
        let loaded = load_durable_state_snapshot(&path)
            .unwrap()
            .unwrap()
            .snapshot;
        let library = &loaded.libraries["alpha"];

        assert_eq!(loaded.library_order, vec!["alpha"]);
        assert_eq!(library.lifecycle_state, "archived");
        assert_eq!(library.source_root_order, vec!["root_1"]);
        assert_eq!(library.source_order, vec!["src_1"]);
        assert_eq!(library.asset_order, vec!["asset_1", "asset_2"]);
        assert_eq!(
            library.sources["src_1"].asset_ids,
            vec!["asset_2", "asset_1"]
        );
        assert_eq!(library.assets["asset_1"].locator, json!({ "page": 1 }));
        assert_eq!(library.units["unit_1"].unit_type, "page_image");
        assert_eq!(library.units["unit_1"].point_id, 11);
        assert!(library.vector_spaces.contains_key("vs_active"));
        assert_eq!(
            library.unit_indexes[&UnitIndexRecord::key("unit_1", "vs_active")].visibility,
            "active"
        );

        let connection = Connection::open(&path).unwrap();
        let version: i64 = connection
            .query_row(
                "SELECT schema_version FROM state_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, STRUCTURED_STATE_SCHEMA_VERSION);
        assert!(!table_exists(&connection, "durable_state_snapshots").unwrap());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn legacy_snapshot_store_is_rejected() {
        let path = unique_test_file_path("legacy-snapshot.sqlite");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "CREATE TABLE durable_state_snapshots (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    payload_json TEXT NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO durable_state_snapshots (id, payload_json, updated_at_ms)
                 VALUES (1, '{}', 0)",
                [],
            )
            .unwrap();
        drop(connection);

        let error = load_durable_state_snapshot(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("legacy durable snapshot store"));

        let _ = std::fs::remove_file(path);
    }

    fn unique_test_file_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "fauni-search-{name}-{}-{}.sqlite",
            std::process::id(),
            current_unix_ms()
        ));
        path
    }

    #[test]
    fn structured_store_loads_empty_initialized_store() {
        let path = unique_test_file_path("empty-structured.sqlite");
        let connection = Connection::open(&path).unwrap();
        initialize_durable_state_store(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO state_meta (id, schema_version, updated_at_ms) VALUES (1, ?1, 0)",
                params![STRUCTURED_STATE_SCHEMA_VERSION],
            )
            .unwrap();
        drop(connection);

        let loaded = load_durable_state_snapshot(&path)
            .unwrap()
            .unwrap()
            .snapshot;
        assert!(loaded.library_order.is_empty());
        assert!(loaded.libraries.is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unsupported_structured_schema_version_is_rejected() {
        let path = unique_test_file_path("unsupported-schema.sqlite");
        let connection = Connection::open(&path).unwrap();
        initialize_durable_state_store(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO state_meta (id, schema_version, updated_at_ms)
                 VALUES (1, 5, 0)",
                [],
            )
            .unwrap();

        let error = load_durable_state_snapshot(&path).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Unsupported durable state schema version 5"));
        assert!(message.contains("Reset or cut over"));

        let _ = std::fs::remove_file(path);
    }
}
