use std::sync::Arc;

use apollo_compiler::{Node, ast};

pub(super) enum PrintableDefinition {
    Operation {
        operation: Node<ast::OperationDefinition>,
        source: Arc<str>,
    },
    Fragment {
        fragment: Node<ast::FragmentDefinition>,
        source: Arc<str>,
    },
}

pub(super) fn print_document(definitions: &[PrintableDefinition]) -> String {
    definitions
        .iter()
        .map(print_definition)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn print_definition(definition: &PrintableDefinition) -> String {
    let mut output = String::new();
    match definition {
        PrintableDefinition::Operation { operation, source } => {
            print_operation(&mut output, operation, source)
        }
        PrintableDefinition::Fragment { fragment, source } => {
            print_fragment(&mut output, fragment, source)
        }
    }
    output
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

pub(super) const fn operation_type_str(operation_type: ast::OperationType) -> &'static str {
    match operation_type {
        ast::OperationType::Query => "query",
        ast::OperationType::Mutation => "mutation",
        ast::OperationType::Subscription => "subscription",
    }
}

