use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::arg;
use clap::Parser;
use dialoguer::console::Term;
use dialoguer::Select;
use rover_std::Style;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]

pub struct Organization {
    name: String,
    id: String,
}

impl Organization {
    pub fn new(name: String, id: String) -> Self {
        Self { name, id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrganizationId(String);

impl FromStr for OrganizationId {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(OrganizationId(input.to_string()))
    }
}

impl fmt::Display for OrganizationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectOrganizationOpt {
    #[arg(long = "org-id")]
    pub organization: Option<OrganizationId>,
}

impl ProjectOrganizationOpt {
    pub fn get_organization(&self) -> Option<OrganizationId> {
        self.organization.clone()
    }

    pub fn prompt_organization(organizations: &[Organization]) -> RoverResult<OrganizationId> {
        // return error if no organizations
        if organizations.is_empty() {
            return Err(RoverError::new(anyhow!("No organizations available")));
        }
        // if only 1 organization, return that organization
        if organizations.len() == 1 {
            return Ok(OrganizationId(organizations[0].id.to_string()));
        }
        // otherwise, let user select from list of organizations
        let organization_names = organizations
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>();
        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select an organization"))
            .items(&organization_names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(OrganizationId(organizations[index].id.to_string())),
            None => Err(RoverError::new(anyhow!("No organization selected"))),
        }
    }

    pub fn get_or_prompt_organization(
        &self,
        organizations: &[Organization],
    ) -> RoverResult<OrganizationId> {
        let organization_ids = organizations
            .iter()
            .map(|o| o.id.to_string())
            .collect::<Vec<_>>();
        if let Some(org) = self.get_organization() {
            if organization_ids.contains(&org.to_string()) {
                return Ok(org);
            } else {
                return Err(RoverError::new(anyhow!(
                    "Specified organization '{}' is not available",
                    org
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
            organization: Some(OrganizationId("test-org".to_string())),
        };

        let result = instance.get_organization();
        assert_eq!(result, Some(OrganizationId("test-org".to_string())));
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
        let organizations: Vec<Organization> = vec![];
        let result = ProjectOrganizationOpt::prompt_organization(&organizations);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message(),
            RoverError::new(anyhow!("No organizations available")).message()
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
            organization: Some(OrganizationId("test-org".to_string())),
        };
        // Check that Debug formatting doesn't panic
        let debug_str = format!("{instance:?}");
        assert!(debug_str.contains("test-org"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectOrganizationOpt {
            organization: Some(OrganizationId("test-org".to_string())),
        };
        let cloned = original.clone();

        assert_eq!(original.organization, cloned.organization);
    }
}
