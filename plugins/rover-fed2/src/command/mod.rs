mod compose;

pub(crate) use compose::Compose;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Compose a supergraph from a fully resolved supergraph config YAML
    Compose(Compose),
}
