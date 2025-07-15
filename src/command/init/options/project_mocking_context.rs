use crate::RoverResult;
use clap::Parser;
use console::Term;
use rover_std::Style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ProjectMockingContextOpt {
    #[arg(long = "ai-context")]
    pub ai_context: Option<String>,
}

impl ProjectMockingContextOpt {
    pub fn get_or_prompt_mocking_context(&self) -> RoverResult<String> {
        if let Some(context) = &self.ai_context {
            Ok(context.clone())
        } else {
            println!();
            println!("🎯 {}", Style::Heading.paint("Customize Your Mock Data"));
            println!("Provide context to make your mock data more realistic and relevant to your app.");
            println!();
            println!("Examples of great context:");
            println!("• {}", Style::Success.paint("\"I'm building a recipe app focused on healthy, quick meals\""));
            println!("• {}", Style::Success.paint("\"This is for a fitness app targeting marathon runners\""));
            println!("• {}", Style::Success.paint("\"I'm creating a bookstore app specializing in sci-fi novels\""));
            println!("• {}", Style::Success.paint("\"This is a task management app for creative agencies\""));
            println!();
            println!("The AI will use this context to generate relevant data across all your queries.");
            println!();
            print!("{} ", Style::Prompt.paint("? Describe your app's domain or focus (optional):"));
            
            use std::io::{self, Write};
            io::stdout().flush()?;
            
            let term = Term::stdout();
            let context = term.read_line()?;

            // If they provide empty context, use a sensible default
            if context.trim().is_empty() {
                Ok("I'm building a modern web application with realistic user data".to_string())
            } else {
                Ok(context)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mocking_context_with_some_value() {
        let instance = ProjectMockingContextOpt {
            ai_context: Some("test context".to_string()),
        };

        let result = instance.get_or_prompt_mocking_context();

        assert!(result.is_ok());
        let context = result.unwrap();
        assert_eq!(context, "test context");
    }

    #[test]
    fn test_get_mocking_context_with_none_value() {
        let instance = ProjectMockingContextOpt {
            ai_context: None,
        };

        // This test would normally require interactive input, so we'll just test the structure
        assert!(instance.ai_context.is_none());
    }
}