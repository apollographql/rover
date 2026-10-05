//! `cargo xtask changeset`: one file per change in `.changeset/`, assembled
//! into `CHANGELOG.md` and the GitHub release notes when a release is cut.
//! See `.changeset/README.md`.

mod entry;
mod notes;

use std::{fs, io::IsTerminal, process::Command};

use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};

use self::entry::{Category, Changeset, Frontmatter};
use crate::utils::PKG_PROJECT_ROOT;

#[derive(Debug, Parser)]
pub struct ChangesetCommand {
    #[clap(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Create a changeset describing a user-visible change
    Add(Add),
    /// Validate changesets: the ones this branch adds, or every one with `--all`
    Check(Check),
    /// Print the release notes the current changesets would produce
    Preview,
    /// Add a release's section to CHANGELOG.md from the changesets, write its
    /// release notes to .changeset/notes/, and delete the changesets used
    Release(Release),
}

impl ChangesetCommand {
    pub(crate) fn run(&self) -> Result<()> {
        let root: &Utf8Path = &PKG_PROJECT_ROOT;
        match &self.command {
            Action::Add(add) => add.run(&changeset_dir(root)),
            Action::Check(check) => check.run(root),
            Action::Preview => {
                let changesets = load_ordered(root)?;
                println!("{}", notes::release_notes(&values(&changesets)));
                Ok(())
            }
            Action::Release(release) => release.run(root),
        }
    }
}

/// `.changeset/` in the repository at `root`.
fn changeset_dir(root: &Utf8Path) -> Utf8PathBuf {
    root.join(".changeset")
}

#[derive(Debug, Parser)]
struct Add {
    /// File name, in kebab-case, without `.md`
    #[arg(long)]
    name: Option<String>,
    #[arg(long, value_enum)]
    category: Option<Category>,
    /// Whether the change breaks an existing invocation, output, or config
    #[arg(long)]
    breaking: Option<bool>,
    /// GitHub handle of an author; repeat for more than one
    #[arg(long = "author")]
    authors: Vec<String>,
    /// The pull request number, if it's known already
    #[arg(long)]
    pr: Option<u32>,
    /// An issue the change fixes, `ROVER-123` or `#123`; repeatable
    #[arg(long)]
    fixes: Vec<String>,
    /// An issue the change is part of; repeatable
    #[arg(long)]
    part_of: Vec<String>,
    /// One line describing the change, for the changelog's bold heading
    #[arg(long)]
    summary: Option<String>,
    /// More detail, shown under the heading
    #[arg(long)]
    body: Option<String>,
}

impl Add {
    fn run(&self, dir: &Utf8Path) -> Result<()> {
        let interactive = std::io::stdin().is_terminal();
        let ask = |what: &str| -> Result<String> {
            if !interactive {
                bail!("--{what} is required when not running in a terminal");
            }
            Ok(dialoguer::Input::<String>::new()
                .with_prompt(what)
                .interact_text()?)
        };

        let name = match &self.name {
            Some(name) => name.clone(),
            None => ask("name")?,
        };
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!("`{name}` isn't kebab-case");
        }
        let path = dir.join(format!("{name}.md"));
        if path.exists() {
            bail!("{path} already exists");
        }
        let category = match self.category {
            Some(category) => category,
            None if interactive => {
                let options = [Category::Feat, Category::Fix, Category::Maint];
                let index = dialoguer::Select::new()
                    .with_prompt("category")
                    .items(options.iter().map(ToString::to_string))
                    .default(1)
                    .interact()?;
                options[index]
            }
            None => bail!("--category is required when not running in a terminal"),
        };
        let breaking = match self.breaking {
            Some(breaking) => breaking,
            None if interactive => dialoguer::Confirm::new()
                .with_prompt("breaking")
                .default(false)
                .interact()?,
            None => bail!("--breaking is required when not running in a terminal"),
        };
        let authors = if self.authors.is_empty() && interactive {
            let handle = ask("author (GitHub handle)")?;
            vec![handle]
        } else {
            self.authors.clone()
        };
        let summary = match &self.summary {
            Some(summary) => summary.clone(),
            None => ask("summary")?,
        };

