use apollo_compiler::{
    Name,
    coordinate::{SchemaCoordinate, TypeAttributeCoordinate, TypeCoordinate},
    schema::ExtendedType,
};

use super::match_score::MatchScore;
use crate::{ParsedSchema, describe::deprecated::IsDeprecated, root_paths::RootPath};

/// The kind of schema element a search result refers to.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    /// A named type (object, interface, enum, union, input, scalar).
    Type,
    /// A field on an object or interface type.
    Field,
    /// A field on an input object type.
    InputField,
    /// A value within an enum type.
    EnumValue,
}

impl std::fmt::Display for ElementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Type => write!(f, "type"),
            Self::Field => write!(f, "field"),
            Self::InputField => write!(f, "input field"),
            Self::EnumValue => write!(f, "enum value"),
        }
    }
}

/// A single result from a schema search.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    /// Schema coordinate, e.g. `User`, `User.email`, `Status.ACTIVE`.
    /// Serialized as the rendered coordinate string (e.g. `"User.email"`).
    #[serde(serialize_with = "serialize_coordinate")]
    pub coordinate: SchemaCoordinate,
    /// The kind of element this result refers to.
    pub kind: ElementKind,
    /// Description from the SDL, if present.
    pub description: Option<String>,
    /// Root paths from Query/Mutation to the containing type.
    pub via: Vec<RootPath>,
    #[serde(skip)]
    score: MatchScore,
}

#[bon::bon]
impl SearchResult {
    /// Build a result pointing at a top-level type (`User`).
    #[builder]
    pub(super) fn for_type(
        type_name: &Name,
        description: Option<String>,
        #[builder(default)] via: Vec<RootPath>,
        score: MatchScore,
    ) -> Self {
        Self {
            coordinate: SchemaCoordinate::Type(TypeCoordinate {
                ty: type_name.clone(),
            }),
            kind: ElementKind::Type,
            description,
            via,
            score,
        }
    }

    /// Build a result pointing at a field, input field, or enum value
    /// (`User.email`, `Status.ACTIVE`).
    #[builder]
    pub(super) fn for_attribute(
        type_name: &Name,
        attribute: &Name,
        kind: ElementKind,
        description: Option<String>,
        #[builder(default)] via: Vec<RootPath>,
        score: MatchScore,
    ) -> Self {
        Self {
            coordinate: SchemaCoordinate::TypeAttribute(TypeAttributeCoordinate {
                ty: type_name.clone(),
                attribute: attribute.clone(),
            }),
            kind,
            description,
            via,
            score,
        }
    }

    pub(super) const fn score(&self) -> MatchScore {
        self.score
    }

    /// Collect all matching results from a single top-level schema type:
    /// the type itself (if it matches) plus any matching fields, input
    /// fields, or enum values it owns.
    pub(super) fn from_extended_type(
        schema: &ParsedSchema,
        type_name: &Name,
        ty: &ExtendedType,
        terms: &[String],
        include_deprecated: bool,
    ) -> Vec<Self> {
        let mut out = Vec::new();
        match ty {
            ExtendedType::Object(obj) => {
                let via = schema.find_root_paths(type_name);
                let desc = obj.description.as_ref().map(|d| d.to_string());
                out.extend(Self::from_type_match(type_name, desc, via.clone(), terms));
                for (field_name, field) in &obj.fields {
                    if include_deprecated || !field.is_deprecated() {
                        let fdesc = field.description.as_ref().map(|d| d.to_string());
                        out.extend(Self::from_attribute_match(
                            type_name,
                            field_name,
                            ElementKind::Field,
                            fdesc,
                            via.clone(),
                            terms,
                        ));
                    }
                }
            }
            ExtendedType::Interface(iface) => {
                let via = schema.find_root_paths(type_name);
                let desc = iface.description.as_ref().map(|d| d.to_string());
                out.extend(Self::from_type_match(type_name, desc, via.clone(), terms));
                for (field_name, field) in &iface.fields {
                    if include_deprecated || !field.is_deprecated() {
                        let fdesc = field.description.as_ref().map(|d| d.to_string());
                        out.extend(Self::from_attribute_match(
                            type_name,
                            field_name,
                            ElementKind::Field,
                            fdesc,
                            via.clone(),
                            terms,
                        ));
                    }
                }
            }
            ExtendedType::InputObject(inp) => {
                let desc = inp.description.as_ref().map(|d| d.to_string());
                out.extend(Self::from_type_match(type_name, desc, Vec::new(), terms));
                for (field_name, field) in &inp.fields {
                    if include_deprecated || !field.is_deprecated() {
                        let fdesc = field.description.as_ref().map(|d| d.to_string());
                        out.extend(Self::from_attribute_match(
                            type_name,
                            field_name,
                            ElementKind::InputField,
                            fdesc,
                            Vec::new(),
                            terms,
                        ));
                    }
                }
            }
            ExtendedType::Enum(e) => {
                let via = schema.find_root_paths(type_name);
                let desc = e.description.as_ref().map(|d| d.to_string());
                out.extend(Self::from_type_match(type_name, desc, via.clone(), terms));
                for (val_name, val) in &e.values {
                    if include_deprecated || !val.is_deprecated() {
                        let vdesc = val.description.as_ref().map(|d| d.to_string());
                        out.extend(Self::from_attribute_match(
                            type_name,
                            val_name,
                            ElementKind::EnumValue,
                            vdesc,
                            via.clone(),
                            terms,
                        ));
                    }
                }
            }
            ExtendedType::Union(u) => {
                let desc = u.description.as_ref().map(|d| d.to_string());
                let via = schema.find_root_paths(type_name);
                out.extend(Self::from_type_match(type_name, desc, via, terms));
            }
            ExtendedType::Scalar(s) => {
                let desc = s.description.as_ref().map(|d| d.to_string());
                out.extend(Self::from_type_match(type_name, desc, Vec::new(), terms));
            }
        }
        out
    }

