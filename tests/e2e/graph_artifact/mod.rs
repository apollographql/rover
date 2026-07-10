use std::{process::Command, str::from_utf8};

use assert_cmd::cargo;
use rand::RngExt;
use tracing::error;

pub(super) const E2E_TEST_ARTIFACT_DIGEST: &str =
    "sha256:9e4067d19c891ff871a6bbe01d1ee157bca7705677394390b2ae1b7fa9af45de";

mod fetch;
mod list_tags;
mod tag;
mod untag;

/// Generates a tag string with a random numeric suffix so concurrent CI jobs
/// (which all share the `rover-e2e-tests` graph) don't collide on the same tag
/// name.
pub(super) fn random_tag(prefix: &str) -> String {
    let n: u16 = rand::rng().random_range(0..500);
    format!("{prefix}-{n:03}")
}

/// Removes a tag, logging (but not failing) if the removal does not succeed.
pub(super) fn delete_tag(graph_id: &str, tag: &str) {
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.args([
        "graph-artifact",
        "untag",
        tag,
        "--graph-id",
        graph_id,
        "--client-timeout",
        "120",
    ]);
    if let Ok(output) = cmd.output()
        && !output.status.success()
    {
        error!(
            "Warning: failed to delete tag '{}': {}",
            tag,
            from_utf8(&output.stderr).unwrap_or("<non-utf8>")
        );
    }
}

/// RAII guard that deletes a tag when dropped, ensuring cleanup even if a test
/// panics before reaching the explicit cleanup call.
pub(super) struct TagCleanup {
    pub graph_id: String,
    pub tag: String,
}

impl Drop for TagCleanup {
    fn drop(&mut self) {
        delete_tag(&self.graph_id, &self.tag);
    }
}
