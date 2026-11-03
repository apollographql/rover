use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::{
    GraphVariantOption,
    helpers::{
        MCP_DEFAULT_PORT, existing_graph_mcp_readme, mcp_docker_run_hint, mcp_local_endpoint,
    },
};

fn selected_graph() -> GraphVariantOption {
    GraphVariantOption {
        organization_name: "Acme".to_string(),
        graph_id: "my-graph".to_string(),
        graph_name: "My Graph".to_string(),
        variant_name: "current".to_string(),
        display_name: "My Graph (current)".to_string(),
    }
}

#[test]
fn mcp_default_port_matches_generated_config() {
    assert_that!(MCP_DEFAULT_PORT).is_equal_to(8000);
}

#[test]
fn mcp_local_endpoint_uses_default_port() {
    assert_that!(mcp_local_endpoint()).is_equal_to("http://127.0.0.1:8000/mcp".to_string());
}

#[rstest]
#[case(
    "my-graph",
    "docker run -p 127.0.0.1:8000:8000 --env-file .env my-graph-mcp"
)]
#[case(
    "other_graph.v2",
    "docker run -p 127.0.0.1:8000:8000 --env-file .env other_graph.v2-mcp"
)]
fn mcp_docker_run_hint_uses_default_port_and_mcp_image(
    #[case] docker_tag: &str,
    #[case] expected: &str,
) {
    assert_that!(mcp_docker_run_hint(docker_tag)).is_equal_to(expected.to_string());
}

#[test]
fn existing_graph_mcp_readme_renders_full_content() {
    let readme = existing_graph_mcp_readme(
        &selected_graph(),
        "my-graph@current",
        "Subgraphs: products, reviews",
        "my-graph",
        "https://studio.apollographql.com/graph/my-graph/explorer",
    );

    let expected = r#"# My Graph MCP Server

This MCP server provides AI-accessible tools for your Apollo graph.

## Graph Information
- **Graph ID**: my-graph
- **Variant**: current
- **Organization**: Acme
- **Graph Reference**: my-graph@current

- **Subgraphs: products, reviews**

## Quick Start

1. Build the MCP server:
   ```bash
   docker build -f mcp.Dockerfile -t my-graph-mcp .
   ```

2. Run the MCP server:
   ```bash
   docker run -p 127.0.0.1:8000:8000 --env-file .env my-graph-mcp
   # Linux users may need: docker run --network=host --env-file .env my-graph-mcp
   ```

3. Test with MCP Inspector:
   ```bash
   npx @mcp/inspector
   ```

4. Configure Claude Desktop to use this server.

## View in Apollo Studio
https://studio.apollographql.com/graph/my-graph/explorer"#;

    assert_that!(readme).is_equal_to(expected.to_string());
}

#[test]
fn existing_graph_mcp_readme_omits_empty_subgraph_line_and_old_port() {
    let readme = existing_graph_mcp_readme(
        &selected_graph(),
        "my-graph@current",
        "",
        "my-graph",
        "https://studio.apollographql.com/graph/my-graph/explorer",
    );

    assert_that!(readme).contains("- **Graph Reference**: my-graph@current\n\n\n## Quick Start");
    assert_that!(readme)
        .contains("   docker run -p 127.0.0.1:8000:8000 --env-file .env my-graph-mcp\n");
    assert_that!(readme.contains("5050")).is_false();
}
