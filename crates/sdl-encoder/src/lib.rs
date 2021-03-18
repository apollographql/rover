//! SDL Encoder provides methods to serialise a GraphQL Schema.
//! For mor information on GraphQL Schema Types, please refer to [official
//! documentation](https://graphql.org/learn/schema/).
//!
//! ## Example
//! ```rust
//! use sdl_encoder::{Schema, ObjectDef, SchemaDef, Field, InputDef, ScalarDef, EnumDef, FieldValue};
//! use indoc::indoc;
//!
//! let mut schema = Schema::new();

//! // create a field
//! let ty = FieldValue::Type {
//!     ty: "String".to_string(),
//!     default: None,
//! };
//! let ty_2 = FieldValue::NonNull { ty: Box::new(ty) };
//! let mut field = Field::new("cat".to_string(), ty_2);
//! field.description(Some("Very good cats".to_string()));

//! // a schema definition
//! let mut schema_def = SchemaDef::new(field.clone());
//! schema_def.description(Some("Simple schema".to_string()));
//! schema.schema(schema_def);

//! // object type defintion
//! let mut object_def = ObjectDef::new("Query".to_string());
//! object_def.description(Some("Example Query type".to_string()));
//! object_def.field(field.clone());
//! schema.object(object_def);

//! let mut scalar = ScalarDef::new("NoriCacheControl".to_string());
//! scalar.description(Some("Scalar description".to_string()));
//! schema.scalar(scalar);

//! // input definition
//! let mut input_def = InputDef::new("SpaceCat".to_string());
//! input_def.field(field);
//! schema.input(input_def);

//! assert_eq!(
//!     schema.finish(),
//!     indoc! { r#"
//!         """
//!         Simple schema
//!         """
//!         schema {
//!           """
//!           Very good cats
//!           """
//!           cat: String!
//!         }
//!         """
//!         Example Query type
//!         """
//!         type Query {
//!           """
//!           Very good cats
//!           """
//!           cat: String!
//!         }
//!         """
//!         Scalar description
//!         """
//!         scalar NoriCacheControl
//!         input SpaceCat {
//!           """
//!           Very good cats
//!           """
//!           cat: String!
//!         }
//!     "# }
//! );
//! ```

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations, nonstandard_style)]
#![warn(missing_docs, future_incompatible, unreachable_pub, rust_2018_idioms)]

mod field;
pub use field::Field;

mod field_value;
pub use field_value::FieldValue;

mod field_argument;
pub use field_argument::FieldArgument;

mod enum_def;
pub use enum_def::EnumDef;

mod enum_value;
pub use enum_value::EnumValue;

mod object_def;
pub use object_def::ObjectDef;

mod scalar_def;
pub use scalar_def::ScalarDef;

mod union_def;
pub use union_def::Union;

mod directive_def;
pub use directive_def::Directive;

mod input_def;
pub use input_def::InputDef;

mod interface_def;
pub use interface_def::Interface;

mod schema_def;
pub use schema_def::SchemaDef;

/// An SDL representation of a GraphQLSchema.
#[derive(Debug)]
pub struct Schema {
    buf: String,
}

impl Schema {
    /// Creates a new instance of Schema.
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    /// Adds a new Directive Definition.
    pub fn directive(&mut self, directive: Directive) {
        self.buf.push_str(&directive.to_string());
    }

    /// Adds a new Type Definition.
    pub fn object(&mut self, object: ObjectDef) {
        self.buf.push_str(&object.to_string());
    }

    /// Adds a new Schema Definition.
    ///
    /// The schema type is only used when the root GraphQL type is different
    /// from default GraphQL types.
    pub fn schema(&mut self, schema: SchemaDef) {
        self.buf.push_str(&schema.to_string());
    }

    /// Adds a new Input object definition
    pub fn input(&mut self, input: InputDef) {
        self.buf.push_str(&input.to_string());
    }

    /// Adds a new Enum type definition
    pub fn enum_(&mut self, enum_: EnumDef) {
        self.buf.push_str(&enum_.to_string());
    }

    /// Adds a new Scalar type definition
    pub fn scalar(&mut self, scalar: ScalarDef) {
        self.buf.push_str(&scalar.to_string());
    }

    /// Adds a new Union type definition
    pub fn union(&mut self, union_: Union) {
        self.buf.push_str(&union_.to_string());
    }

    /// Adds a new Interface type definition
    pub fn interface(&mut self, interface: Interface) {
        self.buf.push_str(&interface.to_string());
    }

    /// Return the encoded SDL string after all types have been
    pub fn finish(self) -> String {
        self.buf
    }
}

