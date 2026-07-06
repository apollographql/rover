use camino::Utf8PathBuf;

use crate::command::output::CliOutput;

#[derive(Debug)]
pub enum GenerateOutput {
    File {
        path: Utf8PathBuf,
        operation_count: usize,
    },
    Stdout {
        manifest: serde_json::Value,
    },
}

impl CliOutput for GenerateOutput {
    fn text(&self) -> String {
        match self {
            GenerateOutput::File {
                path,
                operation_count,
            } => format!(
                "Manifest written to {} with {} operation{}.",
                path,
                operation_count,
                if *operation_count == 1 { "" } else { "s" },
            ),
            GenerateOutput::Stdout { manifest } => {
                serde_json::to_string_pretty(manifest).unwrap_or_default()
            }
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            GenerateOutput::File {
                path,
                operation_count,
            } => Ok(serde_json::json!({
                "path": path,
                "operation_count": operation_count,
            })),
            GenerateOutput::Stdout { manifest } => Ok(manifest.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn text_output_is_singular_for_one_operation() {
        let out = GenerateOutput::File {
            path: "manifest.json".into(),
            operation_count: 1,
        };
        assert_that!(out.text())
            .is_equal_to("Manifest written to manifest.json with 1 operation.".to_string());
    }

    #[test]
    fn text_output_is_plural_for_zero_operations() {
        let out = GenerateOutput::File {
            path: "manifest.json".into(),
            operation_count: 0,
        };
        assert_that!(out.text())
            .is_equal_to("Manifest written to manifest.json with 0 operations.".to_string());
    }

    #[test]
    fn text_output_is_plural_for_multiple_operations() {
        let out = GenerateOutput::File {
            path: "manifest.json".into(),
            operation_count: 42,
        };
        assert_that!(out.text())
            .is_equal_to("Manifest written to manifest.json with 42 operations.".to_string());
    }

    #[test]
    fn text_output_prints_manifest_json_when_no_path_given() {
        let manifest = serde_json::json!({"format": "apollo-persisted-query-manifest"});
        let out = GenerateOutput::Stdout {
            manifest: manifest.clone(),
        };
        assert_that!(out.text()).is_equal_to(serde_json::to_string_pretty(&manifest).unwrap());
    }

    #[test]
    fn json_output_returns_manifest_directly_when_no_path_given() {
        let manifest = serde_json::json!({"format": "apollo-persisted-query-manifest"});
        let out = GenerateOutput::Stdout {
            manifest: manifest.clone(),
        };
        assert_that!(out.json().unwrap()).is_equal_to(manifest);
    }
}
