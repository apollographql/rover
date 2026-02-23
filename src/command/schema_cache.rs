use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result as AnyResult;
use camino::{Utf8Path, Utf8PathBuf};
use rover_client::{
    operations::graph::fetch::{self, GraphFetchInput},
    shared::GraphRef,
};
use rover_std::Fs;
use serde::{Deserialize, Serialize};

use crate::{RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

const CACHE_TTL: Duration = Duration::from_secs(300);
const CACHE_DIR_NAME: &str = "schema-cache";

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    sdl: String,
    cached_at_epoch_secs: u64,
}

/// Fetch SDL for a graph ref, using a local file cache with 5-min TTL.
///
/// When `no_cache` is `true`, always fetches from the registry but still
/// writes the result to cache for subsequent calls.
pub async fn fetch_sdl_cached(
    graph_ref: &GraphRef,
    profile: &ProfileOpt,
    client_config: &StudioClientConfig,
    no_cache: bool,
) -> RoverResult<String> {
    let cache_dir = cache_dir_for(client_config);
    let cache_file = cache_file_for(&cache_dir, graph_ref);

    if !no_cache && let Some(sdl) = read_cache(&cache_file) {
        return Ok(sdl);
    }

    let client = client_config.get_authenticated_client(profile)?;
    let resp = fetch::run(
        GraphFetchInput {
            graph_ref: graph_ref.clone(),
        },
        &client,
    )
    .await?;
    let sdl = resp.sdl.contents;

    // Best-effort — don't fail the command on cache write errors
    let _ = write_cache(&cache_dir, &cache_file, &sdl);

    Ok(sdl)
}

fn cache_dir_for(config: &StudioClientConfig) -> Utf8PathBuf {
    config.config.home.join(CACHE_DIR_NAME)
}

fn cache_file_for(dir: &Utf8Path, graph_ref: &GraphRef) -> Utf8PathBuf {
    dir.join(format!("{}@{}.json", graph_ref.name, graph_ref.variant))
}

fn read_cache(path: &Utf8Path) -> Option<String> {
    let contents = Fs::read_file(path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&contents).ok()?;

    let age = Duration::from_secs(now_epoch_secs().saturating_sub(entry.cached_at_epoch_secs));
    if age > CACHE_TTL {
        return None;
    }

    Some(entry.sdl)
}

fn write_cache(dir: &Utf8Path, path: &Utf8Path, sdl: &str) -> AnyResult<()> {
    Fs::create_dir_all(dir)?;
    let entry = CacheEntry {
        sdl: sdl.to_string(),
        cached_at_epoch_secs: now_epoch_secs(),
    };
    Fs::write_file(path, serde_json::to_string(&entry)?)?;
    Ok(())
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}
