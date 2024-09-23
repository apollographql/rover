use crate::command::supergraph::compose::CompositionOutput;

use reqwest::Url;

pub type SubgraphName = String;
pub type SubgraphUrl = Url;
pub type SubgraphSdl = String;
pub type CompositionResult = std::result::Result<Option<CompositionOutput>, String>;
