//! Schema encoding module used to work with Introspection result.
//!
//! More information on Schema Definition language(SDL) can be found in [this
//! documentation](https://www.apollographql.com/docs/apollo-server/schema/schema/).
//!
use std::convert::TryFrom;

use apollo_encoder::{
    Argument, Directive, DirectiveDefinition, Document as SDL, EnumDefinition, EnumValue,
    FieldDefinition, InputField, InputObjectDefinition, InputValueDefinition, InterfaceDefinition,
    ObjectDefinition, ScalarDefinition, SchemaDefinition, Type_, UnionDefinition, Value,
};
use serde::Deserialize;

use crate::operations::graph::introspect::runner::graph_introspect_query;

type FullTypeField = graph_introspect_query::FullTypeFields;
type FullTypeInputField = graph_introspect_query::FullTypeInputFields;
type FullTypeFieldArg = graph_introspect_query::FullTypeFieldsArgs;
type IntrospectionResult = graph_introspect_query::ResponseData;
type SchemaMutationType = graph_introspect_query::GraphIntrospectQuerySchemaMutationType;
type SchemaQueryType = graph_introspect_query::GraphIntrospectQuerySchemaQueryType;
type SchemaType = graph_introspect_query::GraphIntrospectQuerySchemaTypes;
type SchemaDirective = graph_introspect_query::GraphIntrospectQuerySchemaDirectives;
type SchemaSubscriptionType = graph_introspect_query::GraphIntrospectQuerySchemaSubscriptionType;
type __TypeKind = graph_introspect_query::__TypeKind;

