use super::utils::random::RandomStringGenerator;
use super::utils::strings::slugify;
use crate::command::init::graph_id::GraphId;

const GRAPH_ID_MAX_CHAR: usize = 27;
const UNIQUE_STRING_LENGTH: usize = 7;

/// Generates a graph ID based on a name
/// while meeting all validation rules
pub fn generate_graph_id<T: RandomStringGenerator>(
    graph_name: &str,
    random_generator: &mut T,
    user_provided_id: Option<String>,
) -> GraphId {
    // If user manually provided an ID, format it with
    // a slug and correct character length
    // and return it if it is valid (if invalid, return a default)
    if let Some(id) = user_provided_id {
        let slugified_id = slugify(&id);
        return slugified_id[..slugified_id.len().min(GRAPH_ID_MAX_CHAR)]
            .parse()
            .unwrap_or_else(|_| generate_default_graph_id(graph_name, random_generator));
    }

    // Otherwise, generate an ID
    generate_default_graph_id(graph_name, random_generator)
}

/// Generate a default graph ID with random suffix
fn generate_default_graph_id<T: RandomStringGenerator>(
    graph_name: &str,
    random_generator: &mut T,
) -> GraphId {
    let mut slugified_name = slugify(graph_name);

    let alphabetic_start_index = slugified_name
        .chars()
        .position(|c| c.is_alphabetic())
        .unwrap_or(slugified_name.len());
    slugified_name = slugified_name[alphabetic_start_index..].to_string();

    // Use "id" if name is empty
    let name_part = if slugified_name.is_empty() {
        "id".to_string()
    } else {
        let unique_string_length = UNIQUE_STRING_LENGTH + 1; // +1 for hyphen
        let max_name_length = if GRAPH_ID_MAX_CHAR > unique_string_length {
            GRAPH_ID_MAX_CHAR - unique_string_length
        } else {
            0
        };
        slugified_name[..slugified_name.len().min(max_name_length)].to_string()
    };

    // Generate and append random suffix
    let unique_string = random_generator.generate_string(UNIQUE_STRING_LENGTH);
    let result = format!("{}-{}", name_part, unique_string);

    // Ensure final ID is no longer than maximum length
    let final_result = slugify(&result);
    final_result[..final_result.len().min(GRAPH_ID_MAX_CHAR)]
        .parse()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::init::graph_id::utils::random::TestRandomStringGenerator;
    use std::str::FromStr;

    #[test]
    fn test_generate_graph_id() {
        let mut generator = TestRandomStringGenerator {
            value: "teststr".to_string(),
        };

        // Test with normal name
        assert_eq!(
            generate_graph_id("My Test API", &mut generator, None),
            GraphId::from_str("my-test-api-teststr").unwrap()
        );

        // Name starting with non-alphabetic
        assert_eq!(
            generate_graph_id("123My API", &mut generator, None),
            GraphId::from_str("my-api-teststr").unwrap()
        );

        // Empty string
        assert_eq!(
            generate_graph_id("", &mut generator, None),
            GraphId::from_str("id-teststr").unwrap()
        );

        // Very long name (should be truncated)
        let long_name = "a".repeat(100);
        let result = generate_graph_id(&long_name, &mut generator, None);
        assert!(result.as_str().len() <= GRAPH_ID_MAX_CHAR);
        assert!(result.as_str().ends_with("-teststr"));

        // Test with user-provided ID
        assert_eq!(
            generate_graph_id(
                "Ignored Name",
                &mut generator,
                Some("custom-id".to_string())
            ),
            GraphId::from_str("custom-id").unwrap()
        );
    }
}
