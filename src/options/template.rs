use std::fmt::{self, Display};

use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use dialoguer::{Select, console::Term};
use serde::{Deserialize, Serialize};

use crate::{
    RoverError, RoverResult,
    command::template::queries::{get_templates_for_language, list_templates_for_language},
};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// Filter templates by the available language
    #[arg(long = "language", value_enum)]
    pub language: Option<ProjectLanguage>,
}

impl TemplateOpt {
    pub fn get_or_prompt_language(&self) -> RoverResult<ProjectLanguage> {
        if let Some(language) = &self.language {
            Ok(language.clone())
        } else {
            let languages = <ProjectLanguage as ValueEnum>::value_variants();

            let selection = Select::new()
                .with_prompt("What language are you planning on using for the project?")
                .items(languages)
                .default(0)
                .interact_on_opt(&Term::stderr())?;

            selection
                .map(|index| languages[index].clone())
                .ok_or_else(|| RoverError::new(anyhow!("No language selected")))
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectLanguage {
    CSharp,
    Go,
    Java,
    Javascript,
    Kotlin,
    Python,
    Rust,
    Typescript,
    #[clap(skip)]
    Other(String),
}

impl Display for ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProjectLanguage::*;
        let readable = match self {
            CSharp => "C#",
            Go => "Go",
            Java => "Java",
            Javascript => "JavaScript",
            Kotlin => "Kotlin",
            Python => "Python",
            Rust => "Rust",
            Typescript => "TypeScript",
            Other(other) => other,
        };
        write!(f, "{readable}")
    }
}

impl From<ProjectLanguage> for get_templates_for_language::Language {
    fn from(language: ProjectLanguage) -> get_templates_for_language::Language {
        match language {
            ProjectLanguage::CSharp => get_templates_for_language::Language::C_SHARP,
            ProjectLanguage::Go => get_templates_for_language::Language::GO,
            ProjectLanguage::Java => get_templates_for_language::Language::JAVA,
            ProjectLanguage::Javascript => get_templates_for_language::Language::JAVASCRIPT,
            ProjectLanguage::Kotlin => get_templates_for_language::Language::KOTLIN,
            ProjectLanguage::Python => get_templates_for_language::Language::PYTHON,
            ProjectLanguage::Rust => get_templates_for_language::Language::RUST,
            ProjectLanguage::Typescript => get_templates_for_language::Language::TYPESCRIPT,
            ProjectLanguage::Other(other) => get_templates_for_language::Language::Other(other),
        }
    }
}

impl From<ProjectLanguage> for list_templates_for_language::Language {
    fn from(language: ProjectLanguage) -> list_templates_for_language::Language {
        match language {
            ProjectLanguage::CSharp => list_templates_for_language::Language::C_SHARP,
            ProjectLanguage::Go => list_templates_for_language::Language::GO,
            ProjectLanguage::Java => list_templates_for_language::Language::JAVA,
            ProjectLanguage::Javascript => list_templates_for_language::Language::JAVASCRIPT,
            ProjectLanguage::Kotlin => list_templates_for_language::Language::KOTLIN,
            ProjectLanguage::Python => list_templates_for_language::Language::PYTHON,
            ProjectLanguage::Rust => list_templates_for_language::Language::RUST,
            ProjectLanguage::Typescript => list_templates_for_language::Language::TYPESCRIPT,
            ProjectLanguage::Other(other) => list_templates_for_language::Language::Other(other),
        }
    }
}

impl From<list_templates_for_language::Language> for ProjectLanguage {
    fn from(language: list_templates_for_language::Language) -> Self {
        match language {
            list_templates_for_language::Language::C_SHARP => ProjectLanguage::CSharp,
            list_templates_for_language::Language::GO => ProjectLanguage::Go,
            list_templates_for_language::Language::JAVA => ProjectLanguage::Java,
            list_templates_for_language::Language::JAVASCRIPT => ProjectLanguage::Javascript,
            list_templates_for_language::Language::KOTLIN => ProjectLanguage::Kotlin,
            list_templates_for_language::Language::PYTHON => ProjectLanguage::Python,
            list_templates_for_language::Language::RUST => ProjectLanguage::Rust,
            list_templates_for_language::Language::TYPESCRIPT => ProjectLanguage::Typescript,
            list_templates_for_language::Language::Other(other) => ProjectLanguage::Other(other),
        }
    }
}
