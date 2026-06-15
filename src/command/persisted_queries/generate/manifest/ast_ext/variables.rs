#![allow(dead_code)]

use std::collections::BTreeSet;

use apollo_compiler::{Node, ast};

pub(super) fn collect_variables_from_directives(
    directives: &ast::DirectiveList,
    variables: &mut BTreeSet<String>,
) {
    for directive in directives.iter() {
        collect_variables_from_arguments(&directive.arguments, variables);
    }
}

pub(super) fn collect_variables_from_arguments(
    arguments: &[Node<ast::Argument>],
    variables: &mut BTreeSet<String>,
) {
    for argument in arguments {
        collect_variables_from_value(&argument.value, variables);
    }
}

pub(super) fn collect_variables_from_value(
    value: &Node<ast::Value>,
    variables: &mut BTreeSet<String>,
) {
    match value.as_ref() {
        ast::Value::Variable(name) => {
            variables.insert(name.to_string());
        }
        ast::Value::List(values) => {
            for value in values {
                collect_variables_from_value(value, variables);
            }
        }
        ast::Value::Object(fields) => {
            for (_, value) in fields {
                collect_variables_from_value(value, variables);
            }
        }
        ast::Value::Null
        | ast::Value::Enum(_)
        | ast::Value::String(_)
        | ast::Value::Float(_)
        | ast::Value::Int(_)
        | ast::Value::Boolean(_) => {}
    }
}
