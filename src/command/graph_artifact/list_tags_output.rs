use rover_client::operations::graph_artifact::list_tags::ListTagsResponse;
use rover_std::Style;

use crate::{command::CliOutput, utils::table};

#[derive(Debug)]
pub(super) struct ListTagsOutput(pub ListTagsResponse);

impl CliOutput for ListTagsOutput {
    fn text(&self) -> String {
        if self.0.tags.is_empty() {
            return "No tags found.".to_string();
        }
        let mut table = table::get_table();
        table.add_row(vec![
            &Style::Success.paint("Tag"),
            &Style::Success.paint("Digest"),
            &Style::Success.paint("Created At"),
        ]);
        for entry in &self.0.tags {
            table.add_row(vec![
                entry.tag.clone(),
                entry.digest.clone().unwrap_or_else(|| "N/A".to_string()),
                entry.created_at.clone(),
            ]);
        }
        table.to_string()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&self.0)
    }
}
