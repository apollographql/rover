/// Converts a string to a slug format
pub fn slugify(input: &str) -> String {
    let mut result = input.to_lowercase().replace(' ', "-");

    // Replace consecutive hyphens with a single hyphen
    while result.contains("--") {
        result = result.replace("--", "-");
    }

    // Remove leading and trailing hyphens
    result = result
        .trim_start_matches('-')
        .trim_end_matches('-')
        .to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        let test_cases = vec![
            ("Hello World", "hello-world"),
            ("  Spaced  ", "spaced"),
            ("Multiple--Hyphens", "multiple-hyphens"),
            ("-trim-hyphens-", "trim-hyphens"),
            ("LOWERCASE", "lowercase"),
            ("no changes", "no-changes"),
            ("--double--hyphens--", "double-hyphens"),
        ];

        for (input, expected) in test_cases {
            assert_eq!(
                slugify(input),
                expected,
                "Expected slugify('{input}') to be '{expected}'"
            );
        }
    }
}
