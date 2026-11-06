use std::collections::HashMap;

use camino::Utf8PathBuf;
use rover_print::print::testing::TerminalCapture;
use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::Init;

const PREVIEW_HEADER: &str = "=> You're about to add the following files to your local directory:";

/// Template files keyed by path. Contents are not shown in the preview.
fn files(paths: &[&str]) -> HashMap<Utf8PathBuf, String> {
    paths
        .iter()
        .map(|path| (Utf8PathBuf::from(*path), String::new()))
        .collect()
}

fn lines(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| line.to_string()).collect()
}

// One file per category, so the order within a category does not depend on
// HashMap iteration.
#[rstest]
#[case::every_category(
    &[
        "mcp.Dockerfile",
        ".env.template",
        ".gitignore",
        "AGENTS.md",
        "README.md",
        "tools/GetUser.graphql",
    ],
    &[
        "",
        PREVIEW_HEADER,
        "",
        "MCP Server: Config, sample tools and tests",
        "- mcp.Dockerfile (optional Docker container)",
        "",
        "Apollo GraphOS: Apollo graph credentials and project files",
        "- .env",
        "",
        "IDE: Project settings",
        "- .gitignore",
        "",
        "Guides and references",
        "- AGENTS.md → Agent rules for the robots",
        "",
    ],
)]
#[case::only_uncategorized_files(
    &["README.md", "tools/GetUser.graphql", "supergraph.graphql"],
    &["", PREVIEW_HEADER, ""],
)]
#[case::no_files(&[], &["", PREVIEW_HEADER, ""])]
fn print_mcp_preview_lists_files_by_category(#[case] paths: &[&str], #[case] expected: &[&str]) {
    let stderr = TerminalCapture::new(false);

    Init::print_mcp_preview(&stderr, &files(paths));

    assert_that!(stderr.lines()).is_equal_to(lines(expected));
}

#[rstest]
fn print_mcp_preview_keeps_heading_style() {
    let stderr = TerminalCapture::new(true);

    Init::print_mcp_preview(&stderr, &files(&[".gitignore"]));

    assert_that!(stderr.lines()).is_equal_to(lines(&[
        "",
        PREVIEW_HEADER,
        "",
        "\u{1b}[1mIDE: Project settings\u{1b}[0m",
        "- .gitignore",
        "",
    ]));
}
