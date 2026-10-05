//! Assembling changesets into release notes and a `CHANGELOG.md` section.

use anyhow::{anyhow, Result};

use super::entry::{Category, Changeset};

const BREAKING: &str = "## ❗ BREAKING ❗";
const FEATURES: &str = "## 🚀 Features";
const FIXES: &str = "## 🐛 Fixes";
const MAINTENANCE: &str = "## 🛠 Maintenance";

/// The release notes for `changesets`, in the order given: the breaking-change
/// count, then each non-empty section, breaking entries first.
pub(crate) fn release_notes(changesets: &[Changeset]) -> String {
    let in_section = |heading: &str| -> Vec<&Changeset> {
        changesets
            .iter()
            .filter(|changeset| {
                let frontmatter = &changeset.frontmatter;
                match heading {
                    BREAKING => frontmatter.breaking,
                    FEATURES => !frontmatter.breaking && frontmatter.category == Category::Feat,
                    FIXES => !frontmatter.breaking && frontmatter.category == Category::Fix,
                    _ => !frontmatter.breaking && frontmatter.category == Category::Maint,
                }
            })
            .collect()
    };

    let mut blocks = Vec::new();
    let breaking = in_section(BREAKING).len();
    if breaking > 0 {
        let noun = if breaking == 1 { "change" } else { "changes" };
        blocks.push(format!(
            "> Important: {breaking} potentially breaking {noun} below, indicated by **❗ BREAKING ❗**"
        ));
    }
    for heading in [BREAKING, FEATURES, FIXES, MAINTENANCE] {
        let entries = in_section(heading);
        if !entries.is_empty() {
            blocks.push(format!("{heading}\n\n{}", entries_block(&entries)));
        }
    }
    blocks.join("\n\n")
}

/// Entries as a list. One-line entries sit on consecutive lines, as
/// `CHANGELOG.md` has always listed them; an entry with detail is set apart
/// by blank lines.
fn entries_block(entries: &[&Changeset]) -> String {
    let mut block = String::new();
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            let previous = entries[index - 1];
            let spaced = previous.detail.is_some() || entry.detail.is_some();
            block.push_str(if spaced { "\n\n" } else { "\n" });
        }
        block.push_str(&entry.render());
    }
    block
}

/// `changelog` with a `# [version] - date` section holding `notes` inserted
/// before the newest released version, so it sits under `# [Unreleased]`.
pub(crate) fn insert_release(
    changelog: &str,
    version: &str,
    date: &str,
    notes: &str,
) -> Result<String> {
    let at = changelog
        .match_indices("\n# [")
        .map(|(index, _)| index + 1)
        .find(|index| {
            changelog[*index + 3..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
        })
        .ok_or_else(|| anyhow!("`CHANGELOG.md` has no released version to insert above"))?;
    let section = format!("# [{version}] - {date}\n\n{notes}\n\n");
    Ok(format!("{}{section}{}", &changelog[..at], &changelog[at..]))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use speculoos::prelude::*;

    use super::*;
    use crate::commands::changeset::entry::Frontmatter;

    fn entry(category: Category, breaking: bool, summary: &str, detail: Option<&str>) -> Changeset {
        Changeset {
            frontmatter: Frontmatter {
                category,
                breaking,
                authors: vec!["dotdat".to_string()],
                pr: None,
                fixes: Vec::new(),
                part_of: Vec::new(),
            },
            summary: summary.to_string(),
            detail: detail.map(str::to_string),
        }
    }

    #[test]
    fn notes_group_entries_and_count_breaking_changes() {
        let changesets = [
            entry(Category::Fix, false, "Fix one", Some("Why.")),
            entry(Category::Feat, true, "Break one", None),
            entry(Category::Maint, false, "Chore one", None),
            entry(Category::Maint, false, "Chore two", None),
            entry(Category::Feat, false, "Feature one", None),
            entry(Category::Maint, false, "Chore three", Some("Detail.")),
        ];

        assert_that!(release_notes(&changesets)).is_equal_to(
            indoc! {"
                > Important: 1 potentially breaking change below, indicated by **❗ BREAKING ❗**

                ## ❗ BREAKING ❗

                - **Break one - @dotdat**

                ## 🚀 Features

                - **Feature one - @dotdat**

                ## 🐛 Fixes

                - **Fix one - @dotdat**

                  Why.

                ## 🛠 Maintenance

                - **Chore one - @dotdat**
                - **Chore two - @dotdat**

                - **Chore three - @dotdat**

                  Detail."}
            .to_string(),
        );
    }

    #[test]
    fn notes_without_breaking_changes_omit_the_count() {
        let notes = release_notes(&[entry(Category::Fix, false, "Fix", None)]);

        assert_that!(notes).is_equal_to("## 🐛 Fixes\n\n- **Fix - @dotdat**".to_string());
    }

    #[test]
    fn a_release_is_inserted_above_the_newest_released_version() {
        let changelog = indoc! {"
            # Changelog

            <!-- # [x.x.x] (unreleased) - 2025-mm-dd -->

            # [Unreleased]

            Unreleased changes are in `.changeset/`.

            # [0.41.0] - 2026-07-09

            ## 🐛 Fixes
        "};

        let updated = insert_release(
            changelog,
            "1.0.0",
            "2026-11-01",
            "## 🚀 Features\n\n- **New**",
        )
        .unwrap();

        assert_that!(updated).is_equal_to(
            indoc! {"
                # Changelog

                <!-- # [x.x.x] (unreleased) - 2025-mm-dd -->

                # [Unreleased]

                Unreleased changes are in `.changeset/`.

                # [1.0.0] - 2026-11-01

                ## 🚀 Features

                - **New**

                # [0.41.0] - 2026-07-09

                ## 🐛 Fixes
            "}
            .to_string(),
        );
    }
}
