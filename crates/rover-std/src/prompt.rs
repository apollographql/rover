use crate::Style;

pub fn confirm_delete() -> std::io::Result<bool> {
    prompt_confirm_default_no(&Style::Prompt.paint("Would you like to continue?"))
}

pub fn prompt_confirm_default_no(message: &str) -> std::io::Result<bool> {
    eprintln!("{} [y/N]", &Style::Prompt.paint(message));
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn prompt_confirm_default_yes(message: &str) -> std::io::Result<bool> {
    eprintln!("{} [Y/n]", &Style::Prompt.paint(message));
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"n" {
        Ok(false)
    } else {
        Ok(true)
    }
}
