use crate::command::init::template_fetcher::{Template, TemplateId};
use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::Parser;
use clap::arg;
use dialoguer::Select;
use dialoguer::console::Term;
use rover_std::Style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectTemplateOpt {
    #[arg(long = "template")]
    pub template: Option<TemplateId>,

    /// Add MCP server capabilities to existing project  
    #[arg(long = "mcp")]
    pub mcp: bool,
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

        // Generate MCP variants of templates and combine them
        let all_templates = Self::generate_template_variants(templates);

        // let user select from list of templates (including MCP variants)
        let template_display_names = all_templates
            .iter()
            .map(|t| t.display_name.as_str())
            .collect::<Vec<_>>();
        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select a template"))
            .items(&template_display_names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => {
                let selected_template = &all_templates[index];
                // Don't allow selecting the separator
                if selected_template.id.0 == "separator" {
                    return Err(RoverError::new(anyhow!("Invalid template selection")));
                }
                Ok(TemplateId(selected_template.id.to_string()))
            }
            None => Err(RoverError::new(anyhow!("No template selected"))),
        }
    }

    /// Generate MCP variants of existing templates
    fn generate_template_variants(base_templates: &[Template]) -> Vec<Template> {
        let mut all_templates = base_templates.to_vec();

        // Add separator template for visual grouping
        all_templates.push(Template {
            id: TemplateId("separator".to_string()),
            display_name: "─────────────────────────────────".to_string(),
            path: "".to_string(),
            language: "".to_string(),
            federation_version: "".to_string(),
            max_schema_depth: 0,
            routing_url: "".to_string(),
            commands: None,
            start_point_file: "".to_string(),
            print_depth: None,
        });

        // Generate MCP variants
        for base_template in base_templates {
            // Skip certain templates that don't make sense for MCP
            if base_template.id.0.contains("minimal") || base_template.id.0.contains("test") {
                continue;
            }

            let mcp_template = Template {
                id: TemplateId(format!("mcp-{}", base_template.id.0)),
                display_name: format!("{} + AI tools", base_template.display_name),
                path: base_template.path.clone(),
                language: base_template.language.clone(),
                federation_version: base_template.federation_version.clone(),
                max_schema_depth: base_template.max_schema_depth,
                routing_url: base_template.routing_url.clone(),
                commands: base_template.commands.clone(),
                start_point_file: base_template.start_point_file.clone(),
                print_depth: base_template.print_depth,
            };
            all_templates.push(mcp_template);
        }

        all_templates
    }

    pub fn get_or_prompt_template(&self, templates: &[Template]) -> RoverResult<TemplateId> {
        // Generate all template variants (including MCP) for validation
        let all_templates = Self::generate_template_variants(templates);
        let template_ids = all_templates
            .iter()
            .map(|t| t.id.to_string())
            .collect::<Vec<_>>();

        if let Some(template) = self.get_template() {
            // Check if the specified template exists (could be base or MCP variant)
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

    /// Check if a template is an MCP variant
    pub fn is_mcp_template(template_id: &TemplateId) -> bool {
        template_id.0.starts_with("mcp-")
    }

    /// Get base template ID from MCP template ID
    pub fn get_base_template_id(template_id: &TemplateId) -> TemplateId {
        if Self::is_mcp_template(template_id) {
            TemplateId(
                template_id
                    .0
                    .strip_prefix("mcp-")
                    .unwrap_or(&template_id.0)
                    .to_string(),
            )
        } else {
            template_id.clone()
        }
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
        assert_eq!(default_instance.mcp, false);
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
