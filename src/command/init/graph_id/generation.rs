use super::utils::random::RandomStringGenerator;
use super::utils::strings::slugify;

const GRAPH_ID_MAX_CHAR: usize = 27;
const UNIQUE_STRING_LENGTH: usize = 7;

/// Generates a graph ID based on a name
/// while meeting all validation rules
pub fn generate_graph_id<T: RandomStringGenerator>(
    graph_name: &str,
    random_generator: &mut T,
) -> String {
    let mut slugified_name = slugify(graph_name);

    let alphabetic_start_index = slugified_name
        .chars()
        .position(|c| c.is_alphabetic())
        .unwrap_or(slugified_name.len());
    slugified_name = slugified_name[alphabetic_start_index..].to_string();

    let unique_string = random_generator.generate_string(UNIQUE_STRING_LENGTH);
    let unique_string_length = unique_string.len() + 1; // +1 for hyphen

    let max_name_length = if GRAPH_ID_MAX_CHAR > unique_string_length {
        GRAPH_ID_MAX_CHAR - unique_string_length
    } else {
        0
    };

    let name_part = slugified_name[..slugified_name.len().min(max_name_length)].to_string();

    // Add "id" if name is empty
    let name_part = if name_part.is_empty() {
        "id".to_string()
    } else {
        name_part
    };

    // Append unique string to suggested id
    let result = format!("{}-{}", name_part, unique_string);

    // Ensure generated id does not pass max length
    let final_result = slugify(&result);
    final_result[..final_result.len().min(GRAPH_ID_MAX_CHAR)].to_string()
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
            generate_graph_id("My Test API", &mut generator),
            "my-test-api-teststr"
        );

        // Name starting with non-alphabetic
        assert_eq!(
            generate_graph_id("123My API", &mut generator),
            "my-api-teststr"
        );

        // Empty string
        assert_eq!(generate_graph_id("", &mut generator), "id-teststr");

        // Very long name (should be truncated)
        let long_name = "a".repeat(100);
        let result = generate_graph_id(&long_name, &mut generator);
        assert!(result.len() <= GRAPH_ID_MAX_CHAR);
        assert!(result.ends_with("-teststr"));
    }
}
