use anyhow::anyhow;
use dialoguer::{Select, console::Term};
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};

use super::queries::{
    get_template_by_id::GetTemplateByIdTemplate,
    get_templates_for_language::GetTemplatesForLanguageTemplates,
    list_templates_for_language::ListTemplatesForLanguageTemplates, *,
};
use crate::{RoverError, RoverErrorSuggestion, RoverResult, options::ProjectLanguage};

const DEFAULT_TEMPLATES_API: &str = "https://rover.apollo.dev/templates";

async fn request<Body: Serialize, Data: DeserializeOwned>(
    body: &Body,
    templates_api: Option<&str>,
) -> RoverResult<Data> {
    let uri = templates_api.unwrap_or(DEFAULT_TEMPLATES_API);
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
pub async fn get_template(
    template_id: &str,
    templates_api: Option<&str>,
) -> RoverResult<Option<GetTemplateByIdTemplate>> {
    use super::queries::get_template_by_id::*;
    let query = GetTemplateById::build_query(Variables {
        id: template_id.to_string(),
    });
    let resp: ResponseData = request(&query, templates_api).await?;
    Ok(resp.template)
}

pub async fn get_templates_for_language(
    language: ProjectLanguage,
    templates_api: Option<&str>,
) -> RoverResult<Vec<GetTemplatesForLanguageTemplates>> {
    use super::queries::get_templates_for_language::*;
    let query = GetTemplatesForLanguage::build_query(Variables {
        language: Some(language.into()),
    });
    let resp: ResponseData = request(&query, templates_api).await?;
    error_if_empty(resp.templates)
}

pub async fn list_templates(
    language: Option<ProjectLanguage>,
    templates_api: Option<&str>,
) -> RoverResult<Vec<ListTemplatesForLanguageTemplates>> {
    use super::queries::list_templates_for_language::*;
    let query = ListTemplatesForLanguage::build_query(Variables {
        language: language.map(Into::into),
    });
    let resp: ResponseData = request(&query, templates_api).await?;
    error_if_empty(resp.templates)
}

pub fn error_if_empty<T>(values: Vec<T>) -> RoverResult<Vec<T>> {
    if values.is_empty() {
        let mut err = RoverError::new(anyhow!("No matching template found"));
        err.set_suggestion(RoverErrorSuggestion::Adhoc(
            "Run `rover template list` to see all available templates.".to_string(),
        ));
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
