use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use rover_std::Style;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct SchemaNameOpt {
    #[arg(long = "schema-name", short = 's')]
    pub(crate) schema_name: Option<SchemaName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaName(String);

impl FromStr for SchemaName {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        const MAX_LENGTH: usize = 64;
        const MIN_LENGTH: usize = 2;

        // Check if the length of the input is within the valid range
        if input.len() < MIN_LENGTH || input.len() > MAX_LENGTH {
            return Err(format!(
                "Invalid schema name length: must be between {MIN_LENGTH} and {MAX_LENGTH} characters."
            ));
        }

        Ok(SchemaName(input.to_string()))
    }
}

impl fmt::Display for SchemaName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SchemaNameOpt {
    pub fn get_schema_name(&self) -> Option<SchemaName> {
        self.schema_name.clone()
    }

    pub fn prompt_schema_name(&self) -> RoverResult<SchemaName> {
        let default = self.suggest_default_name();

        loop {
            // Prompt for user input
            let input: Input<String> = Input::new()
                .with_prompt(Style::Prompt.paint("? Name your schema"))
                .with_initial_text(default.clone());
            let input_name = input.interact_text().map_err(|e| e.to_string()).unwrap();

            // Try to parse the input into a SchemaName
            let schema_name: Result<SchemaName, _> = input_name.parse();

            // Check for a valid project name
            match schema_name {
                Ok(name) => return Ok(name),
                Err(err) => {
                    eprintln!("{err}"); // Print the error and continue the loop for another attempt
                }
            }
        }
    }

    fn suggest_default_name(&self) -> String {
        "main".to_string()
    }

    pub fn get_or_prompt_schema_name(&self) -> RoverResult<SchemaName> {
        // If a schema name was provided via command line, validate and use it
        if let Some(name) = self.get_schema_name() {
            return Ok(name);
        }

        self.prompt_schema_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_errors_when_input_length_is_greater_than_max_length() {
        let result: Result<SchemaName, _> =
            "This string contains definitely more than sixty-four characters!!".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_errors_when_input_length_is_less_than_min_length() {
        let result: Result<SchemaName, _> = "x".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ok_when_input_includes_valid_chars_and_is_valid_length() {
        let result: Result<SchemaName, _> = "products".parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_schema_name_with_preset_value() {
        let instance = SchemaNameOpt {
            schema_name: "products".parse::<SchemaName>().ok(),
        };

        let result = instance.get_schema_name();
        assert_eq!(result, "products".parse::<SchemaName>().ok());
    }

    #[test]
    fn test_suggest_default_name() {
        let instance = SchemaNameOpt { schema_name: None };
        let default_name = instance.suggest_default_name();

        assert_eq!(default_name, "main");
    }

    // Default trait implementation tests
    #[test]
    fn test_default_trait() {
        let default_instance = SchemaNameOpt::default();
        assert_eq!(default_instance.schema_name, None);
    }

    // Derived trait tests (Debug, Clone, etc.)
    #[test]
    fn test_debug_trait() {
        let instance = SchemaNameOpt {
            schema_name: "test-schema".parse::<SchemaName>().ok(),
        };
        // Check that Debug formatting doesn't panic and has the expected content
        let debug_str = format!("{instance:?}");
        assert!(debug_str.contains("test-schema"));
    }

    #[test]
    fn test_clone_trait() {
        let original = SchemaNameOpt {
            schema_name: "clone-schema".parse::<SchemaName>().ok(),
        };
        let cloned = original.clone();

        assert_eq!(original.schema_name, cloned.schema_name);
    }
}
