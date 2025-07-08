use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use rover_std::Style;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ProjectMockingSetupOpt {
    #[arg(long = "ai-mocking", short = 'm', value_enum)]
    pub ai_mocking: Option<MockingSetup>,
}

impl ProjectMockingSetupOpt {
    pub fn get_or_prompt_mocking_setup(&self) -> RoverResult<MockingSetup> {
        if let Some(mocking_setup) = &self.ai_mocking {
            Ok(mocking_setup.clone())
        } else {
            let mocking_options = <MockingSetup as ValueEnum>::value_variants();

            println!();
            println!("🚀 {}", Style::Heading.paint("AI-Powered Mock Data"));
            println!("Transform your development experience with intelligent mock data generation.");
            println!();
            println!("Instead of writing hundreds of lines of mock data or settling for generic");
            println!("\"lorem ipsum\" placeholders, get realistic, contextual data that fits your app's");
            println!("domain automatically. This eliminates:");
            println!();
            println!("• {} - No more hours spent manually crafting mock data", Style::Success.paint("Time waste"));
            println!("• {} - Catch edge cases with realistic data variations", Style::Success.paint("Testing gaps"));
            println!("• {} - Professional demos with believable content", Style::Success.paint("Demo friction"));
            println!("• {} - Focus on features, not data maintenance", Style::Success.paint("Context switching"));
            println!();

            let selection = Select::new()
                .with_prompt(Style::Prompt.paint("? Would you like AI-powered mock data for faster development?"))
                .items(mocking_options)
                .default(0)
                .interact_on_opt(&Term::stderr())?;

            self.handle_mocking_selection(mocking_options, selection)
        }
    }

    pub fn handle_mocking_selection(
        &self,
        mocking_options: &[MockingSetup],
        selection: Option<usize>,
    ) -> RoverResult<MockingSetup> {
        match selection {
            Some(index) => Ok(mocking_options[index].clone()),
            None => Err(RoverError::new(anyhow!("No mocking option selected"))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum MockingSetup {
    Yes,
    No,
}

impl Display for MockingSetup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use MockingSetup::*;
        let readable = match self {
            Yes => "Yes - Set up AI-powered mock data",
            No => "No - I'll handle mock data myself",
        };
        write!(f, "{readable}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mocking_setup_with_some_value() {
        let instance = ProjectMockingSetupOpt {
            ai_mocking: Some(MockingSetup::Yes),
        };

        let result = instance.get_or_prompt_mocking_setup();

        assert!(result.is_ok());
        let mocking_setup = result.unwrap();
        assert_eq!(mocking_setup, MockingSetup::Yes);
    }

    #[test]
    fn test_handle_mocking_selection_returns_setup_with_some_selection() {
        let instance = ProjectMockingSetupOpt {
            ai_mocking: None,
        };

        let mocking_options = <MockingSetup as ValueEnum>::value_variants();
        let selection: usize = 0;
        let result = instance.handle_mocking_selection(mocking_options, Some(selection));

        assert!(result.is_ok());
        let mocking_setup = result.unwrap();
        assert_eq!(mocking_setup, mocking_options[selection].clone());
    }

    #[test]
    fn test_handle_mocking_selection_returns_error_with_none_selection() {
        let instance = ProjectMockingSetupOpt {
            ai_mocking: None,
        };

        let mocking_options = <MockingSetup as ValueEnum>::value_variants();
        let result = instance.handle_mocking_selection(mocking_options, None);

        assert!(result.is_err());
    }
}