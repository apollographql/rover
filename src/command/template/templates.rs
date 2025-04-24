use std::env;

use anyhow::anyhow;
use console::Term;
use dialoguer::Select;
use reqwest::Client;
use rover_std::Style;
use serde::de::DeserializeOwned;
use serde::Serialize;

use graphql_client::{GraphQLQuery, Response};

use crate::options::ProjectLanguage;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

use super::queries::{
    get_template_by_id::GetTemplateByIdTemplate,
    get_templates_for_language::GetTemplatesForLanguageTemplates,
    list_templates_for_language::ListTemplatesForLanguageTemplates, *,
};

async fn request<Body: Serialize, Data: DeserializeOwned>(body: &Body) -> RoverResult<Data> {
    let uri = env::var("APOLLO_TEMPLATES_API")
        .unwrap_or_else(|_| "https://rover.apollo.dev/templates".to_string());
    let resp = Client::new()
        .post(uri)
        .json(body)
        .send()
        .await
        .map_err(|e| anyhow!("Could not reach templates server: {}", e))?;
    let response: Response<Data> = resp
        .json()
        .await
        .map_err(|e| anyhow!("Could not parse response from templates server: {}", e))?;
    response
        .data
        .ok_or_else(|| anyhow!("No data in response from templates server").into())
}

/// Get a template by ID
pub async fn get_template(template_id: &str) -> RoverResult<Option<GetTemplateByIdTemplate>> {
    use super::queries::get_template_by_id::*;
    let query = GetTemplateById::build_query(Variables {
        id: template_id.to_string(),
    });
    let resp: ResponseData = request(&query).await?;
    Ok(resp.template)
}

pub async fn get_templates_for_language(
    language: ProjectLanguage,
) -> RoverResult<Vec<GetTemplatesForLanguageTemplates>> {
    use super::queries::get_templates_for_language::*;
    let query = GetTemplatesForLanguage::build_query(Variables {
        language: Some(language.into()),
    });
    let resp: ResponseData = request(&query).await?;
    error_if_empty(resp.templates)
}

pub async fn list_templates(
    language: Option<ProjectLanguage>,
) -> RoverResult<Vec<ListTemplatesForLanguageTemplates>> {
    use super::queries::list_templates_for_language::*;
    let query = ListTemplatesForLanguage::build_query(Variables {
        language: language.map(Into::into),
    });
    let resp: ResponseData = request(&query).await?;
    error_if_empty(resp.templates)
}

pub fn error_if_empty<T>(values: Vec<T>) -> RoverResult<Vec<T>> {
    if values.is_empty() {
        let mut err = RoverError::new(anyhow!("No matching template found"));
        err.set_suggestion(RoverErrorSuggestion::Adhoc(format!(
            "Run `{}` to see all available templates.",
            Style::Command.paint("rover template list")
        )));
        Err(err)
    } else {
        Ok(values)
    }
}

/// Prompt to select a template
pub fn selection_prompt(
    mut templates: Vec<GetTemplatesForLanguageTemplates>,
) -> RoverResult<GetTemplatesForLanguageTemplates> {
    let names = templates
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>();
    let selection = Select::new()
        .with_prompt("Which template would you like to use?")
        .items(&names)
        .default(0)
        .interact_on_opt(&Term::stderr())?;

    match selection {
        Some(index) => Ok(templates.remove(index)),
        None => Err(RoverError::new(anyhow!("No template selected"))),
    }
}
