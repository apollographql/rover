use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LatestPluginVersions {
    pub(crate) supergraph: Plugin,
    router: Plugin,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Plugin {
    pub(crate) versions: HashMap<String, String>,
    repository: String,
}