// Represents GraphQL types we will not be encoding to SDL.
const GRAPHQL_NAMED_TYPES: [&str; 13] = [
    "__Schema",
    "__Type",
    "__TypeKind",
    "__Field",
    "__InputValue",
    "__EnumValue",
    "__DirectiveLocation",
    "__Directive",
    "Boolean",
    "Float",
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
            let mut schema_def = SchemaDefinition::new();
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
                let mut schema_def = SchemaDefinition::new();
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
            .filter(|type_| match type_.name.as_deref() {
                Some(name) => !GRAPHQL_NAMED_TYPES.contains(&name),
                None => false,
            })
            .for_each(|type_| Self::encode_full_type(type_, &mut sdl));

        sdl.to_string()
    }

    fn encode_directives(directive: SchemaDirective, sdl: &mut SDL) {
        let mut directive_ = DirectiveDefinition::new(directive.name);
        if let Some(desc) = directive.description {
            directive_.description(desc);
        }
        for arg in directive.args {
            let input_value = Self::encode_arg(arg);
            directive_.arg(input_value);
        }

        for location in directive.locations {
            // Location is of a __DirectiveLocation enum that doesn't implement
            // Display (meaning we can't just do .to_string). This next line
            // just forces it into a String with format! debug.
            directive_.location(format!("{location:?}"));
        }

        sdl.directive(directive_)
    }

    fn encode_full_type(type_: SchemaType, sdl: &mut SDL) {
        match type_.kind {
            __TypeKind::OBJECT => {
                let mut object_def = ObjectDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    object_def.description(desc);
                }
                if let Some(interfaces) = type_.interfaces {
                    for interface in interfaces {
                        object_def.interface(interface.name.unwrap_or_default());
                    }
                }
                if let Some(field) = type_.fields {
                    for f in field {
                        let field_def = Self::encode_field(f);
                        object_def.field(field_def);
                    }
                    sdl.object(object_def);
                }
            }
            __TypeKind::INPUT_OBJECT => {
                let mut input_def = InputObjectDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    input_def.description(desc);
                }
                if let Some(field) = type_.input_fields {
                    for f in field {
                        let input_field_def = Self::encode_input_field(f);
                        input_def.field(input_field_def);
                    }
                    sdl.input_object(input_def);
                }
            }
            __TypeKind::INTERFACE => {
                let mut interface_def = InterfaceDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    interface_def.description(desc);
                }
                if let Some(interfaces) = type_.interfaces {
                    for interface in interfaces {
                        interface_def.interface(interface.name.unwrap_or_default());
                    }
                }
                if let Some(field) = type_.fields {
                    for f in field {
                        let field_def = Self::encode_field(f);
                        interface_def.field(field_def);
                    }
                    sdl.interface(interface_def);
                }
            }
            __TypeKind::SCALAR => {
                let mut scalar_def = ScalarDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    scalar_def.description(desc);
                }
                sdl.scalar(scalar_def);
            }
            __TypeKind::UNION => {
                let mut union_def = UnionDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    union_def.description(desc);
                }
                if let Some(possible_types) = type_.possible_types {
                    for possible_type in possible_types {
                        union_def.member(possible_type.name.unwrap_or_default());
                    }
                }
                sdl.union(union_def);
            }
            __TypeKind::ENUM => {
                let mut enum_def = EnumDefinition::new(type_.name.unwrap_or_default());
                if let Some(desc) = type_.description {
                    enum_def.description(desc);
                }
                if let Some(enums) = type_.enum_values {
                    for enum_ in enums {
                        let mut enum_value = EnumValue::new(enum_.name);
                        if let Some(desc) = enum_.description {
                            enum_value.description(desc);
                        }

                        if enum_.is_deprecated {
                            enum_value
                                .directive(create_deprecated_directive(enum_.deprecation_reason));
                        }

                        enum_def.value(enum_value);
                    }
                }
                sdl.enum_(enum_def);
            }
            _ => (),
        }
    }

    fn encode_field(field: FullTypeField) -> FieldDefinition {
        let ty = Self::encode_type(field.type_);
        let mut field_def = FieldDefinition::new(field.name, ty);

        for value in field.args {
            let field_value = Self::encode_arg(value);
            field_def.arg(field_value);
        }

        if field.is_deprecated {
            field_def.directive(create_deprecated_directive(field.deprecation_reason));
        }
        if let Some(desc) = field.description {
            field_def.description(desc);
        }
        field_def
    }

    fn encode_input_field(field: FullTypeInputField) -> InputField {
        let ty = Self::encode_type(field.type_);
        let mut field_def = InputField::new(field.name, ty);
        if let Some(default_value) = field.default_value {
            field_def.default_value(default_value);
        }
        if let Some(desc) = field.description {
            field_def.description(desc);
        }

        if field.is_deprecated {
            field_def.directive(create_deprecated_directive(field.deprecation_reason));
        }
        field_def
    }

    fn encode_arg(value: FullTypeFieldArg) -> InputValueDefinition {
        let ty = Self::encode_type(value.type_);
        let mut value_def = InputValueDefinition::new(value.name, ty);
        if let Some(default_value) = value.default_value {
            value_def.default_value(default_value);
        }
        if let Some(desc) = value.description {
            value_def.description(desc);
        }

        if value.is_deprecated {
            value_def.directive(create_deprecated_directive(value.deprecation_reason));
        }
        value_def
    }

    fn encode_type(ty: impl OfType) -> Type_ {
        use graph_introspect_query::__TypeKind::*;
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
            Other(ty) => unreachable!("Unknown type kind: {}", ty),
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

/// This trait is used to be able to iterate over ofType fields in
/// IntrospectionResponse.
pub trait OfType {
    type TypeRef: OfType;

    fn kind(&self) -> &__TypeKind;
    fn name(&self) -> Option<&str>;
    fn of_type(self) -> Option<Self::TypeRef>;
}

macro_rules! impl_of_type {
    ($target:ty, $assoc:ty) => {
        impl OfType for $target {
            type TypeRef = $assoc;

            fn kind(&self) -> &__TypeKind {
                &self.kind
            }

            fn name(&self) -> Option<&str> {
                self.name.as_deref()
            }

            fn of_type(self) -> Option<Self::TypeRef> {
                self.of_type
            }
        }
    };
}

impl_of_type!(
    graph_introspect_query::TypeRef,
    graph_introspect_query::TypeRefOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfType,
    graph_introspect_query::TypeRefOfTypeOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfTypeOfType,
    graph_introspect_query::TypeRefOfTypeOfTypeOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfTypeOfTypeOfType,
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfType,
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfType,
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfTypeOfType
);

impl_of_type!(
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfTypeOfType,
    graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfTypeOfTypeOfType
);

// NOTE(lrlna): This is a **hack**. This makes sure that the last possible
// generated ofType by graphql_client can return a None for of_type method.
impl OfType for graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfTypeOfTypeOfType {
    type TypeRef = graph_introspect_query::TypeRefOfTypeOfTypeOfTypeOfTypeOfTypeOfTypeOfType;

    fn kind(&self) -> &graph_introspect_query::__TypeKind {
        &self.kind
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn of_type(self) -> Option<Self::TypeRef> {
        None
    }
}

fn create_deprecated_directive(reason: Option<String>) -> Directive {
    let mut deprecated_directive = Directive::new(String::from("deprecated"));
    if let Some(reason) = reason {
        deprecated_directive.arg(Argument::new(String::from("reason"), Value::String(reason)));
    }

    deprecated_directive
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, fs::File};

    use graphql_client::Response;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::operations::graph::introspect::types::QueryResponseData;

    #[test]
    fn it_builds_simple_schema() {
        let file = File::open("src/operations/graph/introspect/fixtures/simple.json").unwrap();
        let res: Response<QueryResponseData> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
        "The `Upload` scalar type represents a file upload."
        scalar Upload
        type Query {
          "A simple type for getting started!"
          hello: String
          cats(cat: [String]! = ["Nori"]): [String]!
        }
        enum CacheControlScope {
          PUBLIC
          PRIVATE
        }
        input BooleanQueryOperatorInput {
          eq: Boolean
          ne: Boolean
          in: [Boolean]
          nin: [Boolean]
        }
        directive @cacheControl(maxAge: Int, scope: CacheControlScope) on FIELD_DEFINITION | OBJECT | INTERFACE
        "Exposes a URL that specifies the behaviour of this scalar."
        directive @specifiedBy(
            "The URL that specifies the behaviour of this scalar."
            url: String!
          ) on SCALAR
    "#}
        )
    }

    #[test]
    fn it_builds_swapi_schema() {
        let file = File::open("src/operations/graph/introspect/fixtures/swapi.json").unwrap();
        let res: Response<QueryResponseData> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
        schema {
          query: Root
        }
        type Root {
          allFilms(after: String, first: Int, before: String, last: Int): FilmsConnection
          film(id: ID, filmID: ID): Film
          allPeople(after: String, first: Int, before: String, last: Int): PeopleConnection
          person(id: ID, personID: ID): Person
          allPlanets(after: String, first: Int, before: String, last: Int): PlanetsConnection
          planet(id: ID, planetID: ID): Planet
          allSpecies(after: String, first: Int, before: String, last: Int): SpeciesConnection
          species(id: ID, speciesID: ID): Species
          allStarships(after: String, first: Int, before: String, last: Int): StarshipsConnection
          starship(id: ID, starshipID: ID): Starship
          allVehicles(after: String, first: Int, before: String, last: Int): VehiclesConnection
          vehicle(id: ID, vehicleID: ID): Vehicle
          "Fetches an object given its ID"
          node(
            "The ID of an object"
            id: ID!
          ): Node
        }
        "A connection to a list of items."
        type FilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "Information about pagination in a connection."
        type PageInfo {
          "When paginating forwards, are there more items?"
          hasNextPage: Boolean!
          "When paginating backwards, are there more items?"
          hasPreviousPage: Boolean!
          "When paginating backwards, the cursor to continue."
          startCursor: String
          "When paginating forwards, the cursor to continue."
          endCursor: String
        }
        "An edge in a connection."
        type FilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A single film."
        type Film implements Node {
          "The title of this film."
          title: String
          "The episode number of this film."
          episodeID: Int
          "The opening paragraphs at the beginning of this film."
          openingCrawl: String
          "The name of the director of this film."
          director: String
          "The name(s) of the producer(s) of this film."
          producers: [String]
          "The ISO 8601 date format of film release at original creator country."
          releaseDate: String
          speciesConnection(after: String, first: Int, before: String, last: Int): FilmSpeciesConnection
          starshipConnection(after: String, first: Int, before: String, last: Int): FilmStarshipsConnection
          vehicleConnection(after: String, first: Int, before: String, last: Int): FilmVehiclesConnection
          characterConnection(after: String, first: Int, before: String, last: Int): FilmCharactersConnection
          planetConnection(after: String, first: Int, before: String, last: Int): FilmPlanetsConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        "A connection to a list of items."
        type FilmSpeciesConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmSpeciesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          species: [Species]
        }
        "An edge in a connection."
        type FilmSpeciesEdge {
          "The item at the end of the edge"
          node: Species
          "A cursor for use in pagination"
          cursor: String!
        }
        "A type of person or character within the Star Wars Universe."
        type Species implements Node {
          "The name of this species."
          name: String
          """
          The classification of this species, such as "mammal" or "reptile".
          """
          classification: String
          """
          The designation of this species, such as "sentient".
          """
          designation: String
          "The average height of this species in centimeters."
          averageHeight: Float
          "The average lifespan of this species in years, null if unknown."
          averageLifespan: Int
          """
          Common eye colors for this species, null if this species does not typically
          have eyes.
          """
          eyeColors: [String]
          """
          Common hair colors for this species, null if this species does not typically
          have hair.
          """
          hairColors: [String]
          """
          Common skin colors for this species, null if this species does not typically
          have skin.
          """
          skinColors: [String]
          "The language commonly spoken by this species."
          language: String
          "A planet that this species originates from."
          homeworld: Planet
          personConnection(after: String, first: Int, before: String, last: Int): SpeciesPeopleConnection
          filmConnection(after: String, first: Int, before: String, last: Int): SpeciesFilmsConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        """
        A large mass, planet or planetoid in the Star Wars Universe, at the time of
        0 ABY.
        """
        type Planet implements Node {
          "The name of this planet."
          name: String
          "The diameter of this planet in kilometers."
          diameter: Int
          """
          The number of standard hours it takes for this planet to complete a single
          rotation on its axis.
          """
          rotationPeriod: Int
          """
          The number of standard days it takes for this planet to complete a single orbit
          of its local star.
          """
          orbitalPeriod: Int
          """
          A number denoting the gravity of this planet, where "1" is normal or 1 standard
          G. "2" is twice or 2 standard Gs. "0.5" is half or 0.5 standard Gs.
          """
          gravity: String
          "The average population of sentient beings inhabiting this planet."
          population: Float
          "The climates of this planet."
          climates: [String]
          "The terrains of this planet."
          terrains: [String]
          """
          The percentage of the planet surface that is naturally occuring water or bodies
          of water.
          """
          surfaceWater: Float
          residentConnection(after: String, first: Int, before: String, last: Int): PlanetResidentsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): PlanetFilmsConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        "A connection to a list of items."
        type PlanetResidentsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PlanetResidentsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          residents: [Person]
        }
        "An edge in a connection."
        type PlanetResidentsEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "An individual person or character within the Star Wars universe."
        type Person implements Node {
          "The name of this person."
          name: String
          """
          The birth year of the person, using the in-universe standard of BBY or ABY -
          Before the Battle of Yavin or After the Battle of Yavin. The Battle of Yavin is
          a battle that occurs at the end of Star Wars episode IV: A New Hope.
          """
          birthYear: String
          """
          The eye color of this person. Will be "unknown" if not known or "n/a" if the
          person does not have an eye.
          """
          eyeColor: String
          """
          The gender of this person. Either "Male", "Female" or "unknown",
          "n/a" if the person does not have a gender.
          """
          gender: String
          """
          The hair color of this person. Will be "unknown" if not known or "n/a" if the
          person does not have hair.
          """
          hairColor: String
          "The height of the person in centimeters."
          height: Int
          "The mass of the person in kilograms."
          mass: Float
          "The skin color of this person."
          skinColor: String
          "A planet that this person was born on or inhabits."
          homeworld: Planet
          filmConnection(after: String, first: Int, before: String, last: Int): PersonFilmsConnection
          "The species that this person belongs to, or null if unknown."
          species: Species
          starshipConnection(after: String, first: Int, before: String, last: Int): PersonStarshipsConnection
          vehicleConnection(after: String, first: Int, before: String, last: Int): PersonVehiclesConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        "A connection to a list of items."
        type PersonFilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PersonFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "An edge in a connection."
        type PersonFilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type PersonStarshipsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PersonStarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        "An edge in a connection."
        type PersonStarshipsEdge {
          "The item at the end of the edge"
          node: Starship
          "A cursor for use in pagination"
          cursor: String!
        }
        "A single transport craft that has hyperdrive capability."
        type Starship implements Node {
          """
          The name of this starship. The common name, such as "Death Star".
          """
          name: String
          """
          The model or official name of this starship. Such as "T-65 X-wing" or "DS-1
          Orbital Battle Station".
          """
          model: String
          """
          The class of this starship, such as "Starfighter" or "Deep Space Mobile
          Battlestation"
          """
          starshipClass: String
          "The manufacturers of this starship."
          manufacturers: [String]
          "The cost of this starship new, in galactic credits."
          costInCredits: Float
          "The length of this starship in meters."
          length: Float
          "The number of personnel needed to run or pilot this starship."
          crew: String
          "The number of non-essential people this starship can transport."
          passengers: String
          """
          The maximum speed of this starship in atmosphere. null if this starship is
          incapable of atmosphering flight.
          """
          maxAtmospheringSpeed: Int
          "The class of this starships hyperdrive."
          hyperdriveRating: Float
          """
          The Maximum number of Megalights this starship can travel in a standard hour.
          A "Megalight" is a standard unit of distance and has never been defined before
          within the Star Wars universe. This figure is only really useful for measuring
          the difference in speed of starships. We can assume it is similar to AU, the
          distance between our Sun (Sol) and Earth.
          """
          MGLT: Int
          "The maximum number of kilograms that this starship can transport."
          cargoCapacity: Float
          """
          The maximum length of time that this starship can provide consumables for its
          entire crew without having to resupply.
          """
          consumables: String
          pilotConnection(after: String, first: Int, before: String, last: Int): StarshipPilotsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): StarshipFilmsConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        "A connection to a list of items."
        type StarshipPilotsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [StarshipPilotsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          pilots: [Person]
        }
        "An edge in a connection."
        type StarshipPilotsEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type StarshipFilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [StarshipFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "An edge in a connection."
        type StarshipFilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type PersonVehiclesConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PersonVehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        "An edge in a connection."
        type PersonVehiclesEdge {
          "The item at the end of the edge"
          node: Vehicle
          "A cursor for use in pagination"
          cursor: String!
        }
        "A single transport craft that does not have hyperdrive capability"
        type Vehicle implements Node {
          """
          The name of this vehicle. The common name, such as "Sand Crawler" or "Speeder
          bike".
          """
          name: String
          """
          The model or official name of this vehicle. Such as "All-Terrain Attack
          Transport".
          """
          model: String
          """
          The class of this vehicle, such as "Wheeled" or "Repulsorcraft".
          """
          vehicleClass: String
          "The manufacturers of this vehicle."
          manufacturers: [String]
          "The cost of this vehicle new, in Galactic Credits."
          costInCredits: Float
          "The length of this vehicle in meters."
          length: Float
          "The number of personnel needed to run or pilot this vehicle."
          crew: String
          "The number of non-essential people this vehicle can transport."
          passengers: String
          "The maximum speed of this vehicle in atmosphere."
          maxAtmospheringSpeed: Int
          "The maximum number of kilograms that this vehicle can transport."
          cargoCapacity: Float
          """
          The maximum length of time that this vehicle can provide consumables for its
          entire crew without having to resupply.
          """
          consumables: String
          pilotConnection(after: String, first: Int, before: String, last: Int): VehiclePilotsConnection
          filmConnection(after: String, first: Int, before: String, last: Int): VehicleFilmsConnection
          "The ISO 8601 date format of the time that this resource was created."
          created: String
          "The ISO 8601 date format of the time that this resource was edited."
          edited: String
          "The ID of an object"
          id: ID!
        }
        "A connection to a list of items."
        type VehiclePilotsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [VehiclePilotsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          pilots: [Person]
        }
        "An edge in a connection."
        type VehiclePilotsEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type VehicleFilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [VehicleFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "An edge in a connection."
        type VehicleFilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type PlanetFilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PlanetFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "An edge in a connection."
        type PlanetFilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type SpeciesPeopleConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [SpeciesPeopleEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          people: [Person]
        }
        "An edge in a connection."
        type SpeciesPeopleEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type SpeciesFilmsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [SpeciesFilmsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          films: [Film]
        }
        "An edge in a connection."
        type SpeciesFilmsEdge {
          "The item at the end of the edge"
          node: Film
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type FilmStarshipsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmStarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        "An edge in a connection."
        type FilmStarshipsEdge {
          "The item at the end of the edge"
          node: Starship
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type FilmVehiclesConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmVehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        "An edge in a connection."
        type FilmVehiclesEdge {
          "The item at the end of the edge"
          node: Vehicle
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type FilmCharactersConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmCharactersEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          characters: [Person]
        }
        "An edge in a connection."
        type FilmCharactersEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type FilmPlanetsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [FilmPlanetsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          planets: [Planet]
        }
        "An edge in a connection."
        type FilmPlanetsEdge {
          "The item at the end of the edge"
          node: Planet
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type PeopleConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PeopleEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          people: [Person]
        }
        "An edge in a connection."
        type PeopleEdge {
          "The item at the end of the edge"
          node: Person
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type PlanetsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [PlanetsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          planets: [Planet]
        }
        "An edge in a connection."
        type PlanetsEdge {
          "The item at the end of the edge"
          node: Planet
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type SpeciesConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [SpeciesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          species: [Species]
        }
        "An edge in a connection."
        type SpeciesEdge {
          "The item at the end of the edge"
          node: Species
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type StarshipsConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [StarshipsEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          starships: [Starship]
        }
        "An edge in a connection."
        type StarshipsEdge {
          "The item at the end of the edge"
          node: Starship
          "A cursor for use in pagination"
          cursor: String!
        }
        "A connection to a list of items."
        type VehiclesConnection {
          "Information to aid in pagination."
          pageInfo: PageInfo!
          "A list of edges."
          edges: [VehiclesEdge]
          """
          A count of the total number of objects in this connection, ignoring pagination.
          This allows a client to fetch the first five objects by passing "5" as the
          argument to "first", then fetch the total count so it could display "5 of 83",
          for example.
          """
          totalCount: Int
          """
          A list of all of the objects returned in the connection. This is a convenience
          field provided for quickly exploring the API; rather than querying for
          "{ edges { node } }" when no edge data is needed, this field can be be used
          instead. Note that when clients like Relay need to fetch the "cursor" field on
          the edge to enable efficient pagination, this shortcut cannot be used, and the
          full "{ edges { node } }" version should be used instead.
          """
          vehicles: [Vehicle]
        }
        "An edge in a connection."
        type VehiclesEdge {
          "The item at the end of the edge"
          node: Vehicle
          "A cursor for use in pagination"
          cursor: String!
        }
        "An object with an ID"
        interface Node {
          "The id of the object."
          id: ID!
        }
    "#}
        );
    }

    #[test]
    fn it_builds_schema_with_interfaces() {
        let file = File::open("src/operations/graph/introspect/fixtures/interfaces.json").unwrap();
        let res: Response<QueryResponseData> = serde_json::from_reader(file).unwrap();

        let data = res.data.unwrap();
        let schema = Schema::try_from(data).unwrap();
        assert_eq!(
            schema.encode(),
            indoc! { r#"
             type Query {
               "Fetch a simple list of products with an offset"
               topProducts(first: Int = 5): [Product] @deprecated(reason: "Use `products` instead")
               "Fetch a paginated list of products based on a filter type."
               products(first: Int = 5, after: Int = 0, type: ProductType): ProductConnection
               """
               The currently authenticated user root. All nodes off of this
               root will be authenticated as the current user
               """
               me: User
             }
             "A review is any feedback about products across the graph"
             type Review {
               id: ID!
               "The plain text version of the review"
               body: String
               "The user who authored the review"
               author: User
               "The product which this review is about"
               product: Product
             }
             "The base User in Acephei"
             type User {
               "A globally unique id for the user"
               id: ID!
               "The users full name as provided"
               name: String
               "The account username of the user"
               username: String
               "A list of all reviews by the user"
               reviews: [Review]
             }
             "A connection wrapper for lists of reviews"
             type ReviewConnection {
               "Helpful metadata about the connection"
               pageInfo: PageInfo
               "List of reviews returned by the search"
               edges: [ReviewEdge]
             }
             """
             The PageInfo type provides pagination helpers for determining
             if more data can be fetched from the list
             """
             type PageInfo {
               "More items exist in the list"
               hasNextPage: Boolean
               "Items earlier in the list exist"
               hasPreviousPage: Boolean
             }
             "A connection edge for the Review type"
             type ReviewEdge {
               review: Review
             }
             "A connection wrapper for lists of products"
             type ProductConnection {
               "Helpful metadata about the connection"
               pageInfo: PageInfo
               "List of products returned by the search"
               edges: [ProductEdge]
             }
             "A connection edge for the Product type"
             type ProductEdge {
               product: Product
             }
             "The basic book in the graph"
             type Book implements Product {
               "All books can be found by an isbn"
               isbn: String!
               "The title of the book"
               title: String
               "The year the book was published"
               year: Int
               """
               Улюблена книга, прочитана цього рока "" | 今年読んだお気に入りの本
               """
               favouriteBook: Book
               "A simple list of similar books"
               similarBooks: [Book]
               reviews: [Review]
               reviewList(first: Int = 5, after: Int = 0): ReviewConnection
               """
               relatedReviews for a book use the knowledge of `similarBooks` from the books
               service to return related reviews that may be of interest to the user
               """
               relatedReviews(first: Int = 5, after: Int = 0): ReviewConnection
               "Since books are now products, we can also use their upc as a primary id"
               upc: String!
               "The name of a book is the book's title + year published"
               name(delimeter: String = " "): String
               price: Int
               weight: Int
             }
             "Information about the brand Amazon"
             type Amazon {
               "The url of a referrer for a product"
               referrer: String
             }
             "Information about the brand Ikea"
             type Ikea {
               "Which asile to find an item"
               asile: Int
             }
             """
             The Furniture type represents all products which are items
             of furniture.
             """
             type Furniture implements Product {
               "The modern primary identifier for furniture"
               upc: String!
               "The SKU field is how furniture was previously stored, and still exists in some legacy systems"
               sku: String!
               name: String
               price: Int
               "The brand of furniture"
               brand: Brand
               weight: Int
               reviews: [Review]
               reviewList(first: Int = 5, after: Int = 0): ReviewConnection
             }
             "The Product type represents all products within the system"
             interface Product {
               "The primary identifier of products in the graph"
               upc: String!
               "The display name of the product"
               name: String
               "A simple integer price of the product in US dollars"
               price: Int
               "How much the product weighs in kg"
               weight: Int @deprecated(reason: "Not all product's have a weight")
               "A simple list of all reviews for a product"
               reviews: [Review] @deprecated(reason: """The `reviews` field on product is deprecated to roll over the return
             type from a simple list to a paginated list. The easiest way to fix your
             operations is to alias the new field `reviewList` to `review`.
             Once all clients have updated, we will roll over this field and deprecate
             `reviewList` in favor of the field name `reviews` again""")
               """
               A paginated list of reviews. This field naming is temporary while all clients
               migrate off of the un-paginated version of this field call reviews. To ease this migration,
               alias your usage of `reviewList` to `reviews` so that after the roll over is finished, you
               can remove the alias and use the final field name.
               """
               reviewList(first: Int = 5, after: Int = 0): ReviewConnection
             }
             "A union of all brands represented within the store"
             union Brand = Ikea | Amazon
             "An enum of product types"
             enum ProductType {
               LATEST
               TRENDING
             }
         "#}
        )
    }
}
