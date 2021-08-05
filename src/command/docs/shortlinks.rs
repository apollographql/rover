pub const URL_BASE: &str = "https://go.apollo.dev/r";

use std::collections::HashMap;

pub fn get_shortlinks_with_description() -> HashMap<&'static str, &'static str> {
    let mut links = HashMap::new();
    links.insert("docs", "Rover's Documentation Homepage");
    links.insert("api-keys", "Understanding Apollo's API Keys");
    links.insert("contributing", "Contributing to Rover");
    links.insert("migration", "Migrate from the Apollo CLI to Rover");
    links.insert("start", "Getting Started with Rover");
    links
}

pub fn possible_shortlinks() -> Vec<&'static str> {
    let mut res = Vec::new();
    for (slug, _) in get_shortlinks_with_description() {
        res.push(slug);
    }
    res
}

pub fn get_url_from_slug(slug: &str) -> String {
    format!("{}/{}", URL_BASE, slug)
}

mod tests {
    #[test]
    fn can_make_shortlink_vec_from_map() {
        let shortlinks = super::possible_shortlinks();
        assert!(!shortlinks.is_empty())
    }

    #[test]
    fn can_get_url_from_slug() {
        let expected_link = "https://go.apollo.dev/r/start";
        let actual_link = super::get_url_from_slug("start");
        assert_eq!(expected_link, actual_link);
    }

    #[test]
    fn each_url_is_valid() {
        for link in super::possible_shortlinks() {
            let url = super::get_url_from_slug(link);
            assert!(reqwest::blocking::get(&url)
                .unwrap()
                .error_for_status()
                .is_ok());
        }
    }
}
