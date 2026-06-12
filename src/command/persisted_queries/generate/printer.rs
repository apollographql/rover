use std::sync::Arc;

use apollo_compiler::{Node, ast};

/// A GraphQL definition paired with its original source text.
///
/// `source` is required to detect block-string literals by comparing
/// source span offsets against the raw text.
pub(super) struct PrintableDefinition {
    pub(super) source: Arc<str>,
    pub(super) ast_node: DefinitionNode,
}

pub(super) enum DefinitionNode {
    Operation(Node<ast::OperationDefinition>),
    Fragment(Node<ast::FragmentDefinition>),
}

impl PrintableDefinition {
    fn print(&self) -> String {
        let mut output = String::new();
        match &self.ast_node {
            DefinitionNode::Operation(op) => print_operation(&mut output, op, &self.source),
            DefinitionNode::Fragment(frag) => print_fragment(&mut output, frag, &self.source),
        }
        output
    }
}

/// Renders `definitions` as a single GraphQL executable document body, separated by blank lines.
pub(super) fn print_document(definitions: &[PrintableDefinition]) -> String {
    definitions
        .iter()
        .map(PrintableDefinition::print)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn print_operation(output: &mut String, operation: &ast::OperationDefinition, source: &str) {
    output.push_str(operation_type_str(operation.operation_type));
    if let Some(name) = &operation.name {
        output.push(' ');
        output.push_str(name);
    }
    print_variable_definitions(output, &operation.variables, source);
    print_directives(output, &operation.directives, source, 0);
    output.push(' ');
    print_selection_set(output, &operation.selection_set, source, 0);
}

fn print_fragment(output: &mut String, fragment: &ast::FragmentDefinition, source: &str) {
    output.push_str("fragment ");
    output.push_str(&fragment.name);
    output.push_str(" on ");
    output.push_str(&fragment.type_condition);
    print_directives(output, &fragment.directives, source, 0);
    output.push(' ');
    print_selection_set(output, &fragment.selection_set, source, 0);
}

fn print_selection_set(
    output: &mut String,
    selections: &[ast::Selection],
    source: &str,
    indent: usize,
) {
    output.push_str("{\n");
    for selection in selections {
        push_indent(output, indent + 2);
        print_selection(output, selection, source, indent + 2);
        output.push('\n');
    }
    push_indent(output, indent);
    output.push('}');
}

fn print_selection(output: &mut String, selection: &ast::Selection, source: &str, indent: usize) {
    match selection {
        ast::Selection::Field(field) => print_field(output, field, source, indent),
        ast::Selection::FragmentSpread(fragment_spread) => {
            output.push_str("...");
            output.push_str(&fragment_spread.fragment_name);
            print_directives(output, &fragment_spread.directives, source, indent);
        }
        ast::Selection::InlineFragment(inline_fragment) => {
            output.push_str("...");
            if let Some(type_condition) = &inline_fragment.type_condition {
                output.push_str(" on ");
                output.push_str(type_condition);
            }
            print_directives(output, &inline_fragment.directives, source, indent);
            output.push(' ');
            print_selection_set(output, &inline_fragment.selection_set, source, indent);
        }
    }
}

fn print_field(output: &mut String, field: &ast::Field, source: &str, indent: usize) {
    if let Some(alias) = &field.alias {
        output.push_str(alias);
        output.push_str(": ");
    }
    output.push_str(&field.name);
    print_arguments(output, &field.arguments, source, indent);
    print_directives(output, &field.directives, source, indent);
    if !field.selection_set.is_empty() {
        output.push(' ');
        print_selection_set(output, &field.selection_set, source, indent);
    }
}

fn print_variable_definitions(
    output: &mut String,
    variables: &[Node<ast::VariableDefinition>],
    source: &str,
) {
    if variables.is_empty() {
        return;
    }

    output.push('(');
    for (idx, variable) in variables.iter().enumerate() {
        if idx > 0 {
            output.push_str(", ");
        }
        output.push('$');
        output.push_str(&variable.name);
        output.push_str(": ");
        print_type(output, &variable.ty);
        if let Some(default_value) = &variable.default_value {
            output.push_str(" = ");
            print_value(output, default_value, source, 0);
        }
        print_directives(output, &variable.directives, source, 0);
    }
    output.push(')');
}

fn print_type(output: &mut String, ty: &ast::Type) {
    match ty {
        ast::Type::Named(name) => output.push_str(name),
        ast::Type::NonNullNamed(name) => {
            output.push_str(name);
            output.push('!');
        }
        ast::Type::List(inner) => {
            output.push('[');
            print_type(output, inner);
            output.push(']');
        }
        ast::Type::NonNullList(inner) => {
            output.push('[');
            print_type(output, inner);
            output.push_str("]!");
        }
    }
}

fn print_directives(
    output: &mut String,
    directives: &ast::DirectiveList,
    source: &str,
    indent: usize,
) {
    for directive in directives.iter() {
        output.push(' ');
        output.push('@');
        output.push_str(&directive.name);
        print_arguments(output, &directive.arguments, source, indent);
    }
}

fn print_arguments(
    output: &mut String,
    arguments: &[Node<ast::Argument>],
    source: &str,
    indent: usize,
) {
    if arguments.is_empty() {
        return;
    }

    output.push('(');
    for (idx, argument) in arguments.iter().enumerate() {
        if idx > 0 {
            output.push_str(", ");
        }
        output.push_str(&argument.name);
        output.push_str(": ");
        print_value(output, &argument.value, source, indent);
    }
    output.push(')');
}

fn print_value(output: &mut String, value_node: &Node<ast::Value>, source: &str, indent: usize) {
    match value_node.as_ref() {
        ast::Value::Null => output.push_str("null"),
        ast::Value::Enum(name) | ast::Value::Variable(name) => {
            if matches!(value_node.as_ref(), ast::Value::Variable(_)) {
                output.push('$');
            }
            output.push_str(name);
        }
        ast::Value::String(value) => {
            if value_was_block_string(value_node, source) {
                print_block_string(output, value, indent);
            } else {
                output.push_str(
                    &ast::Value::String(value.clone())
                        .serialize()
                        .no_indent()
                        .to_string(),
                );
            }
        }
        ast::Value::Float(value) => output.push_str(&value.to_string()),
        ast::Value::Int(value) => output.push_str(&value.to_string()),
        ast::Value::Boolean(value) => output.push_str(if *value { "true" } else { "false" }),
        ast::Value::List(values) => {
            output.push('[');
            for (idx, value) in values.iter().enumerate() {
                if idx > 0 {
                    output.push_str(", ");
                }
                print_value(output, value, source, indent);
            }
            output.push(']');
        }
        ast::Value::Object(fields) => {
            output.push('{');
            for (idx, (name, value)) in fields.iter().enumerate() {
                if idx > 0 {
                    output.push_str(", ");
                }
                output.push_str(name);
                output.push_str(": ");
                print_value(output, value, source, indent);
            }
            output.push('}');
        }
    }
}

// Uses the node's source span to read the raw token from the original source.
fn value_was_block_string(value: &Node<ast::Value>, source: &str) -> bool {
    value
        .location()
        .and_then(|location| source.get(location.offset()..location.end_offset()))
        .is_some_and(|source_text| source_text.trim_start().starts_with("\"\"\""))
}

fn print_block_string(output: &mut String, value: &str, indent: usize) {
    output.push_str("\"\"\"");
    output.push('\n');
    for line in value.split('\n') {
        push_indent(output, indent);
        output.push_str(&line.replace("\"\"\"", "\\\"\"\""));
        output.push('\n');
    }
    push_indent(output, indent);
    output.push_str("\"\"\"");
}

fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push(' ');
    }
}