    /// Build a result for a top-level type when its name or description
    /// matches every term. Returns `None` when no tier matches.
    fn from_type_match(
        type_name: &Name,
        description: Option<String>,
        via: Vec<RootPath>,
        terms: &[String],
    ) -> Option<Self> {
        let score = MatchScore::new(type_name.as_str(), description.as_deref(), terms)?;
        Some(
            Self::for_type()
                .type_name(type_name)
                .maybe_description(description)
                .via(via)
                .score(score)
                .call(),
        )
    }

    /// Build a result for a single attribute — field, input field, or enum
    /// value — when its name or description matches every term.
    fn from_attribute_match(
        type_name: &Name,
        attribute: &Name,
        kind: ElementKind,
        description: Option<String>,
        via: Vec<RootPath>,
        terms: &[String],
    ) -> Option<Self> {
        let score = MatchScore::new(attribute.as_str(), description.as_deref(), terms)?;
        Some(
            Self::for_attribute()
                .type_name(type_name)
                .attribute(attribute)
                .kind(kind)
                .maybe_description(description)
                .via(via)
                .score(score)
                .call(),
        )
    }
}

fn serialize_coordinate<S: serde::Serializer>(
    coord: &SchemaCoordinate,
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_str(coord)
}

#[cfg(test)]
mod tests {
    use apollo_compiler::{coord, name};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    fn terms(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_lowercase).collect()
    }

    fn ty_of<'a>(schema: &'a ParsedSchema, type_name: &Name) -> &'a ExtendedType {
        schema
            .inner()
            .types
            .get(type_name)
            .expect("type missing from fixture")
    }

    #[rstest]
    #[case::type_kind(ElementKind::Type, "type")]
    #[case::field(ElementKind::Field, "field")]
    #[case::input_field(ElementKind::InputField, "input field")]
    #[case::enum_value(ElementKind::EnumValue, "enum value")]
    fn test_element_kind_display(#[case] kind: ElementKind, #[case] expected: &str) {
        assert_that!(kind.to_string().as_str()).is_equal_to(expected);
    }

    #[rstest]
    fn test_for_type_builds_type_coordinate() {
        let result = SearchResult::for_type()
            .type_name(&name!("User"))
            .maybe_description(Some("a user".to_string()))
            .score(MatchScore::Exact)
            .call();
        assert_that!(result.coordinate).is_equal_to(SchemaCoordinate::from(coord!(User)));
        assert_that!(result.kind).is_equal_to(ElementKind::Type);
        assert_that!(result.via).is_empty();
        assert_that!(result.description)
            .is_some()
            .is_equal_to("a user".to_string());
    }

    #[rstest]
    fn test_for_attribute_builds_typeattribute_coordinate() {
        let result = SearchResult::for_attribute()
            .type_name(&name!("User"))
            .attribute(&name!("email"))
            .kind(ElementKind::Field)
            .score(MatchScore::Stem)
            .call();
        assert_that!(result.coordinate).is_equal_to(SchemaCoordinate::from(coord!(User.email)));
        assert_that!(result.kind).is_equal_to(ElementKind::Field);
        assert_that!(result.score()).is_equal_to(MatchScore::Stem);
        assert_that!(result.via).is_empty();
    }

    #[rstest]
    fn test_serializes_to_full_json_structure() {
        use crate::root_paths::PathSegment;

        let result = SearchResult::for_attribute()
            .type_name(&name!("User"))
            .attribute(&name!("email"))
            .kind(ElementKind::Field)
            .description("the user's email address".to_string())
            .via(vec![RootPath {
                segments: vec![PathSegment {
                    type_name: name!("Query"),
                    field_name: name!("user"),
                }],
            }])
            .score(MatchScore::Exact)
            .call();

        let actual = serde_json::to_value(&result);
        let expected = serde_json::json!({
            "coordinate": "User.email",
            "kind": "field",
            "description": "the user's email address",
            "via": [
                {
                    "segments": [
                        {"type_name": "Query", "field_name": "user"}
                    ]
                }
            ],
        });
        // Pins down every public field: coordinate flattens to a string,
        // kind is snake-cased, description renders verbatim, via nests
        // RootPath/PathSegment structure, and score is `serde(skip)`'d.
        assert_that!(actual).is_ok().is_equal_to(expected);
    }

    #[rstest]
    fn test_from_extended_type_object_returns_type_and_matching_field(schema: ParsedSchema) {
        let user = name!("User");
        let ty = ty_of(&schema, &user);
        let results = SearchResult::from_extended_type(&schema, &user, ty, &terms("email"), false);
        assert_that!(&results).matching_contains(|r| {
            r.coordinate == coord!(User.email).into()
                && matches!(r.kind, ElementKind::Field)
                && r.via.iter().any(|p| {
                    p.segments
                        .first()
                        .is_some_and(|s| s.type_name == "Query" && s.field_name == "user")
                })
        });
    }

    #[rstest]
    fn test_from_extended_type_empty_when_no_match(schema: ParsedSchema) {
        let user = name!("User");
        let ty = ty_of(&schema, &user);
        let results =
            SearchResult::from_extended_type(&schema, &user, ty, &terms("xyzzyqqq"), false);
        assert_that!(results).is_empty();
    }

    #[rstest]
    fn test_from_extended_type_interface(schema: ParsedSchema) {
        let node = name!("Node");
        let ty = ty_of(&schema, &node);
        let results = SearchResult::from_extended_type(&schema, &node, ty, &terms("id"), false);
        assert_that!(&results).matching_contains(|r| {
            r.coordinate == coord!(Node.id).into()
                && matches!(r.kind, ElementKind::Field)
                && r.via.is_empty()
        });
    }

    #[rstest]
    fn test_from_extended_type_input_object_uses_inputfield_kind_and_no_via(schema: ParsedSchema) {
        let input = name!("CreatePostInput");
        let ty = ty_of(&schema, &input);
        let results = SearchResult::from_extended_type(&schema, &input, ty, &terms("title"), false);
        assert_that!(&results).matching_contains(|r| {
            r.coordinate == coord!(CreatePostInput.title).into()
                && r.kind == ElementKind::InputField
                && r.via.is_empty()
        });
    }

    #[rstest]
    fn test_from_extended_type_enum_uses_enumvalue_kind(schema: ParsedSchema) {
        let role = name!("Role");
        let ty = ty_of(&schema, &role);
        let results = SearchResult::from_extended_type(&schema, &role, ty, &terms("admin"), false);
        assert_that!(&results).matching_contains(|r| {
            r.coordinate == coord!(Role.ADMIN).into() && r.kind == ElementKind::EnumValue
        });
    }

    #[rstest]
    fn test_from_extended_type_enum_value_inherits_via(schema: ParsedSchema) {
        // DigestFrequency is reachable via Query.viewer.preferences.digestFrequency,
        // so its values should inherit the same root paths as the type itself.
        let freq = name!("DigestFrequency");
        let ty = ty_of(&schema, &freq);
        let results = SearchResult::from_extended_type(&schema, &freq, ty, &terms("daily"), false);
        assert_that!(&results).matching_contains(|r| {
            r.coordinate == coord!(DigestFrequency.DAILY).into()
                && r.via.iter().any(|p| {
                    p.segments
                        .first()
                        .is_some_and(|s| s.type_name == "Query" && s.field_name == "viewer")
                })
        });
    }

    #[rstest]
    fn test_from_extended_type_union_yields_type_only(schema: ParsedSchema) {
        let ci = name!("ContentItem");
        let ty = ty_of(&schema, &ci);
        let results = SearchResult::from_extended_type(&schema, &ci, ty, &terms("content"), false);
        assert_that!(&results).has_length(1);
        assert_that!(results[0].kind).is_equal_to(ElementKind::Type);
        assert_that!(results[0].coordinate)
            .is_equal_to(SchemaCoordinate::from(coord!(ContentItem)));
        assert_that!(results[0].via).is_empty();
    }

    #[rstest]
    fn test_from_extended_type_scalar_yields_type_only_with_no_via(schema: ParsedSchema) {
        let url = name!("URL");
        let ty = ty_of(&schema, &url);
        let results = SearchResult::from_extended_type(&schema, &url, ty, &terms("url"), false);
        assert_that!(results).has_length(1);
        assert_that!(results[0].kind).is_equal_to(ElementKind::Type);
        assert_that!(results[0].via).is_empty();
    }

    #[rstest]
    fn test_from_extended_type_excludes_deprecated_by_default(schema: ParsedSchema) {
        let user = name!("User");
        let ty = ty_of(&schema, &user);
        let without = SearchResult::from_extended_type(&schema, &user, ty, &terms("legacy"), false);
        let with = SearchResult::from_extended_type(&schema, &user, ty, &terms("legacy"), true);
        assert_that!(&with).matching_contains(|r| r.coordinate == coord!(User.legacyId).into());
        assert_that!(
            without
                .iter()
                .any(|r| r.coordinate == coord!(User.legacyId).into())
        )
        .is_false();
    }
}
