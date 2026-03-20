use apollo_compiler::{Name, schema::ScalarType};

use crate::{ParsedSchema, root_paths::RootPath};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScalarDetail {
    pub name: Name,
    pub description: Option<String>,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_scalar_detail(&self, type_name: &Name, s: &ScalarType) -> ScalarDetail {
        let description = s.description.as_ref().map(|d| d.to_string());
        let via = self.find_root_paths(type_name);
        ScalarDetail {
            name: type_name.clone(),
            description,
            via,
        }
    }
}
