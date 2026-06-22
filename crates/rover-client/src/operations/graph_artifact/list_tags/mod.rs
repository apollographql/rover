mod by_digest;
mod by_graph;
mod runner;
mod types;

pub use runner::run;
pub use types::{ListTagsInput, ListTagsResponse};

fn reached_limit(tags: &mut Vec<types::ListTagEntry>, limit: usize) -> bool {
    if tags.len() >= limit {
        tags.truncate(limit);
        true
    } else {
        false
    }
}
