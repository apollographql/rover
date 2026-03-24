use apollo_compiler::{Name, schema::ExtendedType};

use super::{
    deprecated::{DeprecatedFields, DeprecatedValues},
    type_detail::FieldSummary,
};
use crate::ParsedSchema;

/// High-level statistics and type inventory for a GraphQL schema.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaOverview {
    /// The SDL source location or identifier that was parsed.
    pub schema_source: String,
    /// Total number of user-defined types (excluding built-ins).
    pub total_types: usize,
    /// Total number of fields across all objects, interfaces, inputs, and root types.
    pub total_fields: usize,
    /// Number of deprecated fields and enum values across the schema.
    pub total_deprecated: usize,
    /// Fields exposed on the `Query` root type.
    pub query_fields: Vec<FieldSummary>,
    /// Fields exposed on the `Mutation` root type.
    pub mutation_fields: Vec<FieldSummary>,
    /// Names of all user-defined object types (root types excluded).
    pub objects: Vec<Name>,
    /// Names of all input object types.
    pub inputs: Vec<Name>,
    /// Names of all enum types.
    pub enums: Vec<Name>,
    /// Names of all interface types.
    pub interfaces: Vec<Name>,
    /// Names of all union types.
    pub unions: Vec<Name>,
    /// Names of all custom scalar types.
    pub scalars: Vec<Name>,
}

impl ParsedSchema {
    /// Generate a schema overview.
    pub fn overview(&self, schema_source: String) -> SchemaOverview {
        let schema = self.inner();
        let mut total_fields = 0usize;
        let mut total_deprecated = 0usize;
        let mut objects = Vec::new();
        let mut inputs = Vec::new();
        let mut enums = Vec::new();
        let mut interfaces = Vec::new();
        let mut unions = Vec::new();
        let mut scalars = Vec::new();

        let root_types: std::collections::HashSet<_> = schema
            .schema_definition
            .iter_root_operations()
            .map(|(_, name)| name.name.clone())
            .collect();

        for (name, ty) in &schema.types {
            if ty.is_built_in() {
                continue;
            }
            match ty {
                ExtendedType::Object(obj) => {
                    total_fields += obj.fields.len();
                    total_deprecated += obj.deprecated_fields().len();
                    if !root_types.contains(name) {
                        objects.push(name.clone());
                    }
                }
                ExtendedType::InputObject(inp) => {
                    total_fields += inp.fields.len();
                    inputs.push(name.clone());
                }
                ExtendedType::Enum(e) => {
                    total_deprecated += e.deprecated_values().len();
                    enums.push(name.clone());
                }
                ExtendedType::Interface(interface) => {
                    total_fields += interface.fields.len();
                    total_deprecated += interface.deprecated_fields().len();
                    interfaces.push(name.clone());
                }
                ExtendedType::Union(_) => {
                    unions.push(name.clone());
                }
                ExtendedType::Scalar(_) => {
                    scalars.push(name.clone());
                }
            }
        }

        objects.sort();
        inputs.sort();
        enums.sort();
        interfaces.sort();
        unions.sort();
        scalars.sort();

        let root_types_count = schema.schema_definition.iter_root_operations().count();

        let user_types = objects.len()
            + inputs.len()
            + enums.len()
            + interfaces.len()
            + unions.len()
            + scalars.len()
            + root_types_count;

        let query_fields = FieldSummary::new(schema, "Query");
        let mutation_fields = FieldSummary::new(schema, "Mutation");

        total_fields += query_fields.len() + mutation_fields.len();

        SchemaOverview {
            schema_source,
            total_types: user_types,
            total_fields,
            total_deprecated,
            query_fields,
            mutation_fields,
            objects,
            inputs,
            enums,
            interfaces,
            unions,
            scalars,
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::name;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;
    use crate::ParsedSchema;

    #[fixture]
    fn test_schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl)
    }

    #[rstest]
    fn overview_type_counts(test_schema: ParsedSchema) {
        let schema_overview = test_schema.overview("test_schema.graphql".to_string());
        assert_that!(schema_overview.total_types).is_equal_to(29);
        assert_that!(schema_overview.total_fields).is_equal_to(86);
        assert_that!(schema_overview.total_deprecated).is_equal_to(3);
        assert_that!(schema_overview.query_fields).has_length(5);
        assert_that!(schema_overview.mutation_fields).has_length(3);
        assert_that!(schema_overview.objects).has_length(16);
        assert_that!(schema_overview.enums).has_length(3);
        assert_that!(schema_overview.interfaces).has_length(3);
        assert_that!(schema_overview.inputs).has_length(2);
    }

    #[rstest]
    #[case::string(name!("String"))]
    #[case::int(name!("Int"))]
    #[case::float(name!("Float"))]
    #[case::boolean(name!("Boolean"))]
    #[case::id(name!("ID"))]
    fn overview_excludes_builtin_scalars(test_schema: ParsedSchema, #[case] scalar: Name) {
        let schema_overview = test_schema.overview("test_schema.graphql".to_string());
        assert_that!(schema_overview.scalars).does_not_contain(&scalar);
    }

    #[rstest]
    #[case::schema(name!("__Schema"))]
    #[case::type_(name!("__Type"))]
    #[case::field(name!("__Field"))]
    #[case::input_value(name!("__InputValue"))]
    #[case::enum_value(name!("__EnumValue"))]
    #[case::directive(name!("__Directive"))]
    #[case::directive_location(name!("__DirectiveLocation"))]
    fn overview_excludes_introspection_types(test_schema: ParsedSchema, #[case] type_name: Name) {
        let schema_overview = test_schema.overview("test_schema.graphql".to_string());
        let all_names: Vec<&Name> = schema_overview
            .objects
            .iter()
            .chain(schema_overview.scalars.iter())
            .chain(schema_overview.enums.iter())
            .chain(schema_overview.interfaces.iter())
            .chain(schema_overview.unions.iter())
            .chain(schema_overview.inputs.iter())
            .collect();
        assert_that!(all_names).does_not_contain(&type_name);
    }

    #[rstest]
    #[case::query(name!("Query"))]
    #[case::mutation(name!("Mutation"))]
    fn overview_excludes_root_types_from_objects(
        test_schema: ParsedSchema,
        #[case] root_type: Name,
    ) {
        let schema_overview = test_schema.overview("test_schema.graphql".to_string());
        assert_that!(schema_overview.objects).does_not_contain(&root_type);
    }
}