        let changeset = Changeset {
            frontmatter: Frontmatter {
                category,
                breaking,
                authors,
                pr: self.pr,
                fixes: self.fixes.clone(),
                part_of: self.part_of.clone(),
            },
            summary,
            detail: self.body.clone().filter(|body| !body.trim().is_empty()),
        };
        let contents = changeset.to_file()?;
        // Parse what is about to be written, so `add` can't create a file
        // `check` would reject.
        Changeset::parse(&contents).with_context(|| format!("{path} would be invalid"))?;
        fs::write(&path, contents)?;
        println!("Created {path}");
        Ok(())
    }
}

#[derive(Debug, Parser)]
struct Check {
    /// The branch this one merges into; the changesets it adds are checked
    #[arg(long, default_value = "main", conflicts_with = "all")]
    base: String,
    /// Check every changeset, not only the ones this branch adds
    #[arg(long)]
    all: bool,
}

impl Check {
    /// Checks the changesets in the repository at `root`.
    fn run(&self, root: &Utf8Path) -> Result<()> {
        let paths = if self.all {
            changeset_paths(&changeset_dir(root))?
        } else {
            let added = git(
                root,
                &[
                    "diff",
                    "--name-only",
                    "--diff-filter=A",
                    &format!("origin/{}...HEAD", self.base),
                    "--",
                    ".changeset/",
                ],
            )?;
            let paths: Vec<Utf8PathBuf> = added
                .lines()
                .map(|line| root.join(line))
                .filter(|path| is_changeset(path))
                .collect();
            if paths.is_empty() {
                bail!(
                    "This branch adds no changeset to .changeset/.\n\
                     Create one with `mise run add-changeset` (see .changeset/README.md).\n\
                     If the change isn't user-visible, add the `skip-changeset` label to the pull request."
                );
            }
            paths
        };

        let mut failures = 0;
        for path in &paths {
            match read(path) {
                Ok(_) => println!("ok    {path}"),
                Err(error) => {
                    failures += 1;
                    println!("error {path}: {error:#}");
                }
            }
        }
        if failures > 0 {
            bail!("{failures} invalid changeset(s)");
        }
        Ok(())
    }
}

#[derive(Debug, Parser)]
struct Release {
    /// The version being released, for example `1.0.0` or `v1.0.0`
    version: String,
    /// The release date, `YYYY-MM-DD`; today if omitted
    #[arg(long)]
    date: Option<String>,
}

impl Release {
    /// Releases the changesets in the repository at `root`.
    fn run(&self, root: &Utf8Path) -> Result<()> {
        let dir = changeset_dir(root);
        let version = self.version.trim_start_matches('v');
        semver::Version::parse(version).with_context(|| format!("`{version}` isn't a version"))?;
        let date = self
            .date
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

        let changesets = load_ordered(root)?;
        if changesets.is_empty() {
            bail!("there are no changesets in {dir} to release");
        }
        let notes = notes::release_notes(&values(&changesets));

        let changelog_path = root.join("CHANGELOG.md");
        let changelog = fs::read_to_string(&changelog_path)?;
        fs::write(
            &changelog_path,
            notes::insert_release(&changelog, version, &date, &notes)?,
        )?;
        let notes_dir = dir.join("notes");
        fs::create_dir_all(&notes_dir)?;
        let notes_path = notes_dir.join(format!("v{version}.md"));
        fs::write(&notes_path, format!("{notes}\n"))?;
        for (path, _) in &changesets {
            fs::remove_file(path)?;
        }
        println!(
            "Added {version} to CHANGELOG.md, wrote {notes_path}, and removed {} changeset(s).",
            changesets.len()
        );
        Ok(())
    }
}

fn is_changeset(path: &Utf8Path) -> bool {
    path.extension() == Some("md")
        && path.file_name() != Some("README.md")
        && path.parent().and_then(Utf8Path::file_name) == Some(".changeset")
}

