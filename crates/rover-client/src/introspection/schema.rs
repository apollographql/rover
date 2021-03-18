//! Schema code generation module used to work with Introspection result.
use crate::query::graph::introspect;
use sdl_encoder::{
    Directive, EnumDef, EnumValue, Field, FieldArgument, FieldValue, InputDef, Interface,
    ObjectDef, ScalarDef, Schema as SDL, Union,
};
use serde::Deserialize;
use std::convert::TryFrom;

pub type Introspection = introspect::introspection_query::ResponseData;
pub type SchemaTypes = introspect::introspection_query::IntrospectionQuerySchemaTypes;
pub type SchemaDirectives = introspect::introspection_query::IntrospectionQuerySchemaDirectives;
pub type FullTypeFields = introspect::introspection_query::FullTypeFields;
pub type FullTypeFieldArgs = introspect::introspection_query::FullTypeFieldsArgs;
pub type __TypeKind = introspect::introspection_query::__TypeKind;

const GRAPHQL_NAMED_TYPES: [&str; 8] = [
    "__Schema",
    "__Type",
    "__TypeKind",
    "__Field",
    "__InputValue",
    "__EnumValue",
    "__DirectiveLocation",
    "__Directive",
];

const GRAPHQL_DIRECTIVES: [&str; 3] = ["skip", "include", "deprecated"];

/// A representation of a GraphQL Schema.
///
/// Contains schema Types and Directives.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    types: Vec<SchemaTypes>,
    directives: Vec<SchemaDirectives>,
}

impl Schema {
    /// Encode Schema into an SDL.
    pub fn encode(self) -> String {
        let mut sdl = SDL::new();

        // Exclude GraphQL directives like 'skip' and 'include' before encoding directives.
        self.directives
            .into_iter()
            .filter(|directive| !GRAPHQL_DIRECTIVES.contains(&directive.name.as_str()))
            .for_each(|directive| Self::encode_directives(directive, &mut sdl));

        // Exclude GraphQL named types like __Schema before encoding full type.
        self.types
            .into_iter()
            .filter(|type_| match type_.full_type.name.as_deref() {
                Some(name) => !GRAPHQL_NAMED_TYPES.contains(&name),
                None => false,
            })
            .for_each(|type_| Self::encode_full_type(type_, &mut sdl));

        sdl.finish()
    }

    fn encode_directives(directive: SchemaDirectives, sdl: &mut SDL) {
        let mut directive_ = Directive::new(directive.name);
        directive_.description(directive.description);
        for location in directive.locations {
            // Location is of a __DirectiveLocation enum that doesn't implement
            // Display (meaning we can't just do .to_string). This next line
            // just forces it into a String with format! debug.
            directive_.location(format!("{:?}", location));
        }

        sdl.directive(directive_)
    }

    fn encode_full_type(type_: SchemaTypes, sdl: &mut SDL) {
        let ty = type_.full_type;

        match ty.kind {
            __TypeKind::OBJECT => {
                let mut object_def = ObjectDef::new(ty.name.unwrap_or_else(String::new));
                object_def.description(ty.description);
                if let Some(interfaces) = ty.interfaces {
                    for interface in interfaces {
                        object_def.interface(interface.type_ref.name.unwrap_or_else(String::new));
                    }
                }
                if let Some(field) = ty.fields {
                    for f in field {
                        let field_def = Self::encode_field(f);
                        object_def.field(field_def);
                    }
                    sdl.object(object_def);
                }
            }
            __TypeKind::INPUT_OBJECT => {
                let mut input_def = InputDef::new(ty.name.unwrap_or_else(String::new));
                input_def.description(ty.description);
                if let Some(interfaces) = ty.interfaces {
                    for interface in interfaces {
                        input_def.interface(interface.type_ref.name.unwrap_or_else(String::new));
                    }
                }
                if let Some(field) = ty.fields {
                    for f in field {
                        let field_def = Self::encode_field(f);
                        input_def.field(field_def);
                    }
                    sdl.input(input_def);
                }
            }
            __TypeKind::INTERFACE => {
                let mut interface_def = Interface::new(ty.name.unwrap_or_else(String::new));
                interface_def.description(ty.description);
                if let Some(interfaces) = ty.interfaces {
                    for interface in interfaces {
                        interface_def
                            .interface(interface.type_ref.name.unwrap_or_else(String::new));
                    }
                }
                if let Some(field) = ty.fields {
                    for f in field {
                        let field_def = Self::encode_field(f);
                        interface_def.field(field_def);
                    }
                    sdl.interface(interface_def);
                }
            }
            __TypeKind::SCALAR => {
                let mut scalar_def = ScalarDef::new(ty.name.unwrap_or_else(String::new));
                scalar_def.description(ty.description);
                sdl.scalar(scalar_def);
            }
            __TypeKind::UNION => {
                let mut union_def = Union::new(ty.name.unwrap_or_else(String::new));
                union_def.description(ty.description);
                if let Some(possible_types) = ty.possible_types {
                    for possible_type in possible_types {
                        union_def.member(possible_type.type_ref.name.unwrap_or_else(String::new));
                    }
                }
                sdl.union(union_def);
            }
            __TypeKind::ENUM => {
                let mut enum_def = EnumDef::new(ty.name.unwrap_or_else(String::new));
                if let Some(enums) = ty.enum_values {
                    for enum_ in enums {
                        let mut enum_value = EnumValue::new(enum_.name);
                        enum_value.description(enum_.description);

                        if enum_.is_deprecated {
                            enum_value.deprecated(enum_.deprecation_reason);
                        }

                        enum_def.value(enum_value);
                    }
                }
                sdl.enum_(enum_def);
            }
            _ => (),
        }
    }

    fn encode_field(field: FullTypeFields) -> Field {
        let ty = Self::encode_type(field.type_.type_ref);
        let mut field_def = Field::new(field.name, ty);

        for arg in field.args {
            let field_arg = Self::encode_arg(arg);
            field_def.arg(field_arg);
        }

        if field.is_deprecated {
            field_def.deprecated(field.deprecation_reason);
        }
        field_def.description(field.description);
        field_def
    }

    fn encode_arg(arg: FullTypeFieldArgs) -> FieldArgument {
        let ty = Self::encode_type(arg.input_value.type_.type_ref);
        let mut arg_def = FieldArgument::new(arg.input_value.name, ty);

        arg_def.description(arg.input_value.description);
        arg_def
    }

    fn encode_type(ty: impl introspect::OfType) -> FieldValue {
        use introspect::introspection_query::__TypeKind::*;
        match ty.kind() {
            SCALAR | OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT => FieldValue::Type {
                ty: ty.name().unwrap().to_string(),
                default: None,
            },
            NON_NULL => {
                let ty = Self::encode_type(ty.of_type().unwrap());
                FieldValue::NonNull { ty: Box::new(ty) }
            }
            LIST => {
                let ty = Self::encode_type(ty.of_type().unwrap());
                FieldValue::List { ty: Box::new(ty) }
            }
            Other(ty) => panic!("Unknown type: {}", ty),
        }
    }
}

impl TryFrom<Introspection> for Schema {
    type Error = &'static str;

    fn try_from(src: Introspection) -> Result<Self, Self::Error> {
        match src.schema {
            Some(s) => Ok(Self {
                types: s.types,
                directives: s.directives,
            }),
            None => Err("Schema not found in Introspection Result."),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_build_simple_schema() {}
}
