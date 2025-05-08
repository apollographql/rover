#![warn(missing_docs)]
use rover_std::{hyperlink, Style};
pub struct VersionUpgradeMessage {}

impl VersionUpgradeMessage {
    pub fn print() {
        eprintln!();
        eprintln!(
            "{}",
            Style::WarningHeading.paint("** Notice: Changes in This Release! **")
        );
        eprintln!(
            "This version includes significant updates to the `{}` command.",
            Style::Command.paint("rover dev")
        );
        eprintln!("We highly recommend reviewing the updated documentation to ensure a smooth experience.");
        eprintln!(
            "Read more: {}",
            hyperlink("https://www.apollographql.com/docs/rover/commands/dev")
        );
        eprintln!();
    }
}
