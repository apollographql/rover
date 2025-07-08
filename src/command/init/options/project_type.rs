use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use dialoguer::console::Term;
use dialoguer::Select;
use rover_std::Style;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectTypeOpt {
    #[arg(long = "project-type", short = 't', value_enum)]
    pub project_type: Option<ProjectType>,
}

impl ProjectTypeOpt {
    pub fn get_project_type(&self) -> Option<ProjectType> {
        self.project_type.clone()
    }

    pub fn prompt_project_type(&self) -> RoverResult<ProjectType> {
        let project_types = <ProjectType as ValueEnum>::value_variants();
        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select option"))
            .items(project_types)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        self.handle_project_type_selection(project_types, selection)
    }

    fn handle_project_type_selection(
        &self,
        project_types: &[ProjectType],
        selection: Option<usize>,
    ) -> RoverResult<ProjectType> {
        match selection {
            Some(index) => Ok(project_types[index].clone()),
            None => Err(RoverError::new(anyhow!("No project type selected"))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectType {
    CreateNew,
    AddSubgraph,
}

impl Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProjectType::*;
        let readable = match self {
            CreateNew => "Create a new graph",
            AddSubgraph => "Add a subgraph to an existing graph",
        };
        write!(f, "{readable}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_project_type_with_preset_value() {
        let instance = ProjectTypeOpt {
            project_type: Some(ProjectType::CreateNew),
        };

        let result = instance.get_project_type();
        assert_eq!(result, Some(ProjectType::CreateNew));
    }

    #[test]
    fn test_get_project_type_with_no_value() {
        let instance = ProjectTypeOpt { project_type: None };
        let result = instance.get_project_type();
        assert_eq!(result, None);
    }

    #[test]
    fn test_handle_project_type_selection_with_valid_selection() {
        let instance = ProjectTypeOpt { project_type: None };

        let project_types = <ProjectType as ValueEnum>::value_variants();
        let result = instance.handle_project_type_selection(project_types, Some(0));

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ProjectType::CreateNew);
    }

    #[test]
    fn test_handle_project_type_selection_with_invalid_selection() {
        let instance = ProjectTypeOpt { project_type: None };

        let project_types = <ProjectType as ValueEnum>::value_variants();
        let result = instance.handle_project_type_selection(project_types, None);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            RoverError::new(anyhow!("No project type selected")).to_string()
        );
    }

    // Display trait implementation tests

    #[test]
    fn test_display_trait_for_create_new() {
        let project_type = ProjectType::CreateNew;
        assert_eq!(project_type.to_string(), "Create a new graph");
    }

    #[test]
    fn test_display_trait_for_add_subgraph() {
        let project_type = ProjectType::AddSubgraph;
        assert_eq!(
            project_type.to_string(),
            "Add a subgraph to an existing graph"
        );
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait_for_project_type_opt() {
        let default_instance = ProjectTypeOpt::default();
        assert_eq!(default_instance.project_type, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait_for_project_type() {
        let project_type = ProjectType::CreateNew;
        // Check that Debug formatting doesn't panic
        let debug_str = format!("{project_type:?}");
        assert!(debug_str.contains("CreateNew"));
    }

    #[test]
    fn test_clone_trait_for_project_type() {
        let original = ProjectType::CreateNew;
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_clone_trait_for_project_type_opt() {
        let original = ProjectTypeOpt {
            project_type: Some(ProjectType::CreateNew),
        };
        let cloned = original.clone();

        assert_eq!(original.project_type, cloned.project_type);
    }

    #[test]
    fn test_value_enum_variants() {
        let variants = <ProjectType as ValueEnum>::value_variants();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], ProjectType::CreateNew);
        assert_eq!(variants[1], ProjectType::AddSubgraph);
    }

    #[test]
    fn test_value_enum_to_possible_value() {
        let possible_value = ProjectType::CreateNew.to_possible_value();
        assert!(possible_value.is_some());
        let value = possible_value.unwrap();
        assert_eq!(value.get_name(), "create-new");
    }
}
