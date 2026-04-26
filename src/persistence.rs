use crate::{
    api::SourceRootRulesPayload,
    model::{RetiredVectorSpaceRecord, SourceRecord, VisualUnitRecord},
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path as FsPath,
    time::{SystemTime, UNIX_EPOCH},
};

const STRUCTURED_STATE_SCHEMA_VERSION: i64 = 3;

#[derive(Debug)]
pub(crate) struct LoadedDurableStateSnapshot {
    pub(crate) snapshot: DurableAppStateSnapshot,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DurableAppStateSnapshot {
    pub(crate) version: u32,
    pub(crate) library_order: Vec<String>,
    pub(crate) libraries: BTreeMap<String, DurableLibraryRecord>,
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
    pub(crate) sources: BTreeMap<String, SourceRecord>,
    pub(crate) source_order: Vec<String>,
    pub(crate) visual_units: BTreeMap<String, VisualUnitRecord>,
    pub(crate) visual_unit_order: Vec<String>,
    #[serde(default)]
    pub(crate) active_vector_spaces: BTreeSet<String>,
    #[serde(default)]
    pub(crate) retired_vector_spaces: BTreeMap<String, RetiredVectorSpaceRecord>,
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
                "Unsupported durable state schema version {schema_version} in {}; expected {STRUCTURED_STATE_SCHEMA_VERSION}",
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
            source_root_id TEXT NULL,
            source_root_path TEXT NULL,
            source_path TEXT NOT NULL,
            relative_path TEXT NULL,
            source_type TEXT NOT NULL,
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
        CREATE TABLE IF NOT EXISTS visual_units (
            library_id TEXT NOT NULL,
            visual_unit_id TEXT NOT NULL,
            library_position INTEGER NOT NULL,
            source_id TEXT NOT NULL,
            source_position INTEGER NOT NULL,
            point_id INTEGER NOT NULL,
            source_path TEXT NOT NULL,
            source_type TEXT NOT NULL,
            kind TEXT NOT NULL,
            locator_json TEXT NOT NULL,
            neighbor_context_json TEXT NOT NULL,
            PRIMARY KEY (library_id, visual_unit_id),
            UNIQUE (library_id, library_position),
            UNIQUE (library_id, source_id, source_position)
        );
        CREATE TABLE IF NOT EXISTS library_active_vector_spaces (
            library_id TEXT NOT NULL,
            vector_space_id TEXT NOT NULL,
            PRIMARY KEY (library_id, vector_space_id)
        );
        CREATE TABLE IF NOT EXISTS retired_vector_spaces (
            library_id TEXT NOT NULL,
            vector_space_id TEXT NOT NULL,
            retired_at_ms INTEGER NOT NULL,
            PRIMARY KEY (library_id, vector_space_id)
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
    };

    load_libraries(path, connection, &mut snapshot)?;
    load_source_roots(path, connection, &mut snapshot)?;
    load_sources(path, connection, &mut snapshot)?;
    load_visual_units(path, connection, &mut snapshot)?;
    load_active_vector_spaces(path, connection, &mut snapshot)?;
    load_retired_vector_spaces(path, connection, &mut snapshot)?;

    Ok(LoadedDurableStateSnapshot { snapshot })
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
                sources: BTreeMap::new(),
                source_order: Vec::new(),
                visual_units: BTreeMap::new(),
                visual_unit_order: Vec::new(),
                active_vector_spaces: BTreeSet::new(),
                retired_vector_spaces: BTreeMap::new(),
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
            "SELECT library_id, source_id, source_root_id, source_root_path, source_path,
                    relative_path, source_type, kind, status, status_reason, page_count,
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
        let source_root_id: Option<String> = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read source source_root_id", error))?;
        let source_root_path: Option<String> = row
            .get(3)
            .map_err(|error| sqlite_io(path, "read source source_root_path", error))?;
        let source_path: String = row
            .get(4)
            .map_err(|error| sqlite_io(path, "read source_path", error))?;
        let relative_path: Option<String> = row
            .get(5)
            .map_err(|error| sqlite_io(path, "read relative_path", error))?;
        let source_type: String = row
            .get(6)
            .map_err(|error| sqlite_io(path, "read source_type", error))?;
        let kind: String = row
            .get(7)
            .map_err(|error| sqlite_io(path, "read source kind", error))?;
        let status: String = row
            .get(8)
            .map_err(|error| sqlite_io(path, "read source status", error))?;
        let status_reason: Option<String> = row
            .get(9)
            .map_err(|error| sqlite_io(path, "read source status_reason", error))?;
        let page_count = optional_i64_to_usize(
            path,
            row.get(10)
                .map_err(|error| sqlite_io(path, "read source page_count", error))?,
            "sources.page_count",
        )?;
        let duration_ms = optional_i64_to_u64(
            path,
            row.get(11)
                .map_err(|error| sqlite_io(path, "read source duration_ms", error))?,
            "sources.duration_ms",
        )?;
        let observed_size_bytes = optional_i64_to_u64(
            path,
            row.get(12)
                .map_err(|error| sqlite_io(path, "read source observed_size_bytes", error))?,
            "sources.observed_size_bytes",
        )?;
        let observed_modified_at_ms = optional_i64_to_u128(
            path,
            row.get(13)
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
        library.sources.insert(
            source_id,
            SourceRecord {
                id: row
                    .get(1)
                    .map_err(|error| sqlite_io(path, "read source_id", error))?,
                source_root_id,
                source_root_path,
                source_path,
                relative_path,
                source_type,
                kind,
                status,
                status_reason,
                page_count,
                duration_ms,
                observed_size_bytes,
                observed_modified_at_ms,
                visual_unit_ids: Vec::new(),
            },
        );
    }
    Ok(())
}

fn load_visual_units(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut source_visual_unit_ids = BTreeMap::<(String, String), Vec<(i64, String)>>::new();
    let mut statement = connection
        .prepare(
            "SELECT library_id, visual_unit_id, source_id, source_position, point_id,
                    source_path, source_type, kind, locator_json, neighbor_context_json
             FROM visual_units
             ORDER BY library_id ASC, library_position ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare visual_units query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query visual_units", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read visual_unit row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read visual_unit library_id", error))?;
        let visual_unit_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read visual_unit_id", error))?;
        let source_id: String = row
            .get(2)
            .map_err(|error| sqlite_io(path, "read visual_unit source_id", error))?;
        let source_position: i64 = row
            .get(3)
            .map_err(|error| sqlite_io(path, "read visual_unit source_position", error))?;
        let point_id = i64_to_u64(
            path,
            row.get(4)
                .map_err(|error| sqlite_io(path, "read visual_unit point_id", error))?,
            "visual_units.point_id",
        )?;
        let source_path: String = row
            .get(5)
            .map_err(|error| sqlite_io(path, "read visual_unit source_path", error))?;
        let source_type: String = row
            .get(6)
            .map_err(|error| sqlite_io(path, "read visual_unit source_type", error))?;
        let kind: String = row
            .get(7)
            .map_err(|error| sqlite_io(path, "read visual_unit kind", error))?;
        let locator_json: String = row
            .get(8)
            .map_err(|error| sqlite_io(path, "read visual_unit locator_json", error))?;
        let neighbor_context_json: String = row
            .get(9)
            .map_err(|error| sqlite_io(path, "read visual_unit neighbor_context_json", error))?;
        let locator = parse_json::<Value>(path, &locator_json, "visual_units.locator_json")?;
        let neighbor_context = parse_json::<Value>(
            path,
            &neighbor_context_json,
            "visual_units.neighbor_context_json",
        )?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("visual_unit {visual_unit_id} references missing library {library_id}"),
            )
        })?;
        if !library.sources.contains_key(&source_id) {
            return Err(invalid_data(
                path,
                format!(
                    "visual_unit {visual_unit_id} references missing source {source_id} in library {library_id}"
                ),
            ));
        }

        library.visual_unit_order.push(visual_unit_id.clone());
        library.visual_units.insert(
            visual_unit_id.clone(),
            VisualUnitRecord {
                id: visual_unit_id.clone(),
                point_id,
                source_id: source_id.clone(),
                source_path,
                source_type,
                kind,
                locator,
                neighbor_context,
            },
        );
        source_visual_unit_ids
            .entry((library_id, source_id))
            .or_default()
            .push((source_position, visual_unit_id));
    }

    for ((library_id, source_id), mut visual_unit_ids) in source_visual_unit_ids {
        visual_unit_ids.sort_by_key(|(position, _)| *position);
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("missing library {library_id} while assigning visual units"),
            )
        })?;
        let source = library.sources.get_mut(&source_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("missing source {source_id} while assigning visual units"),
            )
        })?;
        source.visual_unit_ids = visual_unit_ids
            .into_iter()
            .map(|(_, visual_unit_id)| visual_unit_id)
            .collect();
    }

    Ok(())
}

