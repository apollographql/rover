mod field_detail;
mod schema_overview;
mod sdl;
mod type_detail;

pub use sdl::filtered_sdl;

use field_detail::FieldDetailDisplay;
use rover_schema::{FieldDetail, SchemaOverview, TypeDetail};
use serde::Serialize;
use type_detail::TypeDetailDisplay;

use crate::command::CliOutput;
use schema_overview::SchemaOverviewDisplay;

#[derive(Debug, Serialize)]
pub enum DescribeOutput {
    Sdl(String),
    Overview(SchemaOverview),
    Type(TypeDetail),
    Field(FieldDetail),
}

impl CliOutput for DescribeOutput {
    fn text(&self) -> String {
        match self {
            Self::Sdl(sdl) => sdl.clone(),
            Self::Overview(overview) => SchemaOverviewDisplay::from(overview).display(),
            Self::Type(type_detail) => format_type_detail(type_detail),
            Self::Field(field_detail) => format_field_detail(field_detail),
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

fn format_type_detail(type_detail: &TypeDetail) -> String {
    TypeDetailDisplay::from(type_detail).display()
}

fn format_field_detail(field_detail: &FieldDetail) -> String {
    FieldDetailDisplay::from(field_detail).display()
}