fn changeset_paths(dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut paths = Vec::new();
    for entry in dir
        .read_dir_utf8()
        .with_context(|| format!("reading {dir}"))?
    {
        let path = entry?.into_path();
        if is_changeset(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read(path: &Utf8Path) -> Result<Changeset> {
    let text = fs::read_to_string(path)?;
    Changeset::parse(&text)
}

/// Every changeset in the repository at `root`, newest first (see
/// [`newest_first`]).
fn load_ordered(root: &Utf8Path) -> Result<Vec<(Utf8PathBuf, Changeset)>> {
    let mut keyed = Vec::new();
    for path in changeset_paths(&changeset_dir(root))? {
        let changeset = read(&path).with_context(|| format!("{path} is invalid"))?;
        let added = git(
            root,
            &[
                "log",
                "--diff-filter=A",
                "--format=%ct",
                "-1",
                "--",
                path.as_str(),
            ],
        )?
        .trim()
        .parse::<i64>()
        .ok();
        keyed.push((added, path, changeset));
    }
    Ok(newest_first(keyed))
}

/// Orders items by when the commit that added each was made, newest first.
/// Ones not committed yet (`None`) come first, and the file name breaks ties,
/// so changesets added in one commit keep their names' order.
fn newest_first<T>(mut keyed: Vec<(Option<i64>, Utf8PathBuf, T)>) -> Vec<(Utf8PathBuf, T)> {
    keyed.sort_by(|(a_time, a_path, _), (b_time, b_path, _)| {
        let a_time = a_time.unwrap_or(i64::MAX);
        let b_time = b_time.unwrap_or(i64::MAX);
        b_time.cmp(&a_time).then_with(|| a_path.cmp(b_path))
    });
    keyed
        .into_iter()
        .map(|(_, path, item)| (path, item))
        .collect()
}

fn values(changesets: &[(Utf8PathBuf, Changeset)]) -> Vec<Changeset> {
    changesets
        .iter()
        .map(|(_, changeset)| changeset.clone())
        .collect()
}

/// Runs git in the repository at `root`, returning its stdout.
fn git(root: &Utf8Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root.as_std_path())
        .output()
        .context("running git")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use camino::{Utf8Path, Utf8PathBuf};
    use indoc::indoc;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tempfile::TempDir;

    use super::*;

    /// A throwaway git repository with a `.changeset/` directory.
    struct Repo {
        _dir: TempDir,
        root: Utf8PathBuf,
    }

    impl Repo {
        fn new() -> Self {
            let dir = TempDir::new().unwrap();
            let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
            fs::create_dir_all(root.join(".changeset")).unwrap();
            let repo = Self { _dir: dir, root };
            repo.git(&["init", "-q", "-b", "main"], None);
            repo
        }

        fn git(&self, args: &[&str], date: Option<&str>) {
            let mut command = Command::new("git");
            command
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .current_dir(&self.root);
            if let Some(date) = date {
                command
                    .env("GIT_AUTHOR_DATE", date)
                    .env("GIT_COMMITTER_DATE", date);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn write(&self, path: &str, contents: &str) {
            fs::write(self.root.join(path), contents).unwrap();
        }

        /// Writes a valid `fix` changeset named `name` with `summary`.
        fn changeset(&self, name: &str, summary: &str) {
            self.write(
                &format!(".changeset/{name}.md"),
                &format!("---\ncategory: fix\nbreaking: false\n---\n\n{summary}\n"),
            );
        }

        /// Commits everything, dated `epoch` seconds.
        fn commit(&self, epoch: i64) {
            self.git(&["add", "-A"], None);
            self.git(
                &["commit", "-q", "--allow-empty", "-m", "commit"],
                Some(&format!("@{epoch} +0000")),
            );
        }

        /// Makes the current commit `origin/main`, the base `check` compares
        /// against.
        fn mark_base(&self) {
            self.git(&["update-ref", "refs/remotes/origin/main", "HEAD"], None);
        }
    }

    fn summaries(root: &Utf8Path) -> Vec<String> {
        load_ordered(root)
            .unwrap()
            .into_iter()
            .map(|(_, changeset)| changeset.summary)
            .collect()
    }

    #[test]
    fn newest_first_puts_uncommitted_then_newest_and_breaks_ties_by_name() {
        let keyed = vec![
            (Some(100), Utf8PathBuf::from("b-old"), ()),
            (Some(300), Utf8PathBuf::from("c-new"), ()),
            (None, Utf8PathBuf::from("z-pending"), ()),
            (Some(200), Utf8PathBuf::from("b-tied"), ()),
            (Some(200), Utf8PathBuf::from("a-tied"), ()),
            (None, Utf8PathBuf::from("a-pending"), ()),
        ];

        let order: Vec<String> = newest_first(keyed)
            .into_iter()
            .map(|(path, ())| path.to_string())
            .collect();

        assert_that!(order).is_equal_to(
            [
                "a-pending",
                "z-pending",
                "c-new",
                "a-tied",
                "b-tied",
                "b-old",
            ]
            .map(String::from)
            .to_vec(),
        );
    }

    #[rstest]
    #[case::a_changeset("repo/.changeset/fix-thing.md", true)]
    #[case::the_readme("repo/.changeset/README.md", false)]
    #[case::release_notes("repo/.changeset/notes/v1.0.0.md", false)]
    #[case::not_markdown("repo/.changeset/fix-thing.txt", false)]
    #[case::outside_the_directory("repo/docs/fix-thing.md", false)]
    fn only_markdown_files_directly_in_the_directory_are_changesets(
        #[case] path: &str,
        #[case] expected: bool,
    ) {
        assert_that!(is_changeset(Utf8Path::new(path))).is_equal_to(expected);
    }

    #[test]
    fn changesets_load_newest_first_by_the_commit_that_added_them() {
        let repo = Repo::new();
        repo.changeset("old", "Old");
        repo.commit(1_000);
        repo.changeset("new", "New");
        repo.commit(2_000);
        repo.changeset("pending", "Pending");

        assert_that!(summaries(&repo.root))
            .is_equal_to(["Pending", "New", "Old"].map(String::from).to_vec());
    }

    #[test]
    fn check_fails_when_the_branch_adds_no_changeset() {
        let repo = Repo::new();
        repo.commit(1_000);
        repo.mark_base();
        repo.write("code.rs", "fn main() {}\n");
        repo.commit(2_000);

        let error = Check {
            base: "main".to_string(),
            all: false,
        }
        .run(&repo.root)
        .unwrap_err();

        assert_that!(error.to_string()).is_equal_to(
            "This branch adds no changeset to .changeset/.\n\
             Create one with `mise run add-changeset` (see .changeset/README.md).\n\
             If the change isn't user-visible, add the `skip-changeset` label to the pull request."
                .to_string(),
        );
    }

    #[test]
    fn check_validates_only_the_changesets_the_branch_adds() {
        let repo = Repo::new();
        // Already on the base branch, and invalid: not this branch's problem.
        repo.write(".changeset/inherited.md", "not a changeset\n");
        repo.commit(1_000);
        repo.mark_base();
        repo.changeset("added", "Added");
        repo.commit(2_000);
        let check = Check {
            base: "main".to_string(),
            all: false,
        };

        assert_that!(check.run(&repo.root)).is_ok();

        repo.write(
            ".changeset/broken.md",
            "---\ncategory: fix\n---\n\nNo breaking field\n",
        );
        repo.commit(3_000);

        assert_that!(check.run(&repo.root).unwrap_err().to_string())
            .is_equal_to("1 invalid changeset(s)".to_string());
    }

    #[test]
    fn release_adds_the_section_writes_the_notes_and_removes_the_changesets() {
        let repo = Repo::new();
        repo.write(
            "CHANGELOG.md",
            indoc! {"
                # Changelog

                # [Unreleased]

                Unreleased changes are in `.changeset/`.

                # [0.41.0] - 2026-07-09

                ## 🐛 Fixes
            "},
        );
        repo.changeset("first", "First fix");
        repo.commit(1_000);
        repo.changeset("second", "Second fix");
        repo.commit(2_000);

        Release {
            version: "v1.2.3".to_string(),
            date: Some("2030-01-01".to_string()),
        }
        .run(&repo.root)
        .unwrap();

        let notes = "## 🐛 Fixes\n\n- **Second fix**\n- **First fix**";
        assert_that!(fs::read_to_string(repo.root.join("CHANGELOG.md")).unwrap()).is_equal_to(
            indoc! {"
                # Changelog

                # [Unreleased]

                Unreleased changes are in `.changeset/`.

                # [1.2.3] - 2030-01-01

                ## 🐛 Fixes

                - **Second fix**
                - **First fix**

                # [0.41.0] - 2026-07-09

                ## 🐛 Fixes
            "}
            .to_string(),
        );
        assert_that!(fs::read_to_string(repo.root.join(".changeset/notes/v1.2.3.md")).unwrap())
            .is_equal_to(format!("{notes}\n"));
        assert_that!(changeset_paths(&changeset_dir(&repo.root)).unwrap()).is_empty();
    }

    #[test]
    fn release_refuses_when_there_are_no_changesets() {
        let repo = Repo::new();
        repo.write("CHANGELOG.md", "# [0.41.0] - 2026-07-09\n");
        let release = Release {
            version: "1.2.3".to_string(),
            date: None,
        };

        let error = release.run(&repo.root).unwrap_err();

        assert_that!(error.to_string()).is_equal_to(format!(
            "there are no changesets in {} to release",
            changeset_dir(&repo.root)
        ));
    }
}
