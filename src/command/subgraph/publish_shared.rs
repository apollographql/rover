use crate::{RoverError, RoverErrorSuggestion, RoverResult};
use anyhow::anyhow;
use rover_std::Style;
use std::future::Future;
use std::io;
use std::io::{IsTerminal, Read, Write};
use url::Url;

pub async fn determine_routing_url<F, G>(
    no_url: bool,
    routing_url: &Option<String>,
    allow_invalid_routing_url: bool,
    fetch: F,
) -> RoverResult<Option<String>>
where
    F: Fn() -> G,
    G: Future<Output = RoverResult<String>>,
{
    determine_routing_url_with_test_params(
        no_url,
        routing_url,
        allow_invalid_routing_url,
        fetch,
        &mut io::stderr(),
        &mut io::stdin(),
        io::stderr().is_terminal() && io::stdin().is_terminal(),
    )
    .await
}

pub async fn determine_routing_url_with_test_params<F, G>(
    no_url: bool,
    routing_url: &Option<String>,
    allow_invalid_routing_url: bool,

    // For testing purposes, we pass in a closure for fetching the
    // routing url from GraphOS
    fetch: F,
    // For testing purposes, we pass in stub `Write`er and `Read`ers to
    // simulate input and verify output.
    writer: &mut impl Write,
    reader: &mut impl Read,
    // Simulate a CI environment (non-TTY) for testing
    is_atty: bool,
) -> RoverResult<Option<String>>
where
    F: Fn() -> G,
    G: Future<Output = RoverResult<String>>,
{
    if no_url && routing_url.is_some() {
        return Err(RoverError::new(anyhow!(
            "You cannot use --no-url and --routing-url at the same time."
        )));
    }

    // if --allow-invalid-routing-url is not provided, we need to inspect
    // the URL and possibly prompt the user to publish. this does nothing
    // if the routing url is not provided.
    if !no_url && !allow_invalid_routing_url {
        handle_maybe_invalid_routing_url(routing_url, writer, reader, is_atty)?;
    }

    // don't bother fetching and validating an existing routing url if
    // --no-url is set
    let mut routing_url = routing_url.clone();
    if !no_url && routing_url.is_none() {
        let fetch_response = fetch().await?;
        handle_maybe_invalid_routing_url(&Some(fetch_response.clone()), writer, reader, is_atty)?;
        routing_url = Some(fetch_response)
    }

    if let Some(routing_url) = routing_url {
        Ok(Some(routing_url))
    } else if no_url {
        // --no-url is shorthand for --routing-url ""
        Ok(Some("".to_string()))
    } else {
        Ok(None)
    }
}

pub fn handle_maybe_invalid_routing_url(
    maybe_invalid_routing_url: &Option<String>,
    // For testing purposes, we pass in stub `Write`er and `Read`ers to
    // simulate input and verify output.
    writer: &mut impl Write,
    reader: &mut impl Read,
    // Simulate a CI environment (non-TTY) for testing
    is_atty: bool,
) -> RoverResult<()> {
    // if a --routing-url is provided AND the URL is unparsable,
    // we need to warn and prompt the user, else we can assume a publish
    if let Some(routing_url) = maybe_invalid_routing_url {
        match Url::parse(routing_url) {
            Ok(parsed_url) => {
                tracing::debug!("Parsed URL: {}", parsed_url.to_string());
                let reason = format!("`{}` is not a valid routing URL. The `{}` protocol is not supported by the router. Valid protocols are `http` and `https`.", Style::Link.paint(routing_url), &parsed_url.scheme());
                if !["http", "https", "unix"].contains(&parsed_url.scheme()) {
                    if is_atty {
                        prompt_for_publish(
                            format!("{reason} Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?").as_str(),
                            reader,
                            writer,
                        )?;
                    } else {
                        non_tty_hard_error(&reason)?;
                    }
                } else if let Some(host) = parsed_url.host_str() {
                    if ["localhost", "127.0.0.1"].contains(&host) {
                        let reason = format!("The host `{}` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only.", host);
                        if is_atty {
                            prompt_for_publish(
                                format!("{reason} Would you still like to publish?").as_str(),
                                reader,
                                writer,
                            )?;
                        } else {
                            non_tty_warn_about_local_url(&reason, writer)?;
                        }
                    }
                }
            }
            Err(parse_error) => {
                tracing::debug!("Parse error: {}", parse_error.to_string());
                let reason = format!(
                    "`{}` is not a valid routing URL.",
                    Style::Link.paint(routing_url)
                );
                if is_atty {
                    prompt_for_publish(
                        format!("{} Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?", &reason).as_str(),
                        reader,
                        writer,
                    )?;
                } else {
                    non_tty_hard_error(&reason)?;
                }
            }
        }
    }
    Ok(())
}

