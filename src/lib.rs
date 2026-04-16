mod api;
mod http;
mod indexing;
mod model;
mod persistence;
mod qdrant;
mod query_assets;
mod runtime;
mod sidecar;
mod source_roots;
mod state;

#[allow(unused_imports)]
pub(crate) use api::*;
#[allow(unused_imports)]
pub(crate) use http::*;
#[allow(unused_imports)]
pub(crate) use indexing::*;
#[allow(unused_imports)]
pub(crate) use model::*;
#[allow(unused_imports)]
pub(crate) use persistence::*;
#[allow(unused_imports)]
pub(crate) use qdrant::*;
#[allow(unused_imports)]
pub(crate) use query_assets::*;
#[allow(unused_imports)]
pub(crate) use runtime::*;
#[allow(unused_imports)]
pub(crate) use sidecar::*;
#[allow(unused_imports)]
pub(crate) use source_roots::*;
#[allow(unused_imports)]
pub(crate) use state::*;

pub use http::build_app;
pub use runtime::spawn_runtime_maintenance;
pub use state::new_state;

pub(crate) const MULTIVECTOR_INDEX_LINE: &str = "multivector";
pub(crate) const DEFAULT_INDEX_EMBED_BATCH_ITEMS: usize = 8;
pub(crate) const DEFAULT_QDRANT_MAX_UPSERT_BODY_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const QDRANT_UPSERT_BODY_OVERHEAD_BYTES: usize = br#"{\"points\":[]}"#.len();
pub(crate) const SIDECAR_REQUEST_TIMEOUT_SECS: u64 = 600;
pub(crate) const TEMP_QUERY_ASSET_TTL_MS: u128 = 60 * 60 * 1000;
pub(crate) const TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS: u64 = 60;
pub(crate) const VIDEO_SEGMENT_WINDOW_MS: u64 = 8_000;
pub(crate) const VIDEO_SEGMENT_OVERLAP_MS: u64 = 2_000;
pub(crate) const APP_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const SOURCE_WATCHER_POLL_INTERVAL_SECS: u64 = 2;
pub(crate) const SOURCE_WATCHER_DEBOUNCE_MS: u128 = 1_500;
pub(crate) const STATE_SNAPSHOT_ROW_ID: i64 = 1;
