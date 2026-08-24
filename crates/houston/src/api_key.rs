//! The shape of a registry API key.

use std::fmt;

/// What a registry API key authenticates as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyActor {
    /// A Personal API Key (`user:...`), belonging to a person.
    User,

    /// A graph API key (`service:...`), scoped to one graph.
    Graph,

    /// Something else. Not an error: the registry is free to introduce actors that this
    /// version of Rover predates, and a key Rover doesn't recognize may still be valid.
    Other,
}

/// An API key that doesn't match the shape the registry documents.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("an API key must have three non-empty parts separated by colons")]
pub struct MalformedApiKey;

/// A registry API key in the `actor:id:secret` shape the registry documents:
/// `user:my-username:secretkey` or `service:graph-id:secretkey`.
///
/// Borrows the key it was parsed from, so the secret is never copied.
///
/// Parsing is deliberately not enforced when a credential is loaded. Only the registry can say
/// whether a key works, and refusing to send one Rover merely finds surprising would turn a
/// cosmetic disagreement into a hard failure. Callers parse when they want the shape, and treat
/// [`MalformedApiKey`] as evidence about a rejection that already happened.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApiKey<'a> {
    actor: ApiKeyActor,
    id: &'a str,
    secret: &'a str,
}

impl<'a> ApiKey<'a> {
    /// What the key authenticates as.
    pub const fn actor(&self) -> ApiKeyActor {
        self.actor
    }

    /// The username or graph id the key belongs to.
    pub const fn id(&self) -> &'a str {
        self.id
    }

    /// The secret portion. Treat as sensitive: never log or display it.
    pub const fn secret(&self) -> &'a str {
        self.secret
    }
}

impl<'a> TryFrom<&'a str> for ApiKey<'a> {
    type Error = MalformedApiKey;

    /// A colon inside the secret is tolerated, since a working key that Rover calls malformed is
    /// worse than a broken one it fails to spot.
    fn try_from(key: &'a str) -> Result<ApiKey<'a>, MalformedApiKey> {
        let mut parts = key.splitn(3, ':');
        let (Some(actor), Some(id), Some(secret)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(MalformedApiKey);
        };
        if actor.is_empty() || id.is_empty() || secret.is_empty() {
            return Err(MalformedApiKey);
        }
        Ok(ApiKey {
            actor: match actor {
                "user" => ApiKeyActor::User,
                "service" => ApiKeyActor::Graph,
                _ => ApiKeyActor::Other,
            },
            id,
            secret,
        })
    }
}

/// Redacts the secret, so an [`ApiKey`] can be logged without leaking the key it came from.
impl fmt::Debug for ApiKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiKey")
            .field("actor", &self.actor)
            .field("id", &self.id)
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::personal_key(
        "user:my-username:secretkey",
        ApiKeyActor::User,
        "my-username",
        "secretkey"
    )]
    #[case::graph_key(
        "service:graph-id:secretkey",
        ApiKeyActor::Graph,
        "graph-id",
        "secretkey"
    )]
    #[case::unknown_actor("robot:some-id:secretkey", ApiKeyActor::Other, "some-id", "secretkey")]
    #[case::colon_in_the_secret(
        "user:my-username:secret:key",
        ApiKeyActor::User,
        "my-username",
        "secret:key"
    )]
    fn parses_the_documented_shape(
        #[case] key: &str,
        #[case] actor: ApiKeyActor,
        #[case] id: &str,
        #[case] secret: &str,
    ) {
        let parsed = ApiKey::try_from(key).expect("should have parsed");

        assert_that!(parsed.actor()).is_equal_to(actor);
        assert_that!(parsed.id()).is_equal_to(id);
        assert_that!(parsed.secret()).is_equal_to(secret);
    }

    #[rstest]
    #[case::no_colons("not-a-real-key")]
    #[case::too_few_parts("user:secretkey")]
    #[case::empty_actor(":my-username:secretkey")]
    #[case::empty_id("user::secretkey")]
    #[case::empty_secret("user:my-username:")]
    #[case::empty_key("")]
    fn rejects_anything_else(#[case] key: &str) {
        assert_that!(ApiKey::try_from(key)).is_equal_to(Err(MalformedApiKey));
    }

    // The whole point of the manual Debug impl.
    #[test]
    fn debug_does_not_leak_the_secret() {
        let parsed = ApiKey::try_from("user:my-username:secretkey").unwrap();

        let rendered = format!("{parsed:?}");

        assert_that!(rendered).does_not_contain("secretkey");
        assert_that!(rendered).contains("my-username");
    }
}
