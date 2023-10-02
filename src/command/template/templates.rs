use std::env;
use std::thread::sleep;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use console::Term;
use dialoguer::Select;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;

use graphql_client::{GraphQLQuery, Response};
use rover_std::Fs;

use crate::options::ProjectLanguage;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

use super::queries::{
    get_template_by_id::GetTemplateByIdTemplate,
    get_templates_for_language::GetTemplatesForLanguageTemplates,
    list_templates_for_language::ListTemplatesForLanguageTemplates, *,
};

fn request<Body: Serialize, Data: DeserializeOwned>(body: &Body) -> RoverResult<Data> {
    let uri = env::var("APOLLO_TEMPLATES_API")
        .unwrap_or_else(|_| "https://rover.apollo.dev/templates".to_string());
    let resp = Client::new()
        .post(uri)
        .json(body)
        .send()
        .map_err(|e| anyhow!("Could not reach templates server: {}", e))?;
    let response: Response<Data> = resp
        .json()
        .map_err(|e| anyhow!("Could not parse response from templates server: {}", e))?;
    response.data.ok_or_else(|| {
        anyhow!(
            "No data in response from templates server. Errors: {:?}",
            response.errors
        )
        .into()
    })
}

/// Get a template by ID
pub fn get_template(template_id: &str) -> RoverResult<Option<GetTemplateByIdTemplate>> {
    use super::queries::get_template_by_id::*;
    let query = GetTemplateById::build_query(Variables {
        id: template_id.to_string(),
    });
    let resp: ResponseData = request(&query)?;
    Ok(resp.template)
}

pub fn get_templates_for_language(
    language: ProjectLanguage,
) -> RoverResult<Vec<GetTemplatesForLanguageTemplates>> {
    use super::queries::get_templates_for_language::*;
    let query = GetTemplatesForLanguage::build_query(Variables {
        language: Some(language.into()),
    });
    let resp: ResponseData = request(&query)?;
    error_if_empty(resp.templates)
}

pub fn list_templates(
    language: Option<ProjectLanguage>,
) -> RoverResult<Vec<ListTemplatesForLanguageTemplates>> {
    use super::queries::list_templates_for_language::*;
    let query = ListTemplatesForLanguage::build_query(Variables {
        language: language.map(Into::into),
    });
    let resp: ResponseData = request(&query)?;
    error_if_empty(resp.templates)
}

pub fn run_codegen(
    url: String,
    operations: Vec<String>,
    graphos_token: String,
) -> RoverResult<String> {
    let query = StartCodegen::build_query(start_codegen::Variables {
        input: start_codegen::StartCodegenInput {
            recaptcha_token: None,
            graphos_token: Some(graphos_token),
            url,
            email: None,
            operations,
        },
    });
    let start_codegen::ResponseData {
        start_codegen: start_codegen::StartCodegenStartCodegen { id },
    } = request(&query)?;
    eprintln!("Codegen ID: {id}");
    let wait_query = CheckOnCodegen::build_query(check_on_codegen::Variables { id });
    let mut schema_done = false;
    let mut resolvers_done = false;
    loop {
        sleep(std::time::Duration::from_secs(5));
        let check_on_codegen::ResponseData {
            codegen_status: Some(status),
        } = request(&wait_query)?
        else {
            continue;
        };
        if let Some(sandbox_url) = status.sandbox_url {
            return Ok(sandbox_url);
        }
        if !schema_done {
            if let Some(schema) = status.schema {
                let mut path = Utf8PathBuf::from("schema.graphql");
                let mut i = 0;
                while path.exists() {
                    i += 1;
                    path = Utf8PathBuf::from(&format!("schema-{i}.graphql"));
                }
                Fs::write_file(&path, schema)?;
                eprintln!("Schema generation complete, you can view it at {path}");
                eprintln!("Generating resolvers...");
                schema_done = true;
            }
        }
        if !resolvers_done && status.resolvers.is_some() {
            eprintln!("Resolver generation complete, creating codesandbox...");
            resolvers_done = true;
        }
    }
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