fn load_active_vector_spaces(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, vector_space_id
             FROM library_active_vector_spaces
             ORDER BY library_id ASC, vector_space_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare active vector spaces query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query active vector spaces", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read active vector space row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read active vector space library_id", error))?;
        let vector_space_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read active vector_space_id", error))?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!(
                    "active vector space {vector_space_id} references missing library {library_id}"
                ),
            )
        })?;
        library.active_vector_spaces.insert(vector_space_id);
    }
    Ok(())
}

fn load_retired_vector_spaces(
    path: &FsPath,
    connection: &Connection,
    snapshot: &mut DurableAppStateSnapshot,
) -> Result<(), io::Error> {
    let mut statement = connection
        .prepare(
            "SELECT library_id, vector_space_id, retired_at_ms
             FROM retired_vector_spaces
             ORDER BY library_id ASC, vector_space_id ASC",
        )
        .map_err(|error| sqlite_io(path, "prepare retired vector spaces query", error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| sqlite_io(path, "query retired vector spaces", error))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| sqlite_io(path, "read retired vector space row", error))?
    {
        let library_id: String = row
            .get(0)
            .map_err(|error| sqlite_io(path, "read retired vector space library_id", error))?;
        let vector_space_id: String = row
            .get(1)
            .map_err(|error| sqlite_io(path, "read retired vector_space_id", error))?;
        let retired_at_ms = i64_to_u128(
            path,
            row.get(2)
                .map_err(|error| sqlite_io(path, "read retired_at_ms", error))?,
            "retired_vector_spaces.retired_at_ms",
        )?;
        let library = snapshot.libraries.get_mut(&library_id).ok_or_else(|| {
            invalid_data(
                path,
                format!("retired vector space {vector_space_id} references missing library {library_id}"),
            )
        })?;
        library
            .retired_vector_spaces
            .insert(vector_space_id, RetiredVectorSpaceRecord { retired_at_ms });
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
        write_visual_units(transaction, &library.id, library)?;
        write_active_vector_spaces(transaction, &library.id, library)?;
        write_retired_vector_spaces(transaction, &library.id, library)?;
    }

    Ok(())
}

