use apollo_compiler::{
    Name,
    ast::Type as AstType,
    coordinate::{SchemaLookupError, TypeAttributeCoordinate},
};

use crate::{ParsedSchema, SchemaError, describe::deprecated::IsDeprecated, root_paths::RootPath};

use super::arg_info::ArgInfo;
use super::expanded_type::ExpandedType;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldDetail {
    pub type_name: Name,
    pub field_name: Name,
    pub return_type: AstType,
    pub description: Option<String>,
    pub arg_count: usize,
    pub args: Vec<ArgInfo>,
    pub via: Vec<RootPath>,
    pub input_expansions: Vec<ExpandedType>,
    pub return_expansion: Option<ExpandedType>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
}

impl ParsedSchema {
    pub fn field_detail(
        &self,
        coord: &TypeAttributeCoordinate,
    ) -> Result<FieldDetail, SchemaError> {
        let field = coord.lookup_field(self.inner()).map_err(|e| match e {
            SchemaLookupError::MissingType(name) => SchemaError::TypeNotFound(name.clone()),
            _ => SchemaError::FieldNotFound {
                type_name: coord.ty.clone(),
                field: coord.attribute.clone(),
            },
        })?;

        let type_name = coord.ty.clone();
        let field_name = coord.attribute.clone();
        let return_type = field.ty.clone();
        let description = field.description.as_ref().map(|d| d.to_string());
        let is_deprecated = field.is_deprecated();
        let deprecation_reason = field.deprecation_reason();

        let args: Vec<ArgInfo> = field
            .arguments
            .iter()
            .map(|arg| ArgInfo {
                name: arg.name.clone(),
                arg_type: arg.ty.inner_named_type().clone(),
                description: arg.description.as_ref().map(|d| d.to_string()),
                default_value: arg.default_value.as_ref().map(|v| v.to_string()),
            })
            .collect();

        let via = self.find_root_paths(&type_name);

        let input_expansions = args
            .iter()
            .filter_map(|arg| self.expand_single_type(arg.arg_type.as_str(), true))
            .filter(|expanded| matches!(expanded, ExpandedType::Input { .. }))
            .collect();

        let return_expansion = self.expand_single_type(field.ty.inner_named_type().as_str(), true);

        Ok(FieldDetail {
            type_name,
            field_name,
            return_type,
            description,
            arg_count: args.len(),
            args,
            via,
            input_expansions,
            return_expansion,
            is_deprecated,
            deprecation_reason,
        })
    }
}