/// Returns the lowercase keyword for `operation_type` ("query", "mutation", "subscription").
pub(super) const fn operation_type_str(operation_type: ast::OperationType) -> &'static str {
    match operation_type {
        ast::OperationType::Query => "query",
        ast::OperationType::Mutation => "mutation",
        ast::OperationType::Subscription => "subscription",
    }
}

#[cfg(test)]
mod tests {
    use apollo_compiler::{Node, parser::Parser as ApolloParser};
    use speculoos::prelude::*;

    use super::*;

    fn parse_print(src: &str) -> String {
        let doc = ApolloParser::new().parse_ast(src, "test.graphql").unwrap();
        let source: std::sync::Arc<str> = src.into();
        let defs: Vec<PrintableDefinition> = doc
            .definitions
            .into_iter()
            .filter_map(|d| match d {
                ast::Definition::OperationDefinition(op) => Some(PrintableDefinition {
                    source: source.clone(),
                    ast_node: DefinitionNode::Operation(op),
                }),
                ast::Definition::FragmentDefinition(frag) => Some(PrintableDefinition {
                    source: source.clone(),
                    ast_node: DefinitionNode::Fragment(frag),
                }),
                _ => None,
            })
            .collect();
        print_document(&defs)
    }

    fn print_type_str(ty: ast::Type) -> String {
        let mut out = String::new();
        print_type(&mut out, &ty);
        out
    }

