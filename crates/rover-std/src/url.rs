use url::Url;
use crate::style::Style;

pub fn sanitize_url(url: &str) -> Option<String> {
    Url::parse(url).ok().and_then(|mut parsed_url| {
        if (parsed_url.username() != "" && parsed_url.set_username("").is_err())
            || parsed_url.set_password(None).is_err()
        {
            None
        } else {
            Some(parsed_url.to_string())
        }
    })
}

pub fn hyperlink(url: &str) -> String {
    let sanitized_url = sanitize_url(url).unwrap_or_else(|| url.to_string());
    
    Style::Link.paint(sanitized_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNAUTHENTICATED_URL: &str = "https://rover.apollo.dev/nix/latest";
    const AUTHENTICATED_URL: &str = "https://username:password@customer.proxy/nix/latest";
    const SANITIZED_AUTHENTICATED_URL: &str = "https://customer.proxy/nix/latest";

    const INVALID_URL: &str = "not-a-url";

    #[test]
    fn it_leaves_unauthenticated_url_alone() {
        let sanitized_url = sanitize_url(UNAUTHENTICATED_URL);
        assert_eq!(sanitized_url, Some(UNAUTHENTICATED_URL.to_string()));
    }

    #[test]
    fn it_sanitizes_authenticated_url() {
        let sanitized_url = sanitize_url(AUTHENTICATED_URL);
        assert_eq!(sanitized_url, Some(SANITIZED_AUTHENTICATED_URL.to_string()));
    }

    #[test]
    fn it_returns_none_for_invalid_url() {
        let sanitized_url = sanitize_url(INVALID_URL);
        assert_eq!(sanitized_url, None);
    }
}
