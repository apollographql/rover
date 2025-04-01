use std::fmt;

use anyhow::anyhow;
use dialoguer::Input;

use crate::{RoverError, RoverResult};

pub struct Project {
    _name: ValidProjectName,
}

impl Project {
    pub fn prompt_for_valid_project_name() -> RoverResult<ValidProjectName> {
        let max_length = 64;
        let min_length = 2;
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+<>/?\\";

       loop {
            let input: Input<String> = Input::new().with_prompt("? Name your GraphQL API");
            let name = input.interact_text()?.to_string();

            let valid_name = ValidProjectName::validate(&name, allowed_chars, max_length, min_length);

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
}

pub struct ValidProjectName(String);

impl ValidProjectName {
    pub fn validate(input: &str, allowed_chars: &str, max_length: usize, min_length: usize) -> RoverResult<ValidProjectName> {
        // Check length
        if !validate_max_length(input, max_length) {
            return Err(RoverError::new(anyhow!("Names must be a maximum of {} characters long.", max_length)));
        }

        if !validate_min_length(input, min_length) {
            return Err(RoverError::new(anyhow!("Names must be a minimum of {} characters long.", min_length)));
        }

        // Check characters
        if !validate_allowed_chars(input, allowed_chars) {
            return Err(RoverError::new(anyhow!("Names may only contain the following characters: {}", allowed_chars)));
        }

        Ok(ValidProjectName(input.to_string()))
    }
}

impl fmt::Display for ValidProjectName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
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
        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let max_length =  1;
        let min_length = 0;

        let result = ValidProjectName::validate(input, allowed_chars, max_length, min_length);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_returns_error_when_input_has_invalid_chars() {
        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890";
        let max_length =  100;
        let min_length =  1;

        let result = ValidProjectName::validate(input, allowed_chars, max_length, min_length);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_returns_ok_when_input_has_valid_chars_and_length() {
        let input = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let allowed_chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890-!@#$%^&*()_+";
        let max_length =  100;
        let min_length =  1;

        let result = ValidProjectName::validate(input, allowed_chars, max_length, min_length);

        assert!(result.is_ok());
    }
}