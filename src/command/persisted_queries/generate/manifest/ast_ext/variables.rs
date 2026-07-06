#![allow(dead_code)]

use std::collections::BTreeSet;

use apollo_compiler::{Node, ast};

/// Returns all variable names referenced in any argument of any directive in `directives`.
pub(super) fn collect_variables_from_directives(
    directives: &ast::DirectiveList,
) -> BTreeSet<String> {
    directives
        .iter()
        .flat_map(|d| collect_variables_from_arguments(&d.arguments))
        .collect()
}

/// Returns all variable names referenced across the given `arguments`.
pub(super) fn collect_variables_from_arguments(
    arguments: &[Node<ast::Argument>],
) -> BTreeSet<String> {
    arguments
        .iter()
        .flat_map(|arg| collect_variables_from_value(&arg.value))
        .collect()
}

/// Recursively returns every variable name found within `value`.
pub(super) fn collect_variables_from_value(value: &Node<ast::Value>) -> BTreeSet<String> {
    match value.as_ref() {
        ast::Value::Variable(name) => std::iter::once(name.to_string()).collect(),
        ast::Value::List(values) => values
            .iter()
            .flat_map(collect_variables_from_value)
            .collect(),
        ast::Value::Object(fields) => fields
            .iter()
            .flat_map(|(_, v)| collect_variables_from_value(v))
            .collect(),
        ast::Value::Null
        | ast::Value::Enum(_)
        | ast::Value::String(_)
        | ast::Value::Float(_)
        | ast::Value::Int(_)
        | ast::Value::Boolean(_) => BTreeSet::new(),
    }
}
