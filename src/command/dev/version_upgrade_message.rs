#![warn(missing_docs)]
use rover_std::Style;

pub struct VersionUpgradeMessage {}

impl VersionUpgradeMessage {
    pub fn print() {
        eprintln!();
        eprintln!(
            "{}",
            Style::WarningPrefix.paint("** Notice: Changes in This Release! **")
        );
        eprintln!("This version includes significant updates to the `rover dev` command.");
        eprintln!("We highly recommend reviewing the updated documentation to ensure a smooth experience.");
        eprintln!("Read more: https://www.apollographql.com/docs/rover/commands/dev");
        eprintln!();
    }
}
