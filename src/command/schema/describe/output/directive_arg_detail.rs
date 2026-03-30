use itertools::Itertools;
use rover_schema::DirectiveArgDetail;

pub struct DirectiveArgDetailDisplay<'a> {
    detail: &'a DirectiveArgDetail,
}

impl<'a> DirectiveArgDetailDisplay<'a> {
    pub fn display(&self) -> String {
        [Some(self.header()), self.description(), self.default_value()]
            .into_iter()
            .flatten()
            .join("\n\n")
    }

    fn header(&self) -> String {
        let d = self.detail;
        format!(
            "DIRECTIVE ARG @{}({}:): {}",
            d.directive_name, d.arg_name, d.arg_type
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

impl<'a> From<&'a DirectiveArgDetail> for DirectiveArgDetailDisplay<'a> {
    fn from(detail: &'a DirectiveArgDetail) -> Self {
        DirectiveArgDetailDisplay { detail }
    }
}
