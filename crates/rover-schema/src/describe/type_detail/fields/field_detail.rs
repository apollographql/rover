use apollo_compiler::{Name, ast::Type as AstType, coordinate::SchemaCoordinate, schema::ExtendedType};

use crate::{ParsedSchema, SchemaError, describe::deprecated::IsDeprecated, root_paths::RootPath};

use super::arg_info::ArgInfo;
use super::expanded_type::ExpandedType;
use super::type_kind::TypeKind;

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
    pub fn field_detail(&self, coord: &SchemaCoordinate) -> Result<FieldDetail, SchemaError> {
        let (type_name, field_name) = match coord {
            SchemaCoordinate::TypeAttribute(tac) => (tac.ty.clone(), tac.attribute.clone()),
            _ => {
                return Err(SchemaError::InvalidCoordinate(coord.clone()));
            }
        };

        let ty = self
            .inner()
            .types
            .get(type_name.as_str())
            .ok_or_else(|| SchemaError::TypeNotFound(type_name.clone()))?;

        let field = match ty {
            ExtendedType::Object(obj) => obj.fields.get(field_name.as_str()),
            ExtendedType::Interface(iface) => iface.fields.get(field_name.as_str()),
            _ => None,
        }
        .ok_or_else(|| SchemaError::FieldNotFound {
            type_name: type_name.clone(),
            field: field_name.clone(),
        })?;

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

        let mut input_expansions = Vec::new();
        for arg in &args {
            if let Some(expanded) = self.expand_single_type(arg.arg_type.as_str(), true)
                && expanded.kind == TypeKind::Input
            {
                input_expansions.push(expanded);
            }
        }

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
