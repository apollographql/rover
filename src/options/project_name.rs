use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use regex::Regex;
use std::str::FromStr;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectNameOpt {
    #[arg(long = "project-name")]
    project_name: Option<ProjectName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectName(String);

impl FromStr for ProjectName {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        const MAX_LENGTH: usize = 64;
        const MIN_LENGTH:usize = 2;

        // Check if the length of the input is within the valid range
        if input.len() < MIN_LENGTH || input.len() > MAX_LENGTH {
            return Err(format!(
                "Invalid project name length: must be between {} and {} characters.",
                MIN_LENGTH, MAX_LENGTH
            ));
        }

        // Regex pattern for allowed characters.
        let pattern = r"^[a-zA-Z0-9\-!@#$%^&*()_+<>/?\\\[\]{};: ]+$";

        let re = Regex::new(pattern).unwrap();

        // Check if the input string matches the regex.
        if re.is_match(input) {
            // If the input matches, wrap it in the ProjectName struct.
            Ok(ProjectName(input.to_string()))
        } else {
            // If the input doesn't match, return an error.
            Err(format!("Invalid project name: '{}'", input))
        }
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ProjectNameOpt {
    pub fn get_project_name(&self) -> Option<ProjectName> {
        self.project_name.clone()
    }

    pub fn prompt_project_name(&self) -> RoverResult<ProjectName> {
        let default = self.suggest_default_name();

        loop {
            // Prompt for user input
            let input: Input<String> = Input::new().with_prompt("? Name your GraphQL API").default(default.clone());
            let input_name = input.interact_text().map_err(|e| e.to_string()).unwrap();

            // Try to parse the input into a ProjectName
            let project_name: Result<ProjectName, _> = input_name.parse();

            // Check for a valid project name
            match project_name {
                Ok(name) => return Ok(name),
                Err(err) => {
                    eprintln!("{}", err); // Print the error and continue the loop for another attempt
                }
            }
        }
    }

    fn suggest_default_name(&self) -> String {
        "My API".to_string()
    }

    pub fn get_or_prompt_project_name(&self) -> RoverResult<ProjectName> {
        // If a project name was provided via command line, validate and use it
        if let Some(name) = self.get_project_name() {
            return Ok(name);
        }

        self.prompt_project_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_errors_when_input_length_is_greater_than_max_length() {
        let result: Result<ProjectName, _> = "This is a string that contains more than sixty-five characters!".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_errors_when_input_length_is_less_than_min_length() {
        let result: Result<ProjectName, _> = "x".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_errors_when_input_includes_invalid_char() {
        let result: Result<ProjectName, _> = "\"".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ok_when_input_includes_valid_chars_and_is_valid_length() {
        let result: Result<ProjectName, _> = "My Project".parse();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_project_name_with_preset_value() {
        let instance = ProjectNameOpt {
            project_name: "My Project".parse::<ProjectName>().ok(),
        };

        let result = instance.get_project_name();
        assert_eq!(result, "My Project".parse::<ProjectName>().ok());
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
        assert_eq!(result.unwrap(), "My API".parse::<ProjectName>().ok().unwrap());
    }

    #[test]
    fn test_get_or_prompt_project_name_with_no_value() {
        let instance = ProjectNameOpt { project_name: None };
        let result = instance.get_or_prompt_project_name();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My API".parse::<ProjectName>().ok().unwrap());
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
            project_name: "test-project".parse::<ProjectName>().ok(),
        };
        // Check that Debug formatting doesn't panic and has the expected content
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("test-project"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectNameOpt {
            project_name: "clone-project".parse::<ProjectName>().ok(),
        };
        let cloned = original.clone();

        assert_eq!(original.project_name, cloned.project_name);
    }
}
