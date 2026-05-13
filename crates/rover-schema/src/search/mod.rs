mod match_score;
mod result;

pub use result::{ElementKind, SearchResult};

use crate::ParsedSchema;

impl ParsedSchema {
    /// Search the schema for elements whose name or description matches all of the given terms.
    ///
    /// Terms are whitespace-separated. Matching is case-insensitive substring search after
    /// splitting camelCase / snake_case names into words. Results are sorted by relevance
    /// (name matches rank above description matches) then alphabetically by coordinate.
    pub fn search(&self, query: &str, limit: usize, include_deprecated: bool) -> Vec<SearchResult> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        if terms.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = self
            .inner()
            .types
            .iter()
            .filter(|(type_name, _)| !type_name.starts_with("__"))
            .flat_map(|(type_name, ty)| {
                SearchResult::from_extended_type(self, type_name, ty, &terms, include_deprecated)
            })
            .collect();

        results.sort_by(|a, b| {
            a.score()
                .cmp(&b.score())
                .then_with(|| a.coordinate.to_string().cmp(&b.coordinate.to_string()))
        });
        results.truncate(limit);
        results
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use crate::ParsedSchema;

    #[fixture]
    fn schema() -> ParsedSchema {
        let sdl = include_str!("../test_fixtures/test_schema.graphql");
        ParsedSchema::parse(sdl, "test_schema.graphql")
    }

    #[rstest]
    fn test_search_by_field_name(schema: ParsedSchema) {
        let results = schema.search("email", 10, false);
        assert_that!(&results).matching_contains(|r| r.coordinate.to_string() == "User.email");
    }

    #[rstest]
    fn test_search_camel_case_split(schema: ParsedSchema) {
        // "avatar" should match "avatarUrl" after camelCase splitting
        let results = schema.search("avatar", 10, false);
        assert_that!(&results).matching_contains(|r| r.coordinate.to_string() == "User.avatarUrl");
    }

    #[rstest]
    fn test_search_multi_term_requires_all(schema: ParsedSchema) {
        // "create" alone is enough to match Mutation.createPost.
        let create_only = schema.search("create", 10, false);
        assert_that!(&create_only)
            .matching_contains(|r| r.coordinate.to_string() == "Mutation.createPost");

        // "create post" still matches it — both tokens are present.
        let both = schema.search("create post", 10, false);
        assert_that!(&both)
            .matching_contains(|r| r.coordinate.to_string() == "Mutation.createPost");

        // "create xyzzy" returns nothing: every term must match for any
        // result to be returned, and "xyzzy" matches nothing in the schema.
        let with_unknown = schema.search("create xyzzy", 10, false);
        assert_that!(with_unknown).is_empty();
    }

    #[rstest]
    fn test_search_description_match(schema: ParsedSchema) {
        // "author" appears in description "The author of this post"
        let results = schema.search("author", 10, false);
        assert_that!(results).is_not_empty();
    }

    #[rstest]
    fn test_search_name_beats_description(schema: ParsedSchema) {
        let results = schema.search("author", 10, false);

        // Post.author matches "author" on its name (Exact tier).
        let name_match = results
            .iter()
            .find(|r| r.coordinate.to_string() == "Post.author")
            .expect("Post.author should appear");

        // User.posts has the term only in its description ("Posts authored by this user"),
        // so it matches at the Description tier.
        let desc_match = results
            .iter()
            .find(|r| r.coordinate.to_string() == "User.posts")
            .expect("User.posts should appear");

        assert_that!(name_match.score()).is_less_than(desc_match.score());
    }

    #[rstest]
    fn test_search_no_results_for_gibberish(schema: ParsedSchema) {
        let results = schema.search("xyzzy_notafield", 10, false);
        assert_that!(results).is_empty();
    }

    #[rstest]
    fn test_search_deprecated_excluded_by_default(schema: ParsedSchema) {
        let without = schema.search("legacy", 10, false);
        let with_dep = schema.search("legacy", 10, true);
        // legacyId is deprecated — should appear with include_deprecated=true
        assert_that!(&with_dep).matching_contains(|r| r.coordinate.to_string() == "User.legacyId");
        assert_that!(
            without
                .iter()
                .any(|r| r.coordinate.to_string() == "User.legacyId")
        )
        .is_false();
    }

    #[rstest]
    fn test_search_limit_respected(schema: ParsedSchema) {
        let results = schema.search("id", 3, true);
        assert_that!(results.len()).is_less_than_or_equal_to(3);
    }

    #[rstest]
    fn test_search_stem_match(schema: ParsedSchema) {
        // "creating" stems to "creat", matching "createPost" token "create" → "creat"
        let results = schema.search("creating", 10, false);
        assert_that!(&results)
            .matching_contains(|r| r.coordinate.to_string() == "Mutation.createPost");
    }

    #[rstest]
    fn test_search_exact_beats_stem(schema: ParsedSchema) {
        // exact name match outranks stem match
        let exact = schema.search("create", 10, false);
        let stem = schema.search("creating", 10, false);
        if let (Some(e), Some(s)) = (exact.first(), stem.first()) {
            assert_that!(e.score()).is_less_than(s.score());
        }
    }

    #[rstest]
    fn test_search_fuzzy_typo_in_name(schema: ParsedSchema) {
        // "emaill" is one insertion away from "email", not a substring of anything
        let results = schema.search("emaill", 10, false);
        assert_that!(&results).matching_contains(|r| r.coordinate.to_string() == "User.email");
    }

    #[rstest]
    fn test_search_exact_ranks_above_fuzzy(schema: ParsedSchema) {
        // exact matches should outrank fuzzy matches
        let exact = schema.search("email", 10, false);
        let fuzzy = schema.search("emaill", 10, false);
        if let (Some(e), Some(f)) = (exact.first(), fuzzy.first()) {
            assert_that!(e.score()).is_less_than(f.score());
        }
    }
}
