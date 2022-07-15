use camino::Utf8PathBuf;
use clap::Parser;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Serialize;

use std::sync::mpsc::channel;
use std::time::Duration;
use std::{fs, panic};

use apollo_compiler::ApolloCompiler;

use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::options::SchemaOpt;
use crate::utils::parsers::FileDescriptorType;
use crate::{anyhow, Result};

#[derive(Debug, Serialize, Parser)]
pub struct Lint {
    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema_opt: SchemaOpt,

    /// Enable file watching for your schema.
    ///
    /// This option is incompatible with `--schema -`.
    #[clap(long)]
    watch: bool,

    /// Configures whether to fail if there are validation warnings.
    #[clap(long)]
    strict: bool,
}

impl Lint {
    pub fn run(&self) -> Result<RoverOutput> {
        if !self.watch {
            let (proposed_schema, _) = self.get_schema_and_maybe_path()?;
            self.lint(&proposed_schema)
        } else {
            self.lint_and_watch()
        }
    }

    fn get_schema_and_maybe_path(&self) -> Result<(String, Option<Utf8PathBuf>)> {
        let path = if let FileDescriptorType::File(path) = &self.schema_opt.schema {
            Some(path.clone())
        } else {
            None
        };
        Ok((
            self.schema_opt
                .read_file_descriptor("SDL", &mut std::io::stdin())?,
            path,
        ))
    }

    fn print_lint(&self, proposed_schema: &str) {
        match self.lint(proposed_schema) {
            Ok(output) => {
                let _ = output.print();
            }
            Err(err) => {
                let _ = err.print();
            }
        }
    }

    fn lint_and_watch(&self) -> Result<RoverOutput> {
        let (proposed_schema, maybe_path) = self.get_schema_and_maybe_path()?;

        if let Some(path) = maybe_path {
            self.print_lint(&proposed_schema);

            let (broadcaster, listener) = channel();
            let mut watcher = watcher(broadcaster, Duration::from_secs(1))?;
            watcher.watch(&path, RecursiveMode::NonRecursive)?;

            eprintln!("👀 Watching {} for changes...", path);
            loop {
                match listener.recv() {
                    Ok(event) => match &event {
                        DebouncedEvent::Write(_) => {
                            eprintln!("🔃 Change detected in {}...", &path);
                            self.print_lint(&fs::read_to_string(&path)?);
                        }
                        DebouncedEvent::Error(e, _) => {
                            tracing::debug!("unknown error while watching {}: {}", &path, e);
                        }
                        _ => {}
                    },
                    Err(e) => tracing::debug!("unknown error while watching {}: {:?}", &path, e),
                }
            }
        } else {
            Err(RoverError::new(anyhow!(
                "You cannot combine the `--watch` flag with the `--schema -` argument."
            )))
        }
    }

    fn lint(&self, proposed_schema: &str) -> Result<RoverOutput> {
        let compiler_context = ApolloCompiler::new(proposed_schema);
        eprintln!(
            "🤔 Validating {}...",
            match &self.schema_opt.schema {
                FileDescriptorType::File(path) => path.as_str(),
                FileDescriptorType::Stdin => "`--schema -`",
            }
        );
        let old_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {
            let _ = RoverError::new(anyhow!(
                "fatal: `apollo-compiler` was unable to parse your schema"
            ))
            .print();
        }));
        let diagnostics = compiler_context.validate();
        panic::set_hook(old_hook);
        if !diagnostics.is_empty() {
            let mut num_errors: usize = 0;
            let mut num_warnings: usize = 0;
            diagnostics.iter().for_each(|diagnostic| {
                let diagnostic = diagnostic.to_string();
                eprintln!("{}", &diagnostic);
                if diagnostic.contains("apollo-compiler validation advice") {
                    num_warnings += 1;
                } else if diagnostic.contains("apollo-compiler validation error") {
                    num_errors += 1;
                }
            });
            let mut failed = num_errors > 0;
            if self.strict && !failed && num_warnings > 0 {
                failed = true;
            }
            if failed {
                return Err(RoverError::new(anyhow!(
                    "The schema contained {} error{} and {} warning{}.",
                    num_errors,
                    match num_errors {
                        1 => "",
                        _ => "s",
                    },
                    num_warnings,
                    match num_warnings {
                        1 => "",
                        _ => "s",
                    }
                )));
            }
        }
        Ok(RoverOutput::LintSuccess)
    }
}
