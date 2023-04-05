pub fn confirm_delete() -> std::io::Result<bool> {
    prompt_confirm_default_no("Would you like to continue?", None)
}

pub fn prompt_confirm_default_no(
    message: &str,
    stdin: Option<&mut dyn std::io::Read>,
) -> std::io::Result<bool> {
    eprint!("{} [y/N] ", message);

    let default_stdin = &mut std::io::stdin();
    let input = stdin.unwrap_or(default_stdin);
    let response = std::io::read_to_string(input)?;
    // let term = console::Term::stdout();
    // let confirm = term.read_line()?;
    if response.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn prompt_confirm_default_yes(message: &str) -> std::io::Result<bool> {
    eprint!("{} [Y/n] ", message);
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"n" {
        Ok(false)
    } else {
        Ok(true)
    }
}
