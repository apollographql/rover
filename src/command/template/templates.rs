use crate::command::template::GithubTemplate;
use crate::options::ProjectLanguage;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

use std::iter::IntoIterator;

use anyhow::anyhow;
use console::Term;
use dialoguer::Select;

/// TODO: Fetch templates from an API instead of embedding them
const TEMPLATES: [GithubTemplate; 8] = [
    GithubTemplate {
        id: "subgraph-go-gqlgen",
        git_url: "https://github.com/apollographql/subgraph-template-go-gqlgen-boilerplate",
        display: "Go (gqlgen)",
        language: ProjectLanguage::Go,
    },
    GithubTemplate {
        id: "subgraph-java-spring-graphql",
        git_url: "https://github.com/apollographql/subgraph-template-java-spring-graphql-boilerplate",
        display: "Spring GraphQL",
        language: ProjectLanguage::Java,
    },
    GithubTemplate {
        id: "subgraph-javascript-apollo-server",
        git_url: "https://github.com/apollographql/subgraph-template-javascript-apollo-server-boilerplate",
        display: "Apollo Server (JS)",
        language: ProjectLanguage::Javascript,
    },
    GithubTemplate {
        id: "subgraph-graphql-kotlin",
        git_url: "https://github.com/apollographql/subgraph-template-graphql-kotlin-boilerplate",
        display: "GraphQL Kotlin",
        language: ProjectLanguage::Kotlin,
    },
    GithubTemplate {
        id: "subgraph-python-strawberry-fastapi",
        git_url: "https://github.com/strawberry-graphql/subgraph-template-strawberry-fastapi",
        display: "Strawberry with FastAPI",
        language: ProjectLanguage::Python,
    },
    GithubTemplate {
        id: "subgraph-python-ariadne-fastapi",
        git_url: "https://github.com/mirumee/subgraph-template-ariadne-fastapi",
        display: "Ariadne with FastAPI",
        language: ProjectLanguage::Python,
    },
    GithubTemplate {
        id: "subgraph-rust-async-graphql",
        git_url: "https://github.com/apollographql/subgraph-template-rust-async-graphql",
        display: "async-graphql with Axum",
        language: ProjectLanguage::Rust,
    },
    GithubTemplate {
        id: "subgraph-typescript-apollo-server",
        git_url: "https://github.com/apollographql/subgraph-template-typescript-apollo-server-boilerplate",
        display: "Apollo Server (TS)",
        language: ProjectLanguage::Typescript,
    },
];

pub struct GithubTemplates {
    templates: Vec<GithubTemplate>,
}

impl GithubTemplates {
    /// Instantiate all available templates
    pub fn new() -> Self {
        Self {
            templates: Vec::from(TEMPLATES),
        }
    }

    /// Get a template by ID
    pub fn get(self, template_id: &str) -> RoverResult<GithubTemplate> {
        self.templates
            .into_iter()
            .find(|template| template.id == template_id)
            .ok_or_else(|| {
                let mut err = RoverError::new(anyhow!("No template found with id {}", template_id));
                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                    "Run `rover template list` to see all available templates.".to_string(),
                ));
                err
            })
    }

    /// Filter templates by language
    #[must_use]
    pub fn filter_language(mut self, language: ProjectLanguage) -> Self {
        self.templates = language.filter(self.templates);
        self
    }

    /// Consume self and return the list of templates that were selected.
    ///
    /// # Errors
    ///
    /// Returns an error if there were no matching templates.
    pub fn values(self) -> RoverResult<Vec<GithubTemplate>> {
        if self.templates.is_empty() {
            Err(RoverError::new(anyhow!(
                "No templates matched the provided filters"
            )))
        } else {
            Ok(self.templates)
        }
    }

    /// Prompt to select a template
    pub fn selection_prompt(self) -> RoverResult<GithubTemplate> {
        let mut templates = self.values()?;
        let selection = Select::new()
            .with_prompt("Which template would you like to use?")
            .items(&templates)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(templates.remove(index)),
            None => Err(RoverError::new(anyhow!("No template selected"))),
        }
    }
}