fn clear_structured_tables(transaction: &Transaction<'_>) -> Result<(), String> {
    for table in [
        "retired_vector_spaces",
        "library_active_vector_spaces",
        "visual_units",
        "sources",
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
                    library_id, source_id, position, source_root_id, source_root_path,
                    source_path, relative_path, source_type, kind, status, status_reason,
                    page_count, duration_ms, observed_size_bytes, observed_modified_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    library_id,
                    source.id,
                    usize_to_i64(position, "sources.position")?,
                    source.source_root_id,
                    source.source_root_path,
                    source.source_path,
                    source.relative_path,
                    source.source_type,
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

fn write_visual_units(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    let mut source_positions = BTreeMap::<String, BTreeMap<String, usize>>::new();
    for source in library.sources.values() {
        source_positions.insert(
            source.id.clone(),
            source
                .visual_unit_ids
                .iter()
                .enumerate()
                .map(|(position, visual_unit_id)| (visual_unit_id.clone(), position))
                .collect(),
        );
    }

    for (library_position, visual_unit_id) in library.visual_unit_order.iter().enumerate() {
        let visual_unit = library.visual_units.get(visual_unit_id).ok_or_else(|| {
            format!(
                "visual_unit_order for library `{library_id}` references missing visual unit `{visual_unit_id}`"
            )
        })?;
        let source_position = source_positions
            .get(&visual_unit.source_id)
            .and_then(|positions| positions.get(visual_unit_id))
            .copied()
            .ok_or_else(|| {
                format!(
                    "visual unit `{visual_unit_id}` is missing from source `{}` visual_unit_ids",
                    visual_unit.source_id
                )
            })?;
        let locator_json = serde_json::to_string(&visual_unit.locator)
            .map_err(|error| format!("Failed to encode visual unit locator: {error}"))?;
        let neighbor_context_json = serde_json::to_string(&visual_unit.neighbor_context)
            .map_err(|error| format!("Failed to encode visual unit neighbor context: {error}"))?;
        transaction
            .execute(
                "INSERT INTO visual_units (
                    library_id, visual_unit_id, library_position, source_id, source_position,
                    point_id, source_path, source_type, kind, locator_json, neighbor_context_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    library_id,
                    visual_unit.id,
                    usize_to_i64(library_position, "visual_units.library_position")?,
                    visual_unit.source_id,
                    usize_to_i64(source_position, "visual_units.source_position")?,
                    u64_to_i64(visual_unit.point_id, "visual_units.point_id")?,
                    visual_unit.source_path,
                    visual_unit.source_type,
                    visual_unit.kind,
                    locator_json,
                    neighbor_context_json,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write visual unit {} for library {library_id}: {error}",
                    visual_unit.id
                )
            })?;
    }
    Ok(())
}

