use itertools::Itertools;
use rover_schema::FieldArgDetail;

pub struct FieldArgDetailDisplay<'a> {
    detail: &'a FieldArgDetail,
}

impl<'a> FieldArgDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [Some(self.header()), self.description(), self.default_value()]
            .into_iter()
            .flatten()
            .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        format!(
            "FIELD ARG {}.{}({}:): {}",
            d.type_name, d.field_name, d.arg_name, d.arg_type
        )
    }

    fn description(&self) -> Option<String> {
        self.detail.description.clone()
    }

    fn default_value(&self) -> Option<String> {
        self.detail
            .default_value
            .as_deref()
            .map(|v| format!("Default: {}", v))
    }
}

impl<'a> From<&'a FieldArgDetail> for FieldArgDetailDisplay<'a> {
    fn from(detail: &'a FieldArgDetail) -> Self {
        FieldArgDetailDisplay { detail }
    }
}
