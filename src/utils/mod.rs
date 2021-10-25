pub mod client;
pub mod env;
pub mod loaders;
pub mod parsers;
pub mod pkg;
pub mod stringify;
pub mod table;
pub mod telemetry;
pub mod version;

pub fn confirm_delete() -> std::io::Result<bool> {
    eprintln!("Would you like to continue? [y/n]");
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
    }
}
