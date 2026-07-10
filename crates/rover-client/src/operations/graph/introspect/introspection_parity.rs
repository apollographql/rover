//! Structural comparison helpers for GraphQL introspection JSON.
//!
//! Compares `{ "__schema": ... }` documents order-independently: type names, field
//! names, nested type refs, enum values, interfaces, and directives.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

/// Compare introspection JSON structurally, ignoring array ordering.
///
/// Checks type names, kinds, fields, args, nested type references, enum values,
/// interfaces, possible types, and directives against a reference document.
pub fn assert_structural_parity(actual: &Value, reference: &Value) {
    let actual_schema = &actual["__schema"];
    let reference_schema = &reference["__schema"];

    assert_eq!(
        actual_schema["queryType"]["name"],
        reference_schema["queryType"]["name"]
    );
    assert_eq!(
        actual_schema["mutationType"],
        reference_schema["mutationType"]
    );
    assert_eq!(
        actual_schema["subscriptionType"],
        reference_schema["subscriptionType"]
    );

    let actual_types = index_types_by_name(actual_schema);
    let reference_types = index_types_by_name(reference_schema);

    assert_eq!(
        actual_types.keys().collect::<BTreeSet<_>>(),
        reference_types.keys().collect::<BTreeSet<_>>()
    );

    for (name, actual_type) in &actual_types {
        if name.starts_with("__") {
            continue;
        }
        let reference_type = &reference_types[name];
        assert_eq!(
            actual_type["kind"], reference_type["kind"],
            "kind for {name}"
        );

        compare_fields(name, actual_type, reference_type);
        compare_enum_values(name, actual_type, reference_type);
        compare_interfaces(name, actual_type, reference_type);
        compare_possible_types(name, actual_type, reference_type);
    }

    compare_directives(actual_schema, reference_schema);
}

fn index_types_by_name(schema: &Value) -> BTreeMap<String, &Value> {
    schema["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["name"].as_str().unwrap().to_string(), t))
        .collect()
}

fn compare_fields(type_name: &str, actual: &Value, reference: &Value) {
    let actual_fields = index_by_name(actual.get("fields"));
    let reference_fields = index_by_name(reference.get("fields"));

    assert_eq!(
        actual_fields.keys().collect::<BTreeSet<_>>(),
        reference_fields.keys().collect::<BTreeSet<_>>(),
        "fields on {type_name}"
    );

    for (field_name, actual_field) in &actual_fields {
        let reference_field = reference_fields[field_name];
        assert_eq!(
            normalize_type_ref(&actual_field["type"]),
            normalize_type_ref(&reference_field["type"]),
            "type ref for {type_name}.{field_name}"
        );
        compare_args(
            &format!("{type_name}.{field_name}"),
            actual_field,
            reference_field,
        );
    }

    let actual_input_fields = index_by_name(actual.get("inputFields"));
    let reference_input_fields = index_by_name(reference.get("inputFields"));
    assert_eq!(
        actual_input_fields.keys().collect::<BTreeSet<_>>(),
        reference_input_fields.keys().collect::<BTreeSet<_>>(),
        "inputFields on {type_name}"
    );
    for (field_name, actual_field) in &actual_input_fields {
        let reference_field = reference_input_fields[field_name];
        assert_eq!(
            normalize_type_ref(&actual_field["type"]),
            normalize_type_ref(&reference_field["type"]),
            "input field type for {type_name}.{field_name}"
        );
    }
}

fn compare_args(context: &str, actual: &Value, reference: &Value) {
    let actual_args = index_by_name(actual.get("args"));
    let reference_args = index_by_name(reference.get("args"));
    assert_eq!(
        actual_args.keys().collect::<BTreeSet<_>>(),
        reference_args.keys().collect::<BTreeSet<_>>(),
        "args on {context}"
    );
    for (arg_name, actual_arg) in &actual_args {
        let reference_arg = reference_args[arg_name];
        assert_eq!(
            normalize_type_ref(&actual_arg["type"]),
            normalize_type_ref(&reference_arg["type"]),
            "arg type for {context}({arg_name})"
        );
    }
}

fn compare_enum_values(type_name: &str, actual: &Value, reference: &Value) {
    let actual_values = index_by_name(actual.get("enumValues"));
    let reference_values = index_by_name(reference.get("enumValues"));
    assert_eq!(
        actual_values.keys().collect::<BTreeSet<_>>(),
        reference_values.keys().collect::<BTreeSet<_>>(),
        "enumValues on {type_name}"
    );
}

fn compare_interfaces(type_name: &str, actual: &Value, reference: &Value) {
    let actual_ifaces = names_set(actual.get("interfaces"));
    let reference_ifaces = names_set(reference.get("interfaces"));
    assert_eq!(actual_ifaces, reference_ifaces, "interfaces on {type_name}");
}

fn compare_possible_types(type_name: &str, actual: &Value, reference: &Value) {
    let actual_types = names_set(actual.get("possibleTypes"));
    let reference_types = names_set(reference.get("possibleTypes"));
    assert_eq!(
        actual_types, reference_types,
        "possibleTypes on {type_name}"
    );
}

fn compare_directives(actual_schema: &Value, reference_schema: &Value) {
    let actual_directives = index_by_name(Some(&actual_schema["directives"]));
    let reference_directives = index_by_name(Some(&reference_schema["directives"]));

    for (name, reference_dir) in &reference_directives {
        let Some(actual_dir) = actual_directives.get(name) else {
            panic!("missing directive @{name} in generated introspection JSON");
        };
        let actual_locations = string_array_set(&actual_dir["locations"]);
        let reference_locations = string_array_set(&reference_dir["locations"]);
        assert!(
            reference_locations.is_subset(&actual_locations),
            "locations for directive {name}: reference {reference_locations:?} not subset of actual {actual_locations:?}"
        );
        compare_args(&format!("@{name}"), actual_dir, reference_dir);
    }
}

fn index_by_name(maybe_array: Option<&Value>) -> BTreeMap<String, &Value> {
    maybe_array
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| (v["name"].as_str().unwrap().to_string(), v))
                .collect()
        })
        .unwrap_or_default()
}

fn names_set(maybe_array: Option<&Value>) -> BTreeSet<String> {
    maybe_array
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| v["name"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn string_array_set(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_type_ref(type_ref: &Value) -> Value {
    if type_ref.is_null() {
        return Value::Null;
    }
    let mut normalized = json!({
        "kind": type_ref["kind"],
        "name": type_ref["name"],
    });
    if let Some(of_type) = type_ref.get("ofType") {
        if !of_type.is_null() {
            normalized["ofType"] = normalize_type_ref(of_type);
        }
    }
    normalized
}
