use anyhow::Result;
use anyhow::*;
use regex::Regex;

/// this fn is to be used with structopt's argument parsing.
/// It takes a potential graph id and returns it as a String if it's valid, but
/// will return errors if not.
pub fn parse_graph_id(graph_id: &str) -> Result<String> {
    let pattern = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}$").unwrap();
    let variant_pattern =
        Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]{0,63}@[a-zA-Z][a-zA-Z0-9_-]{0,63}$").unwrap();

    let valid_id = pattern.is_match(graph_id);
    if valid_id {
        Ok(String::from(graph_id))
    } else {
        // if the id _seems_ right with the exception of a `@` in it, the user is
        // probably trying to pass the variant like graph@prod. Warn against that.
        // tell them to use the `--variant` flag instead.
        let id_with_variant = variant_pattern.is_match(graph_id);
        if id_with_variant {
            Err(anyhow!("Invalid graph ID. The character `@` is not supported in graph IDs. If you are trying to pass a variant in the ID. Use the `--variant` flag to specify graph variants."))
        } else {
            Err(anyhow!("Invalid graph ID. Graph IDs can only contain letters, numbers, or the characters `-` or `_`, and must be less than 64 characters."))
        }
    }
}

#[test]
fn parse_graph_id_works() {
    assert!(parse_graph_id("engine#%^").is_err());
    assert!(parse_graph_id("engine@okay").is_err());
    assert!(parse_graph_id(
        "1234567890123456789012345678901234567890123456789012345678901234567890"
    )
    .is_err());
    assert!(parse_graph_id("1boi").is_err());
    assert!(parse_graph_id("_eng").is_err());

    assert_eq!("studio".to_string(), parse_graph_id("studio").unwrap());
    assert_eq!(
        "this_should_work".to_string(),
        parse_graph_id("this_should_work").unwrap()
    );
    assert_eq!(
        "it-is-cool".to_string(),
        parse_graph_id("it-is-cool").unwrap()
    );
}
