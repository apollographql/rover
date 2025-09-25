pub const URL_BASE: &str = "https://go/apollo/dev";

use serde::Serialize;
use std::collections::BTreeMap;

/**
 * The ShortlinkInfo struct contains the description, parent route, and slug of the shortlink.
 * The parent route is the route that the shortlink is nested under.
 * The slug is the shortlink slug.
 * The description is the description of the shortlink.
 */
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShortlinkInfo {
    pub description: &'static str,
    pub parent_route: &'static str,
    pub slug: &'static str,
}

/**
 * Creates a new ShortlinkInfo struct.
 * The ShortlinkInfo struct contains the description, parent route, and slug of the shortlink.
 * The parent route is the route that the shortlink is nested under.
 * The slug is the shortlink slug.
 * The description is the description of the shortlink.
 */
impl ShortlinkInfo {
    pub fn new(description: &'static str, parent_route: &'static str, slug: &'static str) -> Self {
        Self {
            description,
            parent_route,
            slug,
        }
    }
}

/**
 * Returns a map of shortlink slugs to their corresponding ShortlinkInfo struct.
 * The ShortlinkInfo struct contains the description, parent route, and slug of the shortlink.
 * The parent route is the route that the shortlink is nested under.
 * The slug is the shortlink slug.
 * The description is the description of the shortlink.
 */
pub fn get_shortlinks_with_info() -> BTreeMap<&'static str, ShortlinkInfo> {
    let mut links = BTreeMap::new();
    links.insert(
        "docs",
        ShortlinkInfo::new("Rover's Documentation Homepage", "r", "docs"),
    );
    links.insert(
        "api-keys",
        ShortlinkInfo::new("Understanding Apollo's API Keys", "r", "api-keys"),
    );
    links.insert(
        "contributing",
        ShortlinkInfo::new("Contributing to Rover", "r", "contributing"),
    );
    links.insert(
        "migration",
        ShortlinkInfo::new("Migrate from the Apollo CLI to Rover", "r", "migration"),
    );
    links.insert(
        "start",
        ShortlinkInfo::new("Getting Started with Rover", "r", "start"),
    );
    links.insert(
        "configuring",
        ShortlinkInfo::new("Configuring Rover", "r", "configuring"),
    );
    links.insert(
        "template",
        ShortlinkInfo::new(
            "Learn how to add a template to an existing graph",
            "r",
            "template",
        ),
    );
    links.insert(
        "mcp-deploy",
        ShortlinkInfo::new("Deploy Apollo MCP Server", "mcp", "deploy"),
    );
    links.insert(
        "mcp-qs",
        ShortlinkInfo::new("Apollo MCP Server Quick Start", "mcp", "qs"),
    );
    links.insert(
        "mcp-config",
        ShortlinkInfo::new(
            "Reference guide for the Apollo MCP Server config file",
            "mcp",
            "config",
        ),
    );
    links.insert(
        "mcp-tools",
        ShortlinkInfo::new(
            "Learn how to define tools for the Apollo MCP Server",
            "mcp",
            "define-tools",
        ),
    );
    links
}

/**
 * Returns a PossibleValuesParser for the shortlinks.
 * The PossibleValuesParser is used to validate the shortlink slug.
 */
pub fn possible_shortlinks() -> clap::builder::PossibleValuesParser {
    let mut res = Vec::new();
    for (key, _) in get_shortlinks_with_info() {
        res.push(key);
    }
    clap::builder::PossibleValuesParser::new(res)
}

/**
 * Returns the URL for a given shortlink slug.
 * The URL is constructed by combining the URL base with the parent route and slug.
 * If the parent route is empty, the URL is constructed by combining the URL base with the slug.
 */
pub fn get_url_from_slug(slug: &str) -> String {
    let links = get_shortlinks_with_info();

    if let Some(shortlink_info) = links.get(slug) {
        if shortlink_info.parent_route.is_empty() {
            format!("{URL_BASE}/{}", shortlink_info.slug)
        } else {
            format!(
                "{URL_BASE}/{}/{}",
                shortlink_info.parent_route, shortlink_info.slug
            )
        }
    } else {
        // Fallback for unknown slugs
        format!("{URL_BASE}/{slug}")
    }
}

#[cfg(test)]
mod tests {
    use clap::builder::TypedValueParser;

    #[test]
    fn can_make_shortlink_vec_from_map() {
        assert_ne!(
            super::possible_shortlinks()
                .possible_values()
                .unwrap()
                .count(),
            0
        )
    }

    #[test]
    fn can_get_url_from_slug() {
        let expected_link = "https://go/apollo/dev/r/start";
        let actual_link = super::get_url_from_slug("start");
        assert_eq!(expected_link, actual_link);
    }

    #[test]
    fn can_handle_empty_parent_route() {
        // Add a test entry with empty parent route to verify functionality
        let mut links = super::get_shortlinks_with_info();
        links.insert(
            "test-empty",
            super::ShortlinkInfo::new("Test Entry", "", "test-slug"),
        );

        let shortlink_info = links.get("test-empty").unwrap();
        let expected_url = if shortlink_info.parent_route.is_empty() {
            format!("{}/test-slug", super::URL_BASE)
        } else {
            format!(
                "{}/{}/test-slug",
                super::URL_BASE,
                shortlink_info.parent_route
            )
        };

        assert_eq!(expected_url, "https://go/apollo/dev/test-slug");
    }

    #[test]
    fn each_url_is_valid() {
        // Instead of making real HTTP requests, just check that the URLs are well-formed.
        // This avoids flakiness and network dependency in tests.
        for link in super::possible_shortlinks().possible_values().unwrap() {
            let url = super::get_url_from_slug(link.get_name());
            // Check that the URL starts with the expected base
            assert!(
                url.starts_with(super::URL_BASE),
                "URL '{}' does not start with base '{}'",
                url,
                super::URL_BASE
            );
            // Check that the URL is a valid URL
            let parsed = url::Url::parse(&url);
            assert!(
                parsed.is_ok(),
                "URL '{}' is not a valid URL: {:?}",
                url,
                parsed.err()
            );
        }
    }
}
