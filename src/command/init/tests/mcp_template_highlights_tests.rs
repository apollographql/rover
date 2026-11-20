use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::{
    helpers::mcp_template_highlights,
    template_fetcher::{Template, TemplateId},
};

/// An MCP template as `fetch_mcp_template` builds it from a manifest entry.
fn mcp_template(base_id: &str, base_display_name: &str) -> Template {
    Template {
        id: TemplateId(format!("mcp-{base_id}")),
        display_name: format!("{base_display_name} + AI tools"),
        path: String::new(),
        language: String::new(),
        federation_version: "=2.11.0".to_string(),
        max_schema_depth: 5,
        routing_url: String::new(),
        commands: None,
        start_point_file: "GETTING_STARTED.md".to_string(),
        print_depth: None,
    }
}

#[test]
fn connectors_template_is_described_as_rest_connectors() {
    let highlights = mcp_template_highlights(&mcp_template("connectors", "Start with REST"));

    assert_that!(highlights).is_equal_to(vec![
        "Example GraphQL schema and REST connectors".to_string(),
        "Pre-configured MCP server with Docker setup".to_string(),
        "Sample tools showing how to make APIs AI-callable".to_string(),
    ]);
}

#[test]
fn typescript_template_is_described_by_its_manifest_name() {
    let highlights = mcp_template_highlights(&mcp_template(
        "typescript",
        "Build an API with TypeScript and Apollo Server",
    ));

    assert_that!(highlights).is_equal_to(vec![
        "Example GraphQL schema and starter subgraph: Build an API with TypeScript and Apollo Server"
            .to_string(),
        "Pre-configured MCP server with Docker setup".to_string(),
        "Sample tools showing how to make GraphQL APIs AI-callable".to_string(),
    ]);
}

#[rstest]
#[case("typescript", "Build an API with TypeScript and Apollo Server")]
#[case("python", "Build an API with Python")]
fn non_connectors_templates_never_mention_connectors(
    #[case] base_id: &str,
    #[case] base_display_name: &str,
) {
    let highlights = mcp_template_highlights(&mcp_template(base_id, base_display_name));

    for line in highlights {
        assert_that!(line.to_lowercase()).does_not_contain("connectors");
    }
}

/// The deprecated new-project path passes the manifest entry without the
/// MCP prefix and suffix; it gets the same description.
#[test]
fn unprefixed_template_is_described_the_same_way() {
    let template = Template {
        id: TemplateId("typescript".to_string()),
        display_name: "Build an API with TypeScript and Apollo Server".to_string(),
        ..mcp_template("typescript", "")
    };

    assert_that!(mcp_template_highlights(&template)).is_equal_to(mcp_template_highlights(
        &mcp_template(
            "typescript",
            "Build an API with TypeScript and Apollo Server",
        ),
    ));
}