    fn print_value_str(value: ast::Value) -> String {
        let mut out = String::new();
        print_value(&mut out, &Node::new(value), "", 0);
        out
    }

    #[test]
    fn operation_type_str_returns_correct_strings() {
        assert_that!(operation_type_str(ast::OperationType::Query)).is_equal_to("query");
        assert_that!(operation_type_str(ast::OperationType::Mutation)).is_equal_to("mutation");
        assert_that!(operation_type_str(ast::OperationType::Subscription))
            .is_equal_to("subscription");
    }

    #[test]
    fn prints_field_with_alias() {
        let out = parse_print("query Q { display: name }");
        assert_that!(out).contains("display: name".to_string());
    }

    #[test]
    fn prints_inline_fragment_without_type_condition() {
        let out = parse_print("query Q { ... { id } }");
        assert_that!(out).contains("... {".to_string());
    }

    #[test]
    fn prints_inline_fragment_with_type_condition() {
        let out = parse_print("query Q { ... on User { id } }");
        assert_that!(out).contains("... on User {".to_string());
    }

    #[test]
    fn print_type_named() {
        assert_that!(print_type_str(ast::Type::Named(apollo_compiler::name!(
            "User"
        ))))
        .is_equal_to("User".to_string());
    }

    #[test]
    fn print_type_non_null_named() {
        assert_that!(print_type_str(ast::Type::NonNullNamed(
            apollo_compiler::name!("ID")
        )))
        .is_equal_to("ID!".to_string());
    }

    #[test]
    fn print_type_list() {
        assert_that!(print_type_str(ast::Type::List(Box::new(ast::Type::Named(
            apollo_compiler::name!("String")
        )))))
        .is_equal_to("[String]".to_string());
    }

    #[test]
    fn print_type_non_null_list() {
        assert_that!(print_type_str(ast::Type::NonNullList(Box::new(
            ast::Type::Named(apollo_compiler::name!("Int"))
        ))))
        .is_equal_to("[Int]!".to_string());
    }

    #[test]
    fn print_value_null() {
        assert_that!(print_value_str(ast::Value::Null)).is_equal_to("null".to_string());
    }

    #[test]
    fn print_value_enum() {
        assert_that!(print_value_str(ast::Value::Enum(apollo_compiler::name!(
            "ACTIVE"
        ))))
        .is_equal_to("ACTIVE".to_string());
    }

    #[test]
    fn print_value_variable() {
        assert_that!(print_value_str(ast::Value::Variable(
            apollo_compiler::name!("userId")
        )))
        .is_equal_to("$userId".to_string());
    }

    #[test]
    fn print_value_string() {
        assert_that!(print_value_str(ast::Value::String("hello".into())))
            .is_equal_to(r#""hello""#.to_string());
    }

    #[test]
    fn print_value_int() {
        assert_that!(print_value_str(ast::Value::Int(ast::IntValue::from(
            42_i32
        ))))
        .is_equal_to("42".to_string());
    }

    #[test]
    fn print_value_float() {
        assert_that!(print_value_str(ast::Value::Float(ast::FloatValue::from(
            3.14_f64
        ))))
        .is_equal_to("3.14".to_string());
    }

    #[test]
    fn print_value_boolean() {
        assert_that!(print_value_str(ast::Value::Boolean(true))).is_equal_to("true".to_string());
        assert_that!(print_value_str(ast::Value::Boolean(false))).is_equal_to("false".to_string());
    }

    #[test]
    fn print_value_list() {
        let list = ast::Value::List(vec![
            Node::new(ast::Value::Int(ast::IntValue::from(1_i32))),
            Node::new(ast::Value::Int(ast::IntValue::from(2_i32))),
        ]);
        assert_that!(print_value_str(list)).is_equal_to("[1, 2]".to_string());
    }

    #[test]
    fn print_value_object() {
        let obj = ast::Value::Object(vec![(
            apollo_compiler::name!("id"),
            Node::new(ast::Value::Variable(apollo_compiler::name!("userId"))),
        )]);
        assert_that!(print_value_str(obj)).is_equal_to("{id: $userId}".to_string());
    }
}
