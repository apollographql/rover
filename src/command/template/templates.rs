use crate::command::template::GithubTemplate;
use crate::options::ProjectLanguage;
use crate::{anyhow, error::RoverError, Result, Suggestion};

use std::collections::HashMap;
use std::iter::IntoIterator;

use console::Term;
use dialoguer::Select;

/// TODO: Fetch templates from an API instead of embedding them
const TEMPLATES: [GithubTemplate; 4] = [
    GithubTemplate {
        id: "subgraph-javascript-apollo-server",
        git_url: "https://github.com/apollographql/subgraph-template-javascript-apollo-server-boilerplate",
        display: "Apollo Server",
        language: ProjectLanguage::Javascript,
    },
    GithubTemplate {
        id: "subgraph-python-strawberry-fastapi",
        git_url: "https://github.com/strawberry-graphql/subgraph-template-strawberry-fastapi",
        display: "Strawberry with FastAPI",
        language: ProjectLanguage::Python,
    },
    GithubTemplate {
        id: "subgraph-python-ariadne-fastapi",
        git_url: "https://github.com/patrick91/subgraph-template-ariadne-fastapi",
        display: "Ariadne with FastAPI",
        language: ProjectLanguage::Python,
    },
    GithubTemplate {
        id: "subgraph-rust-async-graphql",
        git_url: "https://github.com/apollographql/subgraph-template-rust-async-graphql",
        display: "async-graphql with Axum",
        language: ProjectLanguage::Rust,
    }
];

pub struct GithubTemplates {
    templates: HashMap<&'static str, GithubTemplate>,
}

impl GithubTemplates {
    /// Instantiate all available templates
    pub fn new() -> Self {
        Self::new_from_templates(TEMPLATES)
    }

    /// Instantiate available templates from an iterator
    fn new_from_templates<T>(t: T) -> Self
    where
        T: IntoIterator<Item = GithubTemplate>,
    {
        let mut templates = HashMap::new();
        t.into_iter().for_each(|template| {
            templates.insert(template.id, template);
        });

        Self { templates }
    }

    /// Get a template by ID
    pub fn get(&self, template_id: &str) -> Result<GithubTemplate> {
        self.templates.get(template_id).cloned().ok_or_else(|| {
            let mut err = RoverError::new(anyhow!("No template found with id {}", template_id));
            err.set_suggestion(Suggestion::Adhoc(
                "Run `rover template list` to see all available templates.".to_string(),
            ));
            err
        })
    }

    /// Filter templates by language
    pub fn filter_language(&mut self, language: ProjectLanguage) {
        *self = Self::new_from_templates(language.filter(self.values()));
    }

    /// Get all templates
    pub fn values(&self) -> Vec<GithubTemplate> {
        self.templates
            .iter()
            .map(|(_, template)| template.clone())
            .collect()
    }

    /// Return an error if there are no templates left
    pub fn error_on_empty(&self) -> Result<()> {
        if self.templates.is_empty() {
            Err(RoverError::new(anyhow!(
                "No templates matched the provided filters"
            )))
        } else {
            Ok(())
        }
    }

    /// Prompt to select a template
    pub fn selection_prompt(&self) -> Result<GithubTemplate> {
        let templates = self.values();
        let selection = Select::new()
            .with_prompt("Which template would you like to use?")
            .items(&templates)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(templates[index].clone()),
            None => Err(RoverError::new(anyhow!("No template selected"))),
        }
    }
}
