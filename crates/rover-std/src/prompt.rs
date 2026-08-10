use std::io::{self, BufRead, IsTerminal, Write};

use crate::Style;

pub fn confirm_delete() -> io::Result<bool> {
    prompt_confirm_default_no(&Style::Prompt.paint("Would you like to continue?"))
}

pub fn prompt_confirm_default_no(message: &str) -> io::Result<bool> {
    let stdin = io::stdin();
    let is_atty = stdin.is_terminal();
    confirm_default_no(message, &mut stdin.lock(), &mut io::stderr(), is_atty)
}

pub fn prompt_confirm_default_yes(message: &str) -> io::Result<bool> {
    let stdin = io::stdin();
    let is_atty = stdin.is_terminal();
    confirm_default_yes(message, &mut stdin.lock(), &mut io::stderr(), is_atty)
}

fn confirm_default_no(
    message: &str,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    is_atty: bool,
) -> io::Result<bool> {
    writeln!(writer, "{} [y/N]", Style::Prompt.paint(message))?;
    if !is_atty {
        return Ok(false);
    }
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().eq_ignore_ascii_case("y"))
}

fn confirm_default_yes(
    message: &str,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    is_atty: bool,
) -> io::Result<bool> {
    writeln!(writer, "{} [Y/n]", Style::Prompt.paint(message))?;
    if !is_atty {
        return Ok(true);
    }
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(!line.trim().eq_ignore_ascii_case("n"))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn default_no_returns_false_without_reading_when_not_a_tty() {
        let mut reader = Cursor::new(b"y\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_no("accept?", &mut reader, &mut writer, false).unwrap();
        assert!(!accepted);
    }

    #[test]
    fn default_no_reads_y_and_accepts_when_a_tty() {
        let mut reader = Cursor::new(b"y\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_no("accept?", &mut reader, &mut writer, true).unwrap();
        assert!(accepted);
    }

    #[test]
    fn default_no_rejects_anything_else_when_a_tty() {
        let mut reader = Cursor::new(b"\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_no("accept?", &mut reader, &mut writer, true).unwrap();
        assert!(!accepted);
    }

    #[test]
    fn default_yes_returns_true_without_reading_when_not_a_tty() {
        let mut reader = Cursor::new(b"n\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_yes("accept?", &mut reader, &mut writer, false).unwrap();
        assert!(accepted);
    }

    #[test]
    fn default_yes_reads_n_and_rejects_when_a_tty() {
        let mut reader = Cursor::new(b"n\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_yes("accept?", &mut reader, &mut writer, true).unwrap();
        assert!(!accepted);
    }

    #[test]
    fn default_yes_accepts_anything_else_when_a_tty() {
        let mut reader = Cursor::new(b"\n".to_vec());
        let mut writer = Vec::new();
        let accepted = confirm_default_yes("accept?", &mut reader, &mut writer, true).unwrap();
        assert!(accepted);
    }

    #[test]
    fn writes_prompt_message_and_hint() {
        let mut reader = Cursor::new(b"\n".to_vec());
        let mut writer = Vec::new();
        confirm_default_no("accept?", &mut reader, &mut writer, false).unwrap();
        assert!(String::from_utf8(writer).unwrap().contains("[y/N]"));
    }
}
