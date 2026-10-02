mod client_credentials;
mod grants;
mod whoami;

use serde_json::{Value, json};

/// A Studio `whoami` answer for a user, shared by the tests that authenticate as one.
fn whoami_response() -> Value {
    json!({
        "data": {
            "me": {
                "__typename": "User",
                "id": "a-user-id",
                "asActor": { "type": "USER" }
            }
        }
    })
}
