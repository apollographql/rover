use heck::ToSnakeCase;
use rust_stemmers::{Algorithm, Stemmer};

/// Why a result matched its query, ordered from strongest to weakest precedence.
///
/// The derived [`Ord`] uses declaration order, so:
/// `Exact < Stem < Fuzzy < Description`. Sorting results by `score` in
/// ascending order surfaces the strongest matches first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum_macros::EnumIter)]
pub(super) enum MatchScore {
    /// All terms appeared as a substring of the name or one of its tokens.
    Exact,
    /// All terms matched a name token after English stemming.
    Stem,
    /// All terms came within one edit of a name token (terms must be ≥ 4 chars).
    Fuzzy,
    /// All terms appeared in the description as a substring or stemmed token,
    /// but no token of the name matched.
    Description,
}

impl MatchScore {
    /// Returns the strongest tier at which all `terms` match `name`/`description`,
    /// or `None` if no tier matches.
    ///
    /// Tiers are checked in decreasing precedence and the first match wins —
    /// e.g. a name that matches exactly never falls back to a stem match.
    pub(super) fn new(name: &str, description: Option<&str>, terms: &[String]) -> Option<Self> {
        let words = tokenize(name);

        Self::maybe_exact(name, &words, terms)
            .or_else(|| Self::maybe_stem(&words, terms))
            .or_else(|| Self::maybe_fuzzy(&words, terms))
            .or_else(|| Self::maybe_description(description, terms))
    }

    fn maybe_exact(name: &str, words: &[String], terms: &[String]) -> Option<Self> {
        let name = name.to_lowercase();
        let exact_hit = terms
            .iter()
            .all(|t| name.contains(t.as_str()) || words.iter().any(|w| w.contains(t.as_str())));
        if exact_hit { Some(Self::Exact) } else { None }
    }

    fn maybe_stem(words: &[String], terms: &[String]) -> Option<Self> {
        let stemmer = Stemmer::create(Algorithm::English);
        let stemmed_words: Vec<String> =
            words.iter().map(|w| stemmer.stem(w).into_owned()).collect();
        let stem_hit = terms.iter().all(|t| {
            let stemmed_term = stemmer.stem(t).into_owned();
            stemmed_words.iter().any(|sw| sw == &stemmed_term)
        });
        if stem_hit { Some(Self::Stem) } else { None }
    }

    fn maybe_fuzzy(words: &[String], terms: &[String]) -> Option<Self> {
        let fuzzy_hit = terms
            .iter()
            .all(|t| t.len() >= 4 && words.iter().any(|w| strsim::levenshtein(w, t) <= 1));
        if fuzzy_hit { Some(Self::Fuzzy) } else { None }
    }

    fn maybe_description(description: Option<&str>, terms: &[String]) -> Option<Self> {
        description.and_then(|description| {
            let description = description.to_lowercase();
            let stemmer = Stemmer::create(Algorithm::English);
            let stemmed_words: Vec<String> = tokenize_text(&description)
                .iter()
                .map(|w| stemmer.stem(w).into_owned())
                .collect();
            let hit = terms.iter().all(|t| {
                if description.contains(t.as_str()) {
                    return true;
                }
                let stemmed_term = stemmer.stem(t).into_owned();
                stemmed_words.iter().any(|sw| sw == &stemmed_term)
            });
            if hit { Some(Self::Description) } else { None }
        })
    }
}

/// Splits prose into lowercase words on non-alphanumeric boundaries.
fn tokenize_text(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Splits a GraphQL name into lowercase words by camelCase and snake_case boundaries.
///
/// Examples: `"getUserById"` → `["get", "user", "by", "id"]`
///           `"CREATE_POST"` → `["create", "post"]`
///           `"HTMLParser"`  → `["html", "parser"]`
pub(super) fn tokenize(name: &str) -> Vec<String> {
    name.to_snake_case().split('_').map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::camel("getUserById", vec!["get", "user", "by", "id"])]
    #[case::pascal_with_acronym("HTMLParser", vec!["html", "parser"])]
    #[case::camel_short("avatarUrl", vec!["avatar", "url"])]
    #[case::screaming_snake("CREATE_POST", vec!["create", "post"])]
    #[case::single_word("email", vec!["email"])]
    fn test_tokenize_splits_on_camel_and_snake_boundaries(
        #[case] name: &str,
        #[case] expected: Vec<&str>,
    ) {
        let expected: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
        assert_that!(tokenize(name)).is_equal_to(expected);
    }

    #[rstest]
    fn test_ord_values() {
        use strum::IntoEnumIterator;
        // `EnumIter` yields every variant in declaration order. Asserting the
        // full sequence catches a new variant, a reordering, or a removal —
        // the test fails until this list is updated to match.
        let ordered: Vec<MatchScore> = MatchScore::iter().collect();
        assert_that!(ordered).is_equal_to(vec![
            MatchScore::Exact,
            MatchScore::Stem,
            MatchScore::Fuzzy,
            MatchScore::Description,
        ]);
    }

    fn terms(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_lowercase).collect()
    }

    #[rstest]
    fn test_new_exact_when_term_substring_of_name() {
        let score = MatchScore::new("createPost", None, &terms("post"));
        assert_that!(score).is_equal_to(Some(MatchScore::Exact));
    }

    #[rstest]
    fn test_new_stem_when_inflected_form_matches_token() {
        // "creating" stems to "creat"; "createPost" tokenizes to ["create","post"];
        // "create" stems to "creat" → match at Stem (not Exact since no substring hit).
        let score = MatchScore::new("createPost", None, &terms("creating"));
        assert_that!(score).is_equal_to(Some(MatchScore::Stem));
    }

    #[rstest]
    fn test_new_fuzzy_when_term_within_one_edit_of_token() {
        // "wedget" is 1 substitution from "widget"; neither word stems to the
        // same root, so this falls through Exact and Stem and lands on Fuzzy.
        let score = MatchScore::new("widget", None, &terms("wedget"));
        assert_that!(score).is_equal_to(Some(MatchScore::Fuzzy));
    }

    #[rstest]
    fn test_new_fuzzy_requires_term_length_at_least_four() {
        // 3-char term within 1 edit shouldn't trigger fuzzy.
        let score = MatchScore::new("foo", None, &terms("fop"));
        assert_that!(score).is_equal_to(None);
    }

    #[rstest]
    fn test_new_description_only_when_name_does_not_match() {
        let score = MatchScore::new("Post", Some("Written by the author"), &terms("author"));
        assert_that!(score).is_equal_to(Some(MatchScore::Description));
    }

    #[rstest]
    fn test_new_description_stem_match() {
        // "creating" stems to "creat"; description "Creates a new post" tokens
        // stem to ["creat", "a", "new", "post"]. Name doesn't match → Description.
        let score = MatchScore::new("Post", Some("Creates a new post"), &terms("creating"));
        assert_that!(score).is_equal_to(Some(MatchScore::Description));
    }

    #[rstest]
    fn test_new_returns_none_when_nothing_matches() {
        let score = MatchScore::new("createPost", Some("makes a post"), &terms("xyzzy"));
        assert_that!(score).is_equal_to(None);
    }

    #[rstest]
    fn test_new_requires_all_terms_to_match_at_same_tier() {
        // "create" matches exactly; "xyzzy" matches nothing at any tier → None overall.
        let score = MatchScore::new("createPost", Some("makes a post"), &terms("create xyzzy"));
        assert_that!(score).is_equal_to(None);
    }
}
