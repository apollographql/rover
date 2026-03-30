mod field_detail;
mod schema_overview;
mod type_detail;

use field_detail::FieldDetailDisplay;
use itertools::Itertools;
use rover_schema::{DirectiveArgDetail, DirectiveDetail, FieldArgDetail, FieldDetail, SchemaOverview, TypeDetail};
use schema_overview::SchemaOverviewDisplay;
use serde::Serialize;
use type_detail::TypeDetailDisplay;

use crate::command::CliOutput;

#[derive(Debug, Serialize)]
pub enum DescribeOutput {
    Sdl(String),
    Overview(SchemaOverview),
    Type(TypeDetail),
    Field(FieldDetail),
    Directive(DirectiveDetail),
    DirectiveArg(DirectiveArgDetail),
    FieldArg(FieldArgDetail),
}

impl CliOutput for DescribeOutput {
    fn text(&self) -> String {
        match self {
            Self::Sdl(sdl) => sdl.clone(),
            Self::Overview(overview) => SchemaOverviewDisplay::from(overview).display(),
            Self::Type(type_detail) => format_type_detail(type_detail),
            Self::Field(field_detail) => format_field_detail(field_detail),
            Self::Directive(d) => format_directive_detail(d),
            Self::DirectiveArg(d) => format_directive_arg_detail(d),
            Self::FieldArg(d) => format_field_arg_detail(d),
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl From<rover_schema::describe::DescribeOutput> for DescribeOutput {
    fn from(output: rover_schema::describe::DescribeOutput) -> Self {
        match output {
            rover_schema::describe::DescribeOutput::Overview(o) => Self::Overview(o),
            rover_schema::describe::DescribeOutput::Type(t) => Self::Type(t),
            rover_schema::describe::DescribeOutput::Field(f) => Self::Field(f),
            rover_schema::describe::DescribeOutput::Directive(d) => Self::Directive(d),
            rover_schema::describe::DescribeOutput::DirectiveArg(d) => Self::DirectiveArg(d),
            rover_schema::describe::DescribeOutput::FieldArg(d) => Self::FieldArg(d),
        }
    }
}

fn format_type_detail(type_detail: &TypeDetail) -> String {
    TypeDetailDisplay::from(type_detail).display()
}

fn format_field_detail(field_detail: &FieldDetail) -> String {
    FieldDetailDisplay::from(field_detail).display()
}

fn format_directive_detail(d: &DirectiveDetail) -> String {
    let mut parts: Vec<String> = Vec::new();

    let repeatable = if d.repeatable { " repeatable" } else { "" };
    parts.push(format!("DIRECTIVE @{}{}", d.name, repeatable));

    if let Some(desc) = &d.description {
        parts.push(desc.clone());
    }

    if !d.locations.is_empty() {
        parts.push(format!("Locations: {}", d.locations.iter().join(", ")));
    }

    if !d.args.is_empty() {
        use comfy_table::{Table, presets};
        let mut table = Table::new();
        table.load_preset(presets::ASCII_FULL);
        table.set_header(["Arg", "Type", "Notes"]);
        for arg in &d.args {
            let notes = match (&arg.description, &arg.default_value) {
                (Some(desc), Some(default)) => format!("{} (default: {})", desc, default),
                (Some(desc), None) => desc.clone(),
                (None, Some(default)) => format!("default: {}", default),
                (None, None) => String::new(),
            };
            table.add_row([arg.name.as_str(), arg.arg_type.as_str(), &notes]);
        }
        parts.push(format!("Args\n{table}"));
    }

    parts.join("\n\n")
}

fn format_directive_arg_detail(d: &DirectiveArgDetail) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push(format!(
        "DIRECTIVE ARG @{}({}:): {}",
        d.directive_name, d.arg_name, d.arg_type
    ));

    if let Some(desc) = &d.description {
        parts.push(desc.clone());
    }

    if let Some(default) = &d.default_value {
        parts.push(format!("Default: {}", default));
    }

    parts.join("\n\n")
}

fn format_field_arg_detail(d: &FieldArgDetail) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push(format!(
        "FIELD ARG {}.{}({}:): {}",
        d.type_name, d.field_name, d.arg_name, d.arg_type
    ));

    if let Some(desc) = &d.description {
        parts.push(desc.clone());
    }

    if let Some(default) = &d.default_value {
        parts.push(format!("Default: {}", default));
    }

    parts.join("\n\n")
}
