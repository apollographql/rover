use std::fmt::Display;

use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use serde::Serialize;

use rover_std::Emoji;

#[derive(Debug, Serialize, Parser)]
/// Options for configuring prompts.
pub(crate) struct YesOrNoPromptOpts {
    /// Answer "yes" to all prompts.
    #[clap(short, long, alias = "confirm")]
    pub(crate) yes: bool,
}

pub(crate) type DefaultPromptAnswer = PromptAnswer;

#[derive(Debug)]
pub(crate) enum PromptAnswer {
    Yes,
    No,
}

impl PromptAnswer {
    fn default_descriptor(&self) -> &str {
        match &self {
            Self::Yes => "[Y/n]",
            Self::No => "[y/N]",
        }
    }
}

impl Display for PromptAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                Self::Yes => "yes",
                Self::No => "no",
            }
        )
    }
}

impl YesOrNoPromptOpts {
    /// Print extra information before prompting for user input.
    pub(crate) fn prompt_with_info<I, Q, O>(
        &self,
        info: I,
        question: Q,
        default_answer: PromptAnswer,
        operation: O,
    ) -> Result<PromptAnswer>
    where
        I: Display,
        Q: Display,
        O: Display,
    {
        if self.yes {
            return Ok(PromptAnswer::Yes);
        }
        eprintln!("{emoji}{info}", emoji = Emoji::Info);
        self.prompt(question, default_answer, operation)
    }

    /// Prompt for user input.
    pub(crate) fn prompt<Q, O>(
        &self,
        question: Q,
        default_answer: DefaultPromptAnswer,
        operation: O,
    ) -> Result<PromptAnswer>
    where
        Q: Display,
        O: Display,
    {
        if self.yes {
            return Ok(PromptAnswer::Yes);
        }
        Self::fail_on_uninteractive_terminal(operation)?;
        eprint!(
            "{emoji}{question} {yes_no} {default} ",
            emoji = Emoji::Question,
            yes_no = default_answer.default_descriptor().green().bold(),
            default = format!("(default: {default_answer})").italic()
        );
        let term = console::Term::stderr();
        let confirm = term.read_line()?;
        if confirm.to_lowercase() == *"y" {
            Ok(PromptAnswer::Yes)
        } else {
            Ok(PromptAnswer::No)
        }
    }

    /// If the terminal is not interactive, abort.
    fn fail_on_uninteractive_terminal<O>(operation: O) -> Result<()>
    where
        O: Display,
    {
        if atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stderr) {
            Ok(())
        } else {
            Err(anyhow!("{operation} cancelled."))
        }
    }
}