fn prompt_for_publish(
    message: &str,
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> RoverResult<Option<bool>> {
    write!(writer, "{} [y/N] ", message)?;
    let mut response = [0];
    reader.read_exact(&mut response)?;
    if std::str::from_utf8(&response)?.to_lowercase() == *"y" {
        Ok(Some(true))
    } else {
        Err(anyhow!("You cancelled a subgraph publish due to an invalid routing url.").into())
    }
}

fn non_tty_hard_error(reason: &str) -> RoverResult<()> {
    Err(RoverError::new(anyhow!("{reason}"))
        .with_suggestion(RoverErrorSuggestion::AllowInvalidRoutingUrlOrSpecifyValidUrl))
}

fn non_tty_warn_about_local_url(reason: &str, writer: &mut dyn Write) -> RoverResult<()> {
    writeln!(writer, "{} {reason}", Style::WarningPrefix.paint("WARN:"),)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::command::subgraph::publish_shared::{
        determine_routing_url_with_test_params, handle_maybe_invalid_routing_url,
    };

    #[tokio::test]
    async fn test_no_url() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url_with_test_params(
            true,
            &None,
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();
        assert_eq!(result, Some("".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_provided() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url_with_test_params(
            false,
            &Some("https://provided".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();
        assert_eq!(result, Some("https://provided".to_string()));
    }

    #[tokio::test]
    async fn test_no_url_and_routing_url_provided() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url_with_test_params(
            true,
            &Some("https://provided".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap_err();
        assert_eq!(
            result.message(),
            "You cannot use --no-url and --routing-url at the same time."
        );
    }

    #[tokio::test]
    async fn test_routing_url_not_provided_already_exists() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url_with_test_params(
            false,
            &None,
            false,
            || async { Ok("https://fromstudio".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("https://fromstudio".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_unix_socket() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url_with_test_params(
            false,
            &None,
            false,
            || async { Ok("unix:///path/to/subgraph.sock".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("unix:///path/to/subgraph.sock".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_invalid_provided() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();

        let result = determine_routing_url_with_test_params(
            false,
            &Some("invalid".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("invalid".to_string()));
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[tokio::test]
    async fn test_not_url_invalid_from_studio() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();

        let result = determine_routing_url_with_test_params(
            true,
            &None,
            false,
            || async { Ok("invalid".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("".to_string()));
        assert!(std::str::from_utf8(&output).unwrap().is_empty());
    }

    #[test]
    fn test_confirm_invalid_url_publish() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[test]
    fn test_deny_invalid_url_publish() {
        let mut input = "n".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("You cancelled a subgraph publish due to an invalid routing url."));
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[test]
    fn test_invalid_scheme() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("ftp://invalid-scheme".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "is not a valid routing URL. The `ftp` protocol is not supported by the router. Valid protocols are `http` and `https`."
        ));
    }

    #[test]
    fn test_localhost_tty() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("http://localhost:8000".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "The host `localhost` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only."
        ));
    }

    #[test]
    fn test_localhost_no_tty() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("http://localhost:8000".to_string()),
            &mut output,
            &mut input,
            false,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "The host `localhost` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only."
        ));
    }

    #[test]
    fn test_invalid_url_no_tty() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            false,
        );

        assert!(result.is_err());
        assert!(input.is_empty());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("is not a valid routing URL."));
    }
}
