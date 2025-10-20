use crate::command::init::template_fetcher::TemplateId;
use clap::Parser;
use clap::arg;
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
}

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_template_with_preset_value() {
        let instance = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
            mcp: false,
        };

        let result = instance.get_template();
        assert_eq!(result, Some(TemplateId("test-template".to_string())));
    }

    #[test]
    fn test_get_template_with_no_value() {
        let instance = ProjectTemplateOpt {
            template: None,
            mcp: false,
        };
        let result = instance.get_template();
        assert_eq!(result, None);
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait_for_project_template_opt() {
        let default_instance = ProjectTemplateOpt::default();
        assert_eq!(default_instance.template, None);
        assert!(!default_instance.mcp);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
            mcp: false,
        };
        // Check that Debug formatting doesn't panic
        let debug_str = format!("{instance:?}");
        assert!(debug_str.contains("test-template"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectTemplateOpt {
            template: Some(TemplateId("test-template".to_string())),
            mcp: false,
        };
        let cloned = original.clone();

        assert_eq!(original.template, cloned.template);
    }
}
