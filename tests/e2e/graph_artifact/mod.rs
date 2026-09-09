use std::{process::Command, str::from_utf8, thread, time::Duration};

use assert_cmd::cargo;
use rand::RngExt;
use tracing::error;

pub(super) const E2E_TEST_ARTIFACT_DIGEST: &str =
    "sha256:9e4067d19c891ff871a6bbe01d1ee157bca7705677394390b2ae1b7fa9af45de";

mod fetch;
mod list_tags;
mod tag;
mod untag;

/// Generates a tag string with a random 64-bit hex suffix so concurrent CI jobs
/// (which all share the `rover-e2e-tests` graph, across every runner in the
/// matrix) don't collide on the same tag name.
pub(super) fn random_tag(prefix: &str) -> String {
    let n: u64 = rand::rng().random();
    format!("{prefix}-{n:016x}")
}

/// The registry enforces a minimum interval of about a second between
/// successive operations on the same tag, measured from when the previous one
/// started. `tag` only returns once the tag is applied, so the previous
/// operation is already complete by the time a test untags; the limit is purely
/// on how soon the same tag can be touched again, which leaves waiting as the
/// only way to avoid a rejected-then-retried untag.
pub(super) fn wait_for_tag_reuse_window() {
    thread::sleep(Duration::from_secs(2));
}

/// Removes a tag, logging (but not failing) if the removal does not succeed.
pub(super) fn delete_tag(graph_id: &str, tag: &str) {
    wait_for_tag_reuse_window();
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
/// panics before reaching the explicit cleanup call. A test that removes the
/// tag itself should call [`TagCleanup::disarm`] so the guard doesn't issue a
/// redundant untag.
pub(super) struct TagCleanup {
    graph_id: String,
    tag: String,
    armed: bool,
}

impl TagCleanup {
    pub(super) const fn new(graph_id: String, tag: String) -> Self {
        Self {
            graph_id,
            tag,
            armed: true,
        }
    }

    pub(super) const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TagCleanup {
    fn drop(&mut self) {
        if self.armed {
            delete_tag(&self.graph_id, &self.tag);
        }
    }
}
