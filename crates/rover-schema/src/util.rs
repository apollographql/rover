/// Extract the named type from a GraphQL type reference, stripping `[`, `]`, and `!` wrappers.
///
/// For example: `[String!]!` → `String`, `Post` → `Post`.
pub(crate) fn unwrap_type_name(type_str: &str) -> String {
    type_str.replace(['[', ']', '!'], "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_wrappers() {
        assert_eq!(unwrap_type_name("[String!]!"), "String");
        assert_eq!(unwrap_type_name("Post"), "Post");
        assert_eq!(unwrap_type_name("[User!]"), "User");
        assert_eq!(unwrap_type_name("Int!"), "Int");
    }
}
