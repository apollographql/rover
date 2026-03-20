use apollo_compiler::{Name, schema::UnionType};

use crate::{ParsedSchema, root_paths::RootPath};

#[derive(Debug, Clone, serde::Serialize)]
pub struct UnionDetail {
    pub name: Name,
    pub description: Option<String>,
    pub members: Vec<Name>,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_union_detail(&self, type_name: &Name, u: &UnionType) -> UnionDetail {
        let description = u.description.as_ref().map(|d| d.to_string());
        let members = u.members.iter().map(|m| m.name.clone()).collect();
        let via = self.find_root_paths(type_name);
        UnionDetail { name: type_name.clone(), description, members, via }
    }
}
