use crate::style::Style;
use url::Url;

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

/// Creates a clickable link with custom display text
pub fn hyperlink_with_text(url: &str, display_text: &str) -> String {
    let sanitized_url = sanitize_url(url).unwrap_or_else(|| url.to_string());
    format!(
        "\x1B]8;;{}\x1B\\{}\x1B]8;;\x1B\\",
        sanitized_url,
        Style::Link.paint(display_text)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNAUTHENTICATED_URL: &str = "https://rover.apollo.dev/nix/latest";
    const AUTHENTICATED_URL: &str = "https://username:password@customer.proxy/nix/latest";
    const SANITIZED_AUTHENTICATED_URL: &str = "https://customer.proxy/nix/latest";
    const INVALID_URL: &str = "not-a-url";
    const DISPLAY_TEXT: &str = "Click here";

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

    #[test]
    fn it_creates_hyperlink_with_custom_text() {
        let result = hyperlink_with_text(UNAUTHENTICATED_URL, DISPLAY_TEXT);
        let expected = format!(
            "\x1B]8;;{}\x1B\\{}\x1B]8;;\x1B\\",
            UNAUTHENTICATED_URL,
            Style::Link.paint(DISPLAY_TEXT)
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn it_creates_hyperlink_with_custom_text_for_authenticated_url() {
        let result = hyperlink_with_text(AUTHENTICATED_URL, DISPLAY_TEXT);
        let expected = format!(
            "\x1B]8;;{}\x1B\\{}\x1B]8;;\x1B\\",
            SANITIZED_AUTHENTICATED_URL,
            Style::Link.paint(DISPLAY_TEXT)
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn it_creates_hyperlink_with_custom_text_for_invalid_url() {
        let result = hyperlink_with_text(INVALID_URL, DISPLAY_TEXT);
        let expected = format!(
            "\x1B]8;;{}\x1B\\{}\x1B]8;;\x1B\\",
            INVALID_URL,
            Style::Link.paint(DISPLAY_TEXT)
        );
        assert_eq!(result, expected);
    }
}
