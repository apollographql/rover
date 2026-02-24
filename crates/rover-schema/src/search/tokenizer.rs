/// Split a camelCase or PascalCase identifier into lowercase words.
///
/// Examples:
/// - `getUserById` → `["get", "user", "by", "id"]`
/// - `HTMLParser` → `["html", "parser"]`
/// - `PostConnection` → `["post", "connection"]`
pub fn split_camel_case(s: &str) -> Vec<String> {
    use heck::ToSnakeCase;
    s.to_snake_case()
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Prepare text for indexing: split camelCase, join with spaces.
pub fn prepare_for_index(name: &str) -> String {
    split_camel_case(name).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_camel_case() {
        assert_eq!(
            split_camel_case("getUserById"),
            vec!["get", "user", "by", "id"]
        );
    }

    #[test]
    fn pascal_case() {
        assert_eq!(
            split_camel_case("CreatePostInput"),
            vec!["create", "post", "input"]
        );
    }

    #[test]
    fn uppercase_run() {
        assert_eq!(split_camel_case("HTMLParser"), vec!["html", "parser"]);
    }

    #[test]
    fn single_word() {
        assert_eq!(split_camel_case("name"), vec!["name"]);
    }

    #[test]
    fn all_uppercase() {
        assert_eq!(split_camel_case("ID"), vec!["id"]);
    }

    #[test]
    fn snake_case() {
        assert_eq!(
            split_camel_case("get_user_by_id"),
            vec!["get", "user", "by", "id"]
        );
    }

    #[test]
    fn prepare_for_index_works() {
        assert_eq!(prepare_for_index("CreatePostInput"), "create post input");
    }
}