fn write_active_vector_spaces(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    for vector_space_id in &library.active_vector_spaces {
        transaction
            .execute(
                "INSERT INTO library_active_vector_spaces (library_id, vector_space_id)
                 VALUES (?1, ?2)",
                params![library_id, vector_space_id],
            )
            .map_err(|error| {
                format!(
                    "Failed to write active vector space {vector_space_id} for library {library_id}: {error}"
                )
            })?;
    }
    Ok(())
}

fn write_retired_vector_spaces(
    transaction: &Transaction<'_>,
    library_id: &str,
    library: &DurableLibraryRecord,
) -> Result<(), String> {
    for (vector_space_id, retired) in &library.retired_vector_spaces {
        transaction
            .execute(
                "INSERT INTO retired_vector_spaces (library_id, vector_space_id, retired_at_ms)
                 VALUES (?1, ?2, ?3)",
                params![
                    library_id,
                    vector_space_id,
                    u128_to_i64(retired.retired_at_ms, "retired_vector_spaces.retired_at_ms")?,
                ],
            )
            .map_err(|error| {
                format!(
                    "Failed to write retired vector space {vector_space_id} for library {library_id}: {error}"
                )
            })?;
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
                        relative_path: Some("report.pdf".to_string()),
                        source_type: "document".to_string(),
                        kind: "document_page".to_string(),
                        status: "active".to_string(),
                        status_reason: None,
                        page_count: Some(2),
                        duration_ms: None,
                        observed_size_bytes: Some(99),
                        observed_modified_at_ms: Some(5678),
                        visual_unit_ids: vec!["vu_2".to_string(), "vu_1".to_string()],
                    },
                )]),
                source_order: vec!["src_1".to_string()],
                visual_units: BTreeMap::from([
                    (
                        "vu_1".to_string(),
                        VisualUnitRecord {
                            id: "vu_1".to_string(),
                            point_id: 11,
                            source_id: "src_1".to_string(),
                            source_path: "/tmp/root/report.pdf".to_string(),
                            source_type: "document".to_string(),
                            kind: "document_page".to_string(),
                            locator: json!({ "page": 1 }),
                            neighbor_context: json!({ "next": "vu_2" }),
                        },
                    ),
                    (
                        "vu_2".to_string(),
                        VisualUnitRecord {
                            id: "vu_2".to_string(),
                            point_id: 12,
                            source_id: "src_1".to_string(),
                            source_path: "/tmp/root/report.pdf".to_string(),
                            source_type: "document".to_string(),
                            kind: "document_page".to_string(),
                            locator: json!({ "page": 2 }),
                            neighbor_context: json!({ "previous": "vu_1" }),
                        },
                    ),
                ]),
                visual_unit_order: vec!["vu_1".to_string(), "vu_2".to_string()],
                active_vector_spaces: BTreeSet::from(["vs_active".to_string()]),
                retired_vector_spaces: BTreeMap::from([(
                    "vs_old".to_string(),
                    RetiredVectorSpaceRecord {
                        retired_at_ms: 9999,
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
        assert_eq!(library.visual_unit_order, vec!["vu_1", "vu_2"]);
        assert_eq!(
            library.sources["src_1"].visual_unit_ids,
            vec!["vu_2", "vu_1"]
        );
        assert_eq!(library.visual_units["vu_1"].locator, json!({ "page": 1 }));
        assert!(library.active_vector_spaces.contains("vs_active"));
        assert_eq!(library.retired_vector_spaces["vs_old"].retired_at_ms, 9999);

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
}
