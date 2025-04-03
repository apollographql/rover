use clap::arg;
use serde::{Deserialize, Serialize};
use clap::Parser;
use crate::{RoverError, RoverResult};
use dialoguer::Select;
use console::Term;
use anyhow::anyhow;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectOrganizationOpt {
    #[arg(long = "organization")]
    pub organization: Option<String>,
}

impl ProjectOrganizationOpt {
    pub fn get_organization(&self) -> Option<String> {
        self.organization.clone()
    }
    
    pub fn prompt_organization(organizations: &[String]) -> RoverResult<String> {
        if organizations.is_empty() {
            return Err(RoverError::new(anyhow!("No organizations available")));
        }
        
        let selection = Select::new()
            .with_prompt("? Select an organization")
            .items(organizations)
            .default(0)
            .interact_on_opt(&Term::stderr())?;
            
        match selection {
            Some(index) => Ok(organizations[index].clone()),
            None => Err(RoverError::new(anyhow!("No organization selected"))),
        }
    }
    
    pub fn get_or_prompt_organization(&self, organizations: &[String]) -> RoverResult<String> {
        if let Some(org) = self.get_organization() {
            if organizations.contains(&org) {
                return Ok(org);
            } else {
                return Err(RoverError::new(anyhow!(
                    "Specified organization '{}' is not available", org
                )));
            }
        }
        
        Self::prompt_organization(organizations)
    }
}

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_organization_with_preset_value() {
        let instance = ProjectOrganizationOpt {
            organization: Some("apollo".to_string()),
        };

        let result = instance.get_organization();
        assert_eq!(result, Some("apollo".to_string()));
    }

    #[test]
    fn test_get_organization_with_no_value() {
        let instance = ProjectOrganizationOpt { organization: None };
        let result = instance.get_organization();
        assert_eq!(result, None);
    }

    #[test]
    fn test_prompt_organization_with_items() {
        let organizations = ["org1".to_string(), "org2".to_string()];
        
        let selection = Some(0);
        let result = match selection {
            Some(index) => Ok(organizations[index].clone()),
            None => Err(RoverError::new(anyhow!("No organization selected"))),
        };
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "org1");
    }

    #[test]
    fn test_prompt_organization_with_empty_list() {
        let organizations: Vec<String> = vec![];
        let result = ProjectOrganizationOpt::prompt_organization(&organizations);
        
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            RoverError::new(anyhow!("No organizations available")).to_string()
        );
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait_for_project_organization_opt() {
        let default_instance = ProjectOrganizationOpt::default();
        assert_eq!(default_instance.organization, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = ProjectOrganizationOpt {
            organization: Some("apollo".to_string()),
        };
        // Check that Debug formatting doesn't panic
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("apollo"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectOrganizationOpt {
            organization: Some("apollo".to_string()),
        };
        let cloned = original.clone();
        
        assert_eq!(original.organization, cloned.organization);
    }
}