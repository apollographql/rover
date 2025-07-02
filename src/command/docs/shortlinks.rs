pub const URL_BASE: &str = "https://go.apollo.dev/r";

use std::collections::BTreeMap;

pub fn get_shortlinks_with_description() -> BTreeMap<&'static str, &'static str> {
    let mut links = BTreeMap::new();
    links.insert("docs", "Rover's Documentation Homepage");
    links.insert("api-keys", "Understanding Apollo's API Keys");
    links.insert("contributing", "Contributing to Rover");
    links.insert("migration", "Migrate from the Apollo CLI to Rover");
    links.insert("start", "Getting Started with Rover");
    links.insert("configuring", "Configuring Rover");
    links.insert(
        "template",
        "Learn how to add a template to an existing graph",
    );
    links
}

pub fn possible_shortlinks() -> clap::builder::PossibleValuesParser {
    let mut res = Vec::new();
    for (slug, _) in get_shortlinks_with_description() {
        res.push(slug);
    }
    clap::builder::PossibleValuesParser::new(res)
}

pub fn get_url_from_slug(slug: &str) -> String {
    format!("{URL_BASE}/{slug}")
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
        let expected_link = "https://go.apollo.dev/r/start";
        let actual_link = super::get_url_from_slug("start");
        assert_eq!(expected_link, actual_link);
    }

    #[test]
    fn each_url_is_valid() {
        for link in super::possible_shortlinks().possible_values().unwrap() {
            let url = super::get_url_from_slug(link.get_name());
            assert!(reqwest::blocking::get(&url)
                .unwrap()
                .error_for_status()
                .is_ok());
        }
    }
}
