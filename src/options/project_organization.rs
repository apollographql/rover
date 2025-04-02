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