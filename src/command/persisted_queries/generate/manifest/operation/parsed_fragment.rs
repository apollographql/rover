use std::collections::BTreeSet;

use apollo_compiler::{Node, ast};
use camino::Utf8PathBuf;

#[derive(Debug, Clone)]
pub(super) struct ParsedFragment {
    pub(super) file: Utf8PathBuf,
    pub(super) fragment: Node<ast::FragmentDefinition>,
    pub(super) direct_fragment_spreads: BTreeSet<String>,
}
