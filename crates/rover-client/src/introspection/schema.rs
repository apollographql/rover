//! Schema encoding module used to work with Introspection result.
//!
//! More information on Schema Definition language(SDL) can be found in [this
//! documentation](https://www.apollographql.com/docs/apollo-server/schema/schema/).
//!
use crate::query::graph::introspect;
use sdl_encoder::{
    Directive, EnumDef, EnumValue, Field, InputField, InputObjectDef, InputValue, InterfaceDef,
    ObjectDef, ScalarDef, Schema as SDL, SchemaDef, Type_, UnionDef,
};
use serde::Deserialize;
use std::convert::TryFrom;

pub type FullTypeField = introspect::introspection_query::FullTypeFields;
pub type FullTypeInputField = introspect::introspection_query::FullTypeInputFields;
pub type FullTypeFieldArg = introspect::introspection_query::FullTypeFieldsArgs;
pub type IntrospectionResult = introspect::introspection_query::ResponseData;
pub type SchemaMutationType = introspect::introspection_query::IntrospectionQuerySchemaMutationType;
pub type SchemaQueryType = introspect::introspection_query::IntrospectionQuerySchemaQueryType;
pub type SchemaType = introspect::introspection_query::IntrospectionQuerySchemaTypes;
pub type SchemaDirective = introspect::introspection_query::IntrospectionQuerySchemaDirectives;
pub type SchemaSubscriptionType =
    introspect::introspection_query::IntrospectionQuerySchemaSubscriptionType;
pub type __TypeKind = introspect::introspection_query::__TypeKind;

// Represents GraphQL types we will not be encoding to SDL.
const GRAPHQL_NAMED_TYPES: [&str; 12] = [
    "__Schema",
    "__Type",
    "__TypeKind",
    "__Field",
    "__InputValue",
    "__EnumValue",
    "__DirectiveLocation",
    "__Directive",
    "Boolean",
    "String",
    "Int",
    "ID",
];

// Represents GraphQL directives we will not be encoding to SDL.
const SPECIFIED_DIRECTIVES: [&str; 3] = ["skip", "include", "deprecated"];

/// A representation of a GraphQL Schema.
///
/// Contains Schema Types and Directives.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    types: Vec<SchemaType>,
    directives: Vec<SchemaDirective>,
    mutation_type: Option<SchemaMutationType>,
    query_type: SchemaQueryType,
    subscription_type: Option<SchemaSubscriptionType>,
}

impl Schema {
    /// Encode Schema into an SDL.
    pub fn encode(self) -> String {
        let mut sdl = SDL::new();

        // When we have a defined mutation and subscription, we record
        // everything to Schema Definition.
        // https://www.apollographql.com/docs/graphql-subscriptions/subscriptions-to-schema/
        if self.mutation_type.is_some() | self.subscription_type.is_some() {
            let mut schema_def = SchemaDef::new();
            if let Some(mutation_type) = self.mutation_type {
                schema_def.mutation(mutation_type.name.unwrap());
            }
            if let Some(subscription_type) = self.subscription_type {
                schema_def.subscription(subscription_type.name.unwrap());
            }
            if let Some(name) = self.query_type.name {
                schema_def.query(name);
            }
            sdl.schema(schema_def);
        } else if let Some(name) = self.query_type.name {
            // If we don't have a mutation or a subscription, but do have a
            // query type, only create a Schema Definition when it's something
            // other than `Query`.
            if name != "Query" {
                let mut schema_def = SchemaDef::new();
                schema_def.query(name);
                sdl.schema(schema_def);
            }
        }

        // Exclude GraphQL directives like 'skip' and 'include' before encoding directives.
        self.directives
            .into_iter()
            .filter(|directive| !SPECIFIED_DIRECTIVES.contains(&directive.name.as_str()))
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

    fn encode_directives(directive: SchemaDirective, sdl: &mut SDL) {
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

    fn encode_full_type(type_: SchemaType, sdl: &mut SDL) {
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
                let mut input_def = InputObjectDef::new(ty.name.unwrap_or_else(String::new));
                input_def.description(ty.description);
                if let Some(field) = ty.input_fields {
                    for f in field {
                        let input_field_def = Self::encode_input_field(f);
                        input_def.field(input_field_def);
                    }
                    sdl.input(input_def);
                }
            }
            __TypeKind::INTERFACE => {
                let mut interface_def = InterfaceDef::new(ty.name.unwrap_or_else(String::new));
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
                let mut union_def = UnionDef::new(ty.name.unwrap_or_else(String::new));
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

    fn encode_field(field: FullTypeField) -> Field {
        let ty = Self::encode_type(field.type_.type_ref);
        let mut field_def = Field::new(field.name, ty);

        for value in field.args {
            let field_value = Self::encode_arg(value);
            field_def.arg(field_value);
        }

        if field.is_deprecated {
            field_def.deprecated(field.deprecation_reason);
        }
        field_def.description(field.description);
        field_def
    }

    fn encode_input_field(field: FullTypeInputField) -> InputField {
        let ty = Self::encode_type(field.input_value.type_.type_ref);
        let mut field_def = InputField::new(field.input_value.name, ty);

        field_def.default(field.input_value.default_value);
        field_def.description(field.input_value.description);
        field_def
    }

    fn encode_arg(value: FullTypeFieldArg) -> InputValue {
        let ty = Self::encode_type(value.input_value.type_.type_ref);
        let mut value_def = InputValue::new(value.input_value.name, ty);

        value_def.default(value.input_value.default_value);
        value_def.description(value.input_value.description);
        value_def
    }

    fn encode_type(ty: impl introspect::OfType) -> Type_ {
        use introspect::introspection_query::__TypeKind::*;
        match ty.kind() {
            SCALAR | OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT => Type_::NamedType {
                name: ty.name().unwrap().to_string(),
            },
            NON_NULL => {
                let ty = Self::encode_type(ty.of_type().unwrap());
                Type_::NonNull { ty: Box::new(ty) }
            }
            LIST => {
                let ty = Self::encode_type(ty.of_type().unwrap());
                Type_::List { ty: Box::new(ty) }
            }
            Other(ty) => panic!("Unknown type: {}", ty),
        }
    }
}

impl TryFrom<IntrospectionResult> for Schema {
    type Error = &'static str;

    fn try_from(src: IntrospectionResult) -> Result<Self, Self::Error> {
        match src.schema {
            Some(s) => Ok(Self {
                types: s.types,
                directives: s.directives,
                mutation_type: s.mutation_type,
                query_type: s.query_type,
                subscription_type: s.subscription_type,
            }),
            None => Err("Schema not found in Introspection Result."),
        }
    }
}
