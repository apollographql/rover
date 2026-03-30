use apollo_compiler::{Name, schema::UnionType};

use crate::{ParsedSchema, root_paths::RootPath};

/// Detailed view of a GraphQL union type.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnionDetail {
    /// The union type name.
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// The object types that are members of this union.
    pub members: Vec<Name>,
    /// Root paths from Query/Mutation to this type.
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_union_detail(&self, type_name: &Name, u: &UnionType) -> UnionDetail {
        let description = u.description.as_ref().map(|d| d.to_string());
        let members = u.members.iter().map(|m| m.name.clone()).collect();
        let via = self.find_root_paths(type_name);
        UnionDetail {
            name: type_name.clone(),
            description,
            members,
            via,
        }
    }
}
