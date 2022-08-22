use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
  /// The GitHub id for the templates repository
  #[clap(short = 't', long = "template")]
  #[serde(skip_serializing)]
  pub template: Option<String>,

  /// Filter templates by the available language
  #[clap(long = "language")]
  #[serde(skip_serializing)]
  pub language: Option<String>,

  /// Type of template project: client or subgraph
  #[clap(long = "project-type")]
  #[serde(skip_serializing)]
  pub project_type: Option<String>,
}
