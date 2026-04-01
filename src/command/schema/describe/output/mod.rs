mod directive_arg_detail;
mod directive_detail;
mod field_arg_detail;
mod field_detail;
mod schema_overview;
mod type_detail;

use directive_arg_detail::DirectiveArgDetailDisplay;
use directive_detail::DirectiveDetailDisplay;
use field_arg_detail::FieldArgDetailDisplay;
use field_detail::FieldDetailDisplay;
use rover_schema::{
    DirectiveArgDetail, DirectiveDetail, FieldArgDetail, FieldDetail, SchemaOverview, TypeDetail,
};
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
            Self::Type(type_detail) => TypeDetailDisplay::from(type_detail).display(),
            Self::Field(field_detail) => FieldDetailDisplay::from(field_detail).display(),
            Self::Directive(d) => DirectiveDetailDisplay::from(d).display(),
            Self::DirectiveArg(d) => DirectiveArgDetailDisplay::from(d).display(),
            Self::FieldArg(d) => FieldArgDetailDisplay::from(d).display(),
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
