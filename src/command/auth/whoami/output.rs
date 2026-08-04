use rover_std::Style;

use crate::{command::CliOutput, utils::table};

#[derive(Debug)]
pub(super) struct AuthWhoAmIOutput {
    pub(super) email: String,
    pub(super) name: String,
    pub(super) user_id: String,
    pub(super) origin: String,
    pub(super) access_token: String,
}

impl CliOutput for AuthWhoAmIOutput {
    fn text(&self) -> String {
        let mut table = table::get_table();

        table.add_row(vec![&Style::WhoAmIKey.paint("Name"), &self.name]);
        table.add_row(vec![&Style::WhoAmIKey.paint("Email"), &self.email]);
        table.add_row(vec![&Style::WhoAmIKey.paint("User ID"), &self.user_id]);
        table.add_row(vec![&Style::WhoAmIKey.paint("Origin"), &self.origin]);
        table.add_row(vec![
            &Style::WhoAmIKey.paint("Access Token"),
            &self.access_token,
        ]);

        table.to_string()
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::json!({
            "email": self.email,
            "name": self.name,
            "user_id": self.user_id,
            "origin": self.origin,
            "access_token": self.access_token,
        }))
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;

    use super::*;

    #[fixture]
    fn output() -> AuthWhoAmIOutput {
        AuthWhoAmIOutput {
            email: "grace@apollographql.com".to_string(),
            name: "Grace Hopper".to_string(),
            user_id: "user-123".to_string(),
            origin: "--profile default (OAuth)".to_string(),
            access_token: "an-access-token".to_string(),
        }
    }

    #[rstest]
    fn text_includes_every_field(output: AuthWhoAmIOutput) {
        let text = temp_env::with_var("NO_COLOR", Some("1"), || output.text());

        assert_that!(text).is_equal_to(
            indoc! {"
                ┌──────────────┬───────────────────────────┐
                │ Name         ┆ Grace Hopper              │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Email        ┆ grace@apollographql.com   │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ User ID      ┆ user-123                  │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Origin       ┆ --profile default (OAuth) │
                ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
                │ Access Token ┆ an-access-token           │
                └──────────────┴───────────────────────────┘"}
            .to_string(),
        );
    }

    #[rstest]
    fn json_matches_expected_shape(output: AuthWhoAmIOutput) {
        assert_that!(output.json())
            .is_ok()
            .is_equal_to(serde_json::json!({
                "email": "grace@apollographql.com",
                "name": "Grace Hopper",
                "user_id": "user-123",
                "origin": "--profile default (OAuth)",
                "access_token": "an-access-token",
            }));
    }
}
