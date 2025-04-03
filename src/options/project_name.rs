use crate::RoverError;
use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use anyhow::anyhow;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectNameOpt {
    #[arg(long = "project-name")]
    pub project_name: Option<String>,
}

impl ProjectNameOpt {
    pub fn get_project_name(&self) -> Option<String> {
        self.project_name.clone()
    }

    pub fn prompt_project_name(&self) -> RoverResult<String> {
        let default = self.suggest_default_name();
        let max_length = 64;
        let min_length = 2;
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+<>/?\\[]{};: ";

        loop {
            let input: Input<String> = Input::new().with_prompt("? Name your GraphQL API").default(default.to_string());
            let name = input.interact_text()?.to_string();

            let valid_name =
                self.validate_name(&name, allowed_chars, max_length, min_length);

            match valid_name {
                Ok(valid_name) => {
                    return Ok(valid_name);
                }
                Err(invalid_name) => {
                    println!("{invalid_name}")
                }
            }
        }
    }

    fn suggest_default_name(&self) -> String {
        "My API".to_string()
    }

    pub fn get_or_prompt_project_name(&self) -> RoverResult<String> {
        // If a project name was provided via command line, validate and use it
        if let Some(name) = self.get_project_name() {
            return Ok(name);
        }

        self.prompt_project_name()
    }

    pub fn validate_name(
        &self,
        input: &str,
        allowed_chars: &str,
        max_length: usize,
        min_length: usize,
    ) -> RoverResult<String> {
        // Check length
        if !validate_max_length(input, max_length) {
            return Err(RoverError::new(anyhow!(
                "Names must be a maximum of {} characters long.",
                max_length
            )));
        }

        if !validate_min_length(input, min_length) {
            return Err(RoverError::new(anyhow!(
                "Names must be a minimum of {} characters long.",
                min_length
            )));
        }

        // Check characters
        if !validate_allowed_chars(input, allowed_chars) {
            return Err(RoverError::new(anyhow!(
                "Names may only contain the following characters: {}",
                allowed_chars
            )));
        }

        Ok(input.to_string())
    }
}

fn validate_max_length(input: &str, max_length: usize) -> bool {
    if input.len() > max_length {
        return false;
    }

    return true;
}

fn validate_min_length(input: &str, min_length: usize) -> bool {
    if input.len() < min_length {
        return false;
    }

    return true;
}

fn validate_allowed_chars(input: &str, allowed_chars: &str) -> bool {
    for char in input.chars() {
        if !allowed_chars.contains(char) {
            return false;
        }
    }

    return true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_max_length_returns_false_when_input_length_is_greater_than_max_length() {
        let input = "test";
        let max_length = 3;
        let result = validate_max_length(input, max_length);

        assert_eq!(result, false);
    }

    #[test]
    fn test_validate_max_length_returns_true_when_input_length_is_equal_to_max_length() {
        let input = "test";
        let max_length = 4;
        let result = validate_max_length(input, max_length);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_max_length_returns_true_when_input_length_is_less_than_max_length() {
        let input = "test";
        let max_length = 5;
        let result = validate_max_length(input, max_length);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_min_length_returns_false_when_input_length_is_less_than_min_length() {
        let input = "t";
        let min_length = 2;
        let result = validate_min_length(input, min_length);

        assert_eq!(result, false);
    }

    #[test]
    fn test_validate_min_length_returns_true_when_input_length_is_equal_to_min_length() {
        let input = "test";
        let min_length = 4;
        let result = validate_min_length(input, min_length);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_min_length_returns_true_when_input_length_is_greater_than_min_length() {
        let input = "test";
        let min_length = 3;
        let result = validate_min_length(input, min_length);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_allowed_chars_returns_false_when_input_contains_invalid_characters() {
        let input = "test";
        let allowed_chars = "abc";
        let result = validate_allowed_chars(input, allowed_chars);

        assert_eq!(result, false);
    }

    #[test]
    fn test_validate_allowed_chars_returns_true_when_input_contains_allowed_characters() {
        let input = "test";
        let allowed_chars = "tes";
        let result = validate_allowed_chars(input, allowed_chars);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_allowed_chars_returns_true_when_input_contains_allowed_special_characters() {
        let input = "!@#$%^&*()";
        let allowed_chars = "0123456789!@#$%^&*()";
        let result = validate_allowed_chars(input, allowed_chars);

        assert_eq!(result, true);
    }

    #[test]
    fn test_validate_returns_error_when_input_has_invalid_length() {
        let instance = ProjectNameOpt {
            project_name: Some("my-project".to_string()),
        };

        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let max_length = 1;
        let min_length = 0;

        let result = instance.validate_name(input, allowed_chars, max_length, min_length);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_returns_error_when_input_has_invalid_chars() {
        let instance = ProjectNameOpt {
            project_name: Some("my-project".to_string()),
        };

        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
        let max_length = 100;
        let min_length = 1;

        let result = instance.validate_name(input, allowed_chars, max_length, min_length);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_returns_ok_when_input_has_valid_chars_and_length() {
        let instance = ProjectNameOpt {
            project_name: Some("my-project".to_string()),
        };

        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let max_length = 100;
        let min_length = 1;

        let result = instance.validate_name(input, allowed_chars, max_length, min_length);

        assert!(result.is_ok());
    }

    #[test]
    fn test_get_project_name_with_preset_value() {
        let instance = ProjectNameOpt {
            project_name: Some("my-project".to_string()),
        };

        let result = instance.get_project_name();
        assert_eq!(result, Some("my-project".to_string()));
    }

    #[test]
    fn test_suggest_default_name() {
        let instance = ProjectNameOpt { project_name: None };
        let default_name = instance.suggest_default_name();

        assert_eq!(default_name, "My API");
    }

    #[test]
    fn test_prompt_project_name() {
        let instance = ProjectNameOpt { project_name: None };
        let result = instance.prompt_project_name();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My API");
    }

    #[test]
    fn test_get_or_prompt_project_name_with_no_value() {
        let instance = ProjectNameOpt { project_name: None };
        let result = instance.get_or_prompt_project_name();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My API");
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait() {
        let default_instance = ProjectNameOpt::default();
        assert_eq!(default_instance.project_name, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = ProjectNameOpt {
            project_name: Some("test-project".to_string()),
        };
        // Check that Debug formatting doesn't panic and has the expected content
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("test-project"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectNameOpt {
            project_name: Some("clone-test".to_string()),
        };
        let cloned = original.clone();

        assert_eq!(original.project_name, cloned.project_name);
    }
}