impl Default for Schema {
    fn default() -> Self {
        Schema::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn smoke_test() {
        let mut schema = Schema::new();

        // create a directive
        let mut directive = Directive::new("infer".to_string());
        directive.description(Some("Infer field types from field values.".to_string()));
        directive.location("OBJECT".to_string());
        directive.location("FIELD_DEFINITION".to_string());
        directive.location("INPUT_FIELD_DEFINITION".to_string());
        schema.directive(directive);

        // create a field
        let field_value = FieldValue::Type {
            ty: "String".to_string(),
            default: None,
        };

        let null_field = FieldValue::NonNull {
            ty: Box::new(field_value),
        };

        let mut field = Field::new("cat".to_string(), null_field);
        field.description(Some("Very good cats".to_string()));

        // a schema definition
        let mut schema_def = SchemaDef::new(field.clone());
        schema_def.description(Some("Simple schema".to_string()));
        schema.schema(schema_def);

        // object type defintion
        let mut object_def = ObjectDef::new("Query".to_string());
        object_def.description(Some("Example Query type".to_string()));
        object_def.field(field.clone());
        object_def.interface("Find".to_string());
        object_def.interface("Sort".to_string());
        schema.object(object_def);

        // enum definition
        let mut enum_ty_1 = EnumValue::new("CatTree".to_string());
        enum_ty_1.description(Some("Top bunk of a cat tree.".to_string()));
        let enum_ty_2 = EnumValue::new("Bed".to_string());
        let mut enum_ty_3 = EnumValue::new("CardboardBox".to_string());
        enum_ty_3.deprecated(Some("Box was recycled.".to_string()));

        let mut enum_def = EnumDef::new("NapSpots".to_string());
        enum_def.description(Some("Favourite cat nap spots.".to_string()));
        enum_def.value(enum_ty_1);
        enum_def.value(enum_ty_2);
        enum_def.value(enum_ty_3);
        schema.enum_(enum_def);

        let mut scalar = ScalarDef::new("NoriCacheControl".to_string());
        scalar.description(Some("Scalar description".to_string()));
        schema.scalar(scalar);

        let mut union_def = Union::new("Cat".to_string());
        union_def.description(Some(
            "A union of all cats represented within a household.".to_string(),
        ));
        union_def.member("NORI".to_string());
        union_def.member("CHASHU".to_string());
        schema.union(union_def);

        // input definition
        let mut input_def = InputDef::new("SpaceCat".to_string());
        input_def.field(field);
        schema.input(input_def);

        assert_eq!(
            schema.finish(),
            indoc! { r#"
                """
                Infer field types from field values.
                """
                directive @infer on OBJECT | FIELD_DEFINITION | INPUT_FIELD_DEFINITION
                """
                Simple schema
                """
                schema {
                  """
                  Very good cats
                  """
                  cat: String!
                }
                """
                Example Query type
                """
                type Query implements Find & Sort {
                  """
                  Very good cats
                  """
                  cat: String!
                }
                """
                Favourite cat nap spots.
                """
                enum NapSpots {
                  """
                  Top bunk of a cat tree.
                  """
                  CatTree
                  Bed
                  CardboardBox @deprecated(reason: "Box was recycled.")
                }
                """
                Scalar description
                """
                scalar NoriCacheControl
                """
                A union of all cats represented within a household.
                """
                union Cat = NORI | CHASHU
                input SpaceCat {
                  """
                  Very good cats
                  """
                  cat: String!
                }
            "# }
        );
    }

    #[test]
    fn smoke_test_2() {
        let mut schema = Schema::new();

        let ty_1 = FieldValue::Type {
            ty: "SpaceCatEnum".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::List {
            ty: Box::new(ty_1.clone()),
        };

        let ty_3 = FieldValue::NonNull { ty: Box::new(ty_2) };

        let object_field = Field::new("cat".to_string(), ty_3);
        let mut object_def = ObjectDef::new("Query".to_string());
        object_def.description(Some("Example Query type".to_string()));
        object_def.field(object_field);

        let mut schema_field = Field::new("treat".to_string(), ty_1);
        schema_field.description(Some("Good cats get treats".to_string()));
        let mut schema_def = SchemaDef::new(schema_field);
        schema_def.description(Some("Example schema Def".to_string()));
        schema.schema(schema_def);
        schema.object(object_def);
        assert_eq!(
            schema.finish(),
            indoc! { r#"
                """
                Example schema Def
                """
                schema {
                  """
                  Good cats get treats
                  """
                  treat: SpaceCatEnum
                }
                """
                Example Query type
                """
                type Query {
                  cat: [SpaceCatEnum]!
                }
        "#}
        )
    }
}
