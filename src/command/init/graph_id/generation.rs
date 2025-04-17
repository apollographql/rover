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

    // Total max length (27) - random suffix (7) - hyphen (1) = 19 chars for name
    const MAX_NAME_LENGTH: usize = GRAPH_ID_MAX_CHAR - UNIQUE_STRING_LENGTH - 1;

    // Slugify the name and find the first alphabetic character
    let mut slugified_name = slugify(graph_name);
    let alphabetic_start_index = slugified_name
        .chars()
        .position(|c| c.is_alphabetic())
        .unwrap_or(slugified_name.len());
    slugified_name = slugified_name[alphabetic_start_index..].to_string();

    // Use "id" if name is empty or doesn't start with a letter
    let name_part = if slugified_name.is_empty() {
        "id".to_string()
    } else {
        slugified_name.chars().take(MAX_NAME_LENGTH).collect()
    };

    // Generate and append random suffix
    let unique_string = random_generator.generate_string(UNIQUE_STRING_LENGTH);
    let result = format!("{}-{}", name_part, unique_string);

    // Parse the result... this should always succeed since we've ensured:
    // 1. It starts with a letter (either from name or "id")
    // 2. It's within length limits
    // 3. It only contains valid characters (from slugify)
    result.parse().expect("This should not fail as we've ensured all validation rules are met")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::init::graph_id::utils::random::TestRandomStringGenerator;

    #[test]
    fn test_generate_graph_id() {
        let mut generator = TestRandomStringGenerator {
            value: "teststr".to_string(),
        };

        // Test with normal name
        assert_eq!(
            generate_graph_id("My Test API", &mut generator, None),
            "my-test-api-teststr".parse::<GraphId>().unwrap()
        );

        // Name starting with non-alphabetic
        assert_eq!(
            generate_graph_id("123My API", &mut generator, None),
            "my-api-teststr".parse::<GraphId>().unwrap()
        );

        // Empty string
        assert_eq!(
            generate_graph_id("", &mut generator, None),
            "id-teststr".parse::<GraphId>().unwrap()
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
            "custom-id".parse::<GraphId>().unwrap()
        );
    }
}
