use crate::command::init::template_fetcher::{Template, TemplateId};
use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::arg;
use clap::Parser;
use console::Term;
use dialoguer::Select;
use rover_std::Style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectTemplateOpt {
    #[arg(long = "template")]
    pub template: Option<TemplateId>,
}

impl ProjectTemplateOpt {
    pub fn get_template(&self) -> Option<TemplateId> {
        self.template.clone()
    }

    pub fn prompt_template(templates: &[Template]) -> RoverResult<TemplateId> {
        // return error if no templates
        if templates.is_empty() {
            return Err(RoverError::new(anyhow!("No templates available")));
        }

        // let user select from list of templates
        let template_display_names = templates
            .iter()
            .map(|t| t.display_name.as_str())
            .collect::<Vec<_>>();
        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select a language and server library template"))
            .items(&template_display_names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(TemplateId(templates[index].id.to_string())),
            None => Err(RoverError::new(anyhow!("No template selected"))),
        }
    }

    pub fn get_or_prompt_template(&self, templates: &[Template]) -> RoverResult<TemplateId> {
        let template_ids = templates
            .iter()
            .map(|t| t.id.to_string())
            .collect::<Vec<_>>();
        if let Some(template) = self.get_template() {
            if template_ids.contains(&template.to_string()) {
                return Ok(template);
            } else {
                return Err(RoverError::new(anyhow!(
                    "Specified template '{}' is not available",
                    template
                )));
            }
        }

        Self::prompt_template(templates)
    }
}

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_template_with_preset_value() {
        let instance = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
        };

        let result = instance.get_template();
        assert_eq!(result, Some(TemplateId("test-template".to_string())));
    }

    #[test]
    fn test_get_template_with_no_value() {
        let instance = ProjectTemplateOpt { template: None };
        let result = instance.get_template();
        assert_eq!(result, None);
    }

    #[test]
    fn test_prompt_template_with_items() {
        let templates = ["template-1".to_string(), "template-2".to_string()];

        let selection = Some(0);
        let result = match selection {
            Some(index) => Ok(templates[index].clone()),
            None => Err(RoverError::new(anyhow!("No template selected"))),
        };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "template-1");
    }

    #[test]
    fn test_prompt_template_with_empty_list() {
        let templates: Vec<Template> = vec![];
        let result = ProjectTemplateOpt::prompt_template(&templates);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().message(),
            RoverError::new(anyhow!("No templates available")).message()
        );
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait_for_project_template_opt() {
        let default_instance = ProjectTemplateOpt::default();
        assert_eq!(default_instance.template, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
        };
        // Check that Debug formatting doesn't panic
        let debug_str = format!("{instance:?}");
        assert!(debug_str.contains("test-template"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
        };
        let cloned = original.clone();

        assert_eq!(original.template, cloned.template);
    }
}
