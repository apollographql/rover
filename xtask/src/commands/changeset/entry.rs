//! One changeset file: YAML frontmatter describing where an entry goes, and a
//! Markdown body whose first line is the entry's summary.

use std::{fmt, sync::LazyLock};

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// The `CHANGELOG.md` section an entry belongs in, unless it's breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Category {
    /// 🚀 Features
    Feat,
    /// 🐛 Fixes
    Fix,
    /// 🛠 Maintenance
    Maint,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Maint => "maint",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Frontmatter {
    pub(crate) category: Category,
    /// Breaking entries go under ❗ BREAKING ❗, whatever their category.
    pub(crate) breaking: bool,
    /// GitHub handles, with or without the leading `@`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pr: Option<u32>,
    /// Issues the change fixes: `ROVER-123` or `"#123"` (quoted, or YAML
    /// reads it as a comment).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) fixes: Vec<String>,
    /// Issues the change is part of, in the same forms as `fixes`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) part_of: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Changeset {
    pub(crate) frontmatter: Frontmatter,
    /// The entry's one-line summary, the first line of the body.
    pub(crate) summary: String,
    /// The rest of the body, if any, without surrounding blank lines.
    pub(crate) detail: Option<String>,
}

static ISSUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#\d+|[A-Z][A-Z0-9]*-\d+)$").expect("valid regex"));
static AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$").expect("valid regex")
});

