use std::collections::{BTreeMap, BTreeSet};

use apollo_compiler::{Node, ast};
use camino::Utf8PathBuf;

use super::parsed_fragment::ParsedFragment;
use super::super::{
    ast_ext::{FragmentDefinitionExt, OperationDefinitionExt, SelectionSetExt},
    error::GenerateError,
    printer::{PrintableDefinition, print_document},
};
use crate::RoverResult;

#[derive(Debug, Clone)]
pub(super) struct ParsedOperation {
    pub(super) file: Utf8PathBuf,
    pub(super) operation: Node<ast::OperationDefinition>,
    pub(super) direct_fragment_spreads: BTreeSet<String>,
}

impl ParsedOperation {
    pub(super) fn reachable_fragment_names(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> RoverResult<BTreeSet<String>> {
        let mut reachable = BTreeSet::new();
        let mut queue: Vec<&str> = self
            .direct_fragment_spreads
            .iter()
            .map(String::as_str)
            .collect();

        while let Some(fragment_name) = queue.pop() {
            if !reachable.insert(fragment_name.to_string()) {
                continue;
            }
            let fragment =
                all_fragments
                    .get(fragment_name)
                    .ok_or_else(|| GenerateError::MissingFragment {
                        operation_name: name.to_string(),
                        fragment_name: fragment_name.to_string(),
                    })?;
            queue.extend(fragment.direct_fragment_spreads.iter().map(String::as_str));
        }

        Ok(reachable)
    }

    pub(super) fn body(
        &self,
        name: &str,
        all_fragments: &BTreeMap<String, ParsedFragment>,
    ) -> RoverResult<String> {
        let reachable = self.reachable_fragment_names(name, all_fragments)?;
        let mut operation_node = self.operation.clone();
        {
            let op_mut = operation_node.make_mut();
            op_mut.selection_set.remove_client_selections();
            op_mut.directives.0.retain(|d| d.name != "client");
        }

        let fragment_definitions: Vec<Node<ast::FragmentDefinition>> = reachable
            .iter()
            .map(|fragment_name| {
                let fragment = all_fragments
                    .get(fragment_name)
                    .expect("reachable fragments are validated before returning");
                let mut fragment_node = fragment.fragment.clone();
                let fragment_definition = fragment_node.make_mut();
                fragment_definition
                    .directives
                    .0
                    .retain(|directive| directive.name != "client");
                fragment_definition.selection_set.remove_client_selections();
                fragment_node
            })
            .collect();

        let op = operation_node.make_mut();
        let used: BTreeSet<String> = std::iter::once(op.collect_variables())
            .chain(fragment_definitions.iter().map(|f| f.collect_variables()))
            .fold(BTreeSet::new(), |mut acc, vars| {
                acc.extend(vars);
                acc
            });
        op.variables.retain(|v| used.contains(v.name.as_str()));

        let definitions = std::iter::once(PrintableDefinition::Operation(operation_node))
            .chain(fragment_definitions.into_iter().map(PrintableDefinition::Fragment))
            .collect::<Vec<_>>();

        Ok(print_document(&definitions))
    }
}
