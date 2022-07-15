use camino::Utf8PathBuf;
use clap::Parser;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use serde::Serialize;

use std::fs;
use std::sync::mpsc::channel;
use std::time::Duration;

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

    #[clap(long)]
    watch: bool,
}

impl Lint {
    pub fn run(&self) -> Result<RoverOutput> {
        if !self.watch {
            let (proposed_schema, _) = self.get_schema_and_maybe_path()?;
            self.lint(&proposed_schema)
        } else {
            self.watch()
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

    fn watch(&self) -> Result<RoverOutput> {
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
        let errors = compiler_context.validate();
        if !errors.is_empty() {
            errors.iter().for_each(|e| eprintln!("{}", e));
            let num_errors = errors.len();
            Err(RoverError::new(anyhow!(
                "The schema contained {} error{}.",
                num_errors,
                match num_errors {
                    1 => "",
                    _ => "s",
                }
            )))
        } else {
            Ok(RoverOutput::LintSuccess)
        }
    }
}