impl Changeset {
    /// Parses a changeset file's contents.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let text = text.replace("\r\n", "\n");
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| anyhow!("doesn't start with a `---` frontmatter block"))?;
        let (yaml, body) = rest
            .split_once("\n---\n")
            .or_else(|| rest.strip_suffix("\n---").map(|yaml| (yaml, "")))
            .ok_or_else(|| anyhow!("frontmatter has no closing `---`"))?;
        let mut frontmatter: Frontmatter =
            serde_yaml::from_str(yaml).context("invalid frontmatter")?;

        frontmatter.authors = frontmatter
            .authors
            .iter()
            .map(|author| author.trim().trim_start_matches('@').to_string())
            .collect();
        if let Some(author) = frontmatter.authors.iter().find(|a| !AUTHOR.is_match(a)) {
            bail!("`{author}` isn't a GitHub handle");
        }
        for issue in frontmatter.fixes.iter().chain(&frontmatter.part_of) {
            if !ISSUE.is_match(issue) {
                bail!("`{issue}` isn't an issue reference: use `ROVER-123` or `\"#123\"`");
            }
        }

        let body = body.trim_matches('\n');
        let (summary, detail) = match body.split_once('\n') {
            Some((summary, detail)) => (summary, detail.trim_matches('\n')),
            None => (body, ""),
        };
        let summary = summary.trim();
        if summary.is_empty() {
            bail!("has no summary: the first line after the frontmatter describes the change");
        }
        if summary.contains("**") {
            bail!("the summary can't contain `**`: the changelog renders it in bold");
        }
        let detail = detail
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Self {
            frontmatter,
            summary: summary.to_string(),
            detail: (!detail.trim().is_empty()).then_some(detail),
        })
    }

    /// The file contents for this changeset.
    pub(crate) fn to_file(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&self.frontmatter)?;
        let mut file = format!("---\n{yaml}---\n\n{}\n", self.summary);
        if let Some(detail) = &self.detail {
            file.push('\n');
            file.push_str(detail);
            file.push('\n');
        }
        Ok(file)
    }

    /// The entry as `CHANGELOG.md` lists it: a bold summary carrying the
    /// authors, PR, and issues, then the detail indented under it.
    pub(crate) fn render(&self) -> String {
        let Frontmatter {
            authors,
            pr,
            fixes,
            part_of,
            ..
        } = &self.frontmatter;
        let mut credits = Vec::new();
        if !authors.is_empty() {
            credits.push(
                authors
                    .iter()
                    .map(|author| format!("@{author}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(pr) = pr {
            credits.push(format!("PR #{pr}"));
        }
        if !fixes.is_empty() {
            credits.push(format!("fixes {}", fixes.join(", ")));
        }
        if !part_of.is_empty() {
            credits.push(format!("part of {}", part_of.join(", ")));
        }
        let credits = if credits.is_empty() {
            String::new()
        } else {
            format!(" - {}", credits.join(" "))
        };

        let mut entry = format!("- **{}{credits}**", self.summary);
        if let Some(detail) = &self.detail {
            entry.push_str("\n\n");
            let indented: Vec<String> = detail
                .lines()
                .map(|line| {
                    if line.is_empty() {
                        String::new()
                    } else {
                        format!("  {line}")
                    }
                })
                .collect();
            entry.push_str(&indented.join("\n"));
        }
        entry
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn parses_and_renders_an_entry_with_every_credit() {
        let changeset = Changeset::parse(indoc! {r##"
            ---
            category: fix
            breaking: false
            authors: ["@dotdat", SharkBaitDLS]
            pr: 3980
            fixes: [ROVER-489, "#1455"]
            part_of: ["#3900"]
            ---

            `rover plugin install` warns on a legacy version spelling

            It still works, and now says what replaces it:

            ```
            warning: ...
            ```
        "##})
        .unwrap();

        assert_that!(changeset.render()).is_equal_to(
            indoc! {"
                - **`rover plugin install` warns on a legacy version spelling - @dotdat, @SharkBaitDLS PR #3980 fixes ROVER-489, #1455 part of #3900**

                  It still works, and now says what replaces it:

                  ```
                  warning: ...
                  ```"}
            .to_string(),
        );
    }

    #[rstest]
    #[case::author_only("authors: [dotdat]", "- **Summary - @dotdat**")]
    #[case::issue_only("fixes: [\"#1816\"]", "- **Summary - fixes #1816**")]
    #[case::no_credits("", "- **Summary**")]
    fn a_summary_only_entry_is_one_line(#[case] extra: &str, #[case] expected: &str) {
        let text = format!("---\ncategory: maint\nbreaking: false\n{extra}\n---\n\nSummary\n");

        let changeset = Changeset::parse(&text).unwrap();

        assert_that!(changeset.render()).is_equal_to(expected.to_string());
    }

    #[test]
    fn to_file_round_trips() {
        let changeset = Changeset {
            frontmatter: Frontmatter {
                category: Category::Feat,
                breaking: true,
                authors: vec!["dotdat".to_string()],
                pr: None,
                fixes: vec!["#12".to_string()],
                part_of: Vec::new(),
            },
            summary: "Summary".to_string(),
            detail: Some("Line one\n\nLine two".to_string()),
        };

        let file = changeset.to_file().unwrap();

        assert_that!(Changeset::parse(&file).unwrap()).is_equal_to(changeset);
    }

    #[rstest]
    #[case::no_frontmatter("Summary\n", "doesn't start with a `---` frontmatter block")]
    #[case::unclosed("---\ncategory: fix\n", "frontmatter has no closing `---`")]
    #[case::unknown_category(
        "---\ncategory: docs\nbreaking: false\n---\n\nS\n",
        "invalid frontmatter"
    )]
    #[case::missing_breaking("---\ncategory: fix\n---\n\nS\n", "invalid frontmatter")]
    #[case::unknown_field(
        "---\ncategory: fix\nbreaking: false\nauthor: x\n---\n\nS\n",
        "invalid frontmatter"
    )]
    #[case::no_summary("---\ncategory: fix\nbreaking: false\n---\n\n\n", "has no summary")]
    #[case::bold_summary(
        "---\ncategory: fix\nbreaking: false\n---\n\nA **bold** summary\n",
        "the summary can't contain `**`"
    )]
    #[case::bad_issue(
        "---\ncategory: fix\nbreaking: false\nfixes: [rover-1]\n---\n\nS\n",
        "`rover-1` isn't an issue reference: use `ROVER-123` or `\"#123\"`"
    )]
    #[case::bad_author(
        "---\ncategory: fix\nbreaking: false\nauthors: [\"two words\"]\n---\n\nS\n",
        "`two words` isn't a GitHub handle"
    )]
    fn an_invalid_changeset_says_why(#[case] text: &str, #[case] expected: &str) {
        let error = Changeset::parse(text).unwrap_err();

        assert_that!(error.to_string()).starts_with(expected);
    }
}
