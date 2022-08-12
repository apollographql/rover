pub mod client;
pub mod env;
pub mod parsers;
pub mod pkg;
pub mod stringify;
pub mod table;
pub mod telemetry;
pub mod version;

pub fn confirm_delete() -> std::io::Result<bool> {
    prompt_confirm_default_no("Would you like to continue?")
}

pub fn prompt_confirm_default_no(message: &str) -> std::io::Result<bool> {
    eprintln!("{} [y/N]", message);
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn prompt_confirm_default_yes(message: &str) -> std::io::Result<bool> {
    eprintln!("{} [Y/n]", message);
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"n" {
        Ok(false)
    } else {
        Ok(true)
    }
}
