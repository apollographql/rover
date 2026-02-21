/// Split a camelCase or PascalCase identifier into lowercase words.
///
/// Examples:
/// - `getUserById` → `["get", "user", "by", "id"]`
/// - `HTMLParser` → `["html", "parser"]`
/// - `PostConnection` → `["post", "connection"]`
pub fn split_camel_case(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_uppercase() {
            if !current.is_empty() {
                // If previous was lowercase, split here: "get|U" → word break before U
                if i > 0 && chars[i - 1].is_lowercase() {
                    words.push(current.to_lowercase());
                    current = String::new();
                }
                // If this is an uppercase char followed by lowercase, and we're in an
                // uppercase run, split before this char: "HTM|L|Parser" → "HTML" + "Parser"
                // The current accumulated uppercase run should stay together with this char
                // only if the NEXT char (after this one) is lowercase.
                else if i + 1 < chars.len()
                    && chars[i + 1].is_lowercase()
                    && current.chars().all(|ch| ch.is_uppercase())
                    && !current.is_empty()
                {
                    // Everything accumulated so far is one word
                    words.push(current.to_lowercase());
                    current = String::new();
                }
            }
            current.push(c);
        } else if c == '_' || c == '-' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current = String::new();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        words.push(current.to_lowercase());
    }

    words
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
        assert_eq!(
            prepare_for_index("CreatePostInput"),
            "create post input"
        );
    }
}
