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
        let dir = changeset_dir();
        match &self.command {
            Action::Add(add) => add.run(&dir),
            Action::Check(check) => check.run(&dir),
            Action::Preview => {
                let changesets = load_ordered(&dir)?;
                println!("{}", notes::release_notes(&values(&changesets)));
                Ok(())
            }
            Action::Release(release) => release.run(&dir),
        }
    }
}

fn changeset_dir() -> Utf8PathBuf {
    PKG_PROJECT_ROOT.join(".changeset")
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
    fn run(&self, dir: &Utf8Path) -> Result<()> {
        let paths = if self.all {
            changeset_paths(dir)?
        } else {
            let added = git(&[
                "diff",
                "--name-only",
                "--diff-filter=A",
                &format!("origin/{}...HEAD", self.base),
                "--",
                ".changeset/",
            ])?;
            let paths: Vec<Utf8PathBuf> = added
                .lines()
                .map(|line| PKG_PROJECT_ROOT.join(line))
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
    fn run(&self, dir: &Utf8Path) -> Result<()> {
        let version = self.version.trim_start_matches('v');
        semver::Version::parse(version).with_context(|| format!("`{version}` isn't a version"))?;
        let date = self
            .date
            .clone()
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

        let changesets = load_ordered(dir)?;
        if changesets.is_empty() {
            bail!("there are no changesets in {dir} to release");
        }
        let notes = notes::release_notes(&values(&changesets));

        let changelog_path = PKG_PROJECT_ROOT.join("CHANGELOG.md");
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

/// Every changeset, newest first: by when the commit that added it was made,
/// with ones not committed yet first, and by file name among equals.
fn load_ordered(dir: &Utf8Path) -> Result<Vec<(Utf8PathBuf, Changeset)>> {
    let mut keyed = Vec::new();
    for path in changeset_paths(dir)? {
        let changeset = read(&path).with_context(|| format!("{path} is invalid"))?;
        let added = git(&[
            "log",
            "--diff-filter=A",
            "--format=%ct",
            "-1",
            "--",
            path.as_str(),
        ])?
        .trim()
        .parse::<i64>()
        .unwrap_or(i64::MAX);
        keyed.push((added, path, changeset));
    }
    keyed.sort_by(|(a_time, a_path, _), (b_time, b_path, _)| {
        b_time.cmp(a_time).then_with(|| a_path.cmp(b_path))
    });
    Ok(keyed
        .into_iter()
        .map(|(_, path, changeset)| (path, changeset))
        .collect())
}

fn values(changesets: &[(Utf8PathBuf, Changeset)]) -> Vec<Changeset> {
    changesets
        .iter()
        .map(|(_, changeset)| changeset.clone())
        .collect()
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(PKG_PROJECT_ROOT.as_std_path())
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
