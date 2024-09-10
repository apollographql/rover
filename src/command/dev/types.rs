use reqwest::Url;

use crate::command::supergraph::compose::CompositionOutput;

pub type SubgraphName = String;
pub type SubgraphUrl = Url;
pub type SubgraphSdl = String;
pub type SubgraphKey = (SubgraphName, SubgraphUrl);
pub type SubgraphKeys = Vec<SubgraphKey>;
pub type SubgraphEntry = (SubgraphKey, SubgraphSdl);
pub type CompositionResult = std::result::Result<Option<CompositionOutput>, String>;
