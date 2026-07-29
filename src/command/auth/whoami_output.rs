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
