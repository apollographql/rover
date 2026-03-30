use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;

use crate::RoverResult;

/// Options controlling filesystem discovery.
#[derive(Debug, Clone)]
pub struct DiscoveryOptions {
    /// Paths to start scanning from. If empty, the caller should provide a default root.
    pub includes: Vec<Utf8PathBuf>,
    /// Paths to exclude (directories or files).
    pub excludes: Vec<Utf8PathBuf>,
    /// Extra directory names to ignore by default (e.g., build outputs).
    pub ignore_dirs: Vec<String>,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            includes: Vec::new(),
            excludes: Vec::new(),
            ignore_dirs: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "build".into(),
            ],
        }
    }
}

/// Discovers files under the provided roots that match the given predicate.
pub fn discover_files<F>(
    options: &DiscoveryOptions,
    default_root: &Utf8Path,
    mut predicate: F,
) -> RoverResult<Vec<Utf8PathBuf>>
where
    F: FnMut(&Utf8Path) -> bool,
{
    let roots: Vec<Utf8PathBuf> = if options.includes.is_empty() {
        vec![default_root.to_path_buf()]
    } else {
        options
            .includes
            .iter()
            .map(|p| {
                if p.is_absolute() {
                    p.clone()
                } else {
                    default_root.join(p)
                }
            })
            .collect()
    };

    let excludes: Vec<Utf8PathBuf> = options
        .excludes
        .iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                default_root.join(p)
            }
        })
        .collect();

    let ignore_dirs = options.ignore_dirs.clone();

    let mut builder = WalkBuilder::new(&roots[0]);
    for root in &roots[1..] {
        builder.add(root);
    }
    builder
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false)
        .filter_entry(move |e| {
            if e.file_type().map_or(false, |ft| ft.is_dir()) {
                let name = e.file_name().to_string_lossy();
                !ignore_dirs.iter().any(|d| d.as_str() == name.as_ref())
            } else {
                true
            }
        });

    let mut results = std::collections::BTreeSet::new();
    for entry in builder.build().filter_map(|e| e.ok()) {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.into_path()) else {
            continue;
        };
        if excludes.iter().any(|ex| path.starts_with(ex)) {
            continue;
        }
        if path.is_file() && predicate(&path) {
            results.insert(path);
        }
    }

    Ok(results.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn discovers_files_with_include_and_exclude() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().join("root")).unwrap();
        fs::create_dir_all(&root).unwrap();

        let file_a = root.join("a.graphql");
        let file_b = root.join("nested").join("b.graphql");
        let file_c = root.join("nested").join("ignore.graphql");
        fs::create_dir_all(file_b.parent().unwrap()).unwrap();
        fs::write(&file_a, "query A { x }").unwrap();
        fs::write(&file_b, "query B { x }").unwrap();
        fs::write(&file_c, "query C { x }").unwrap();

        let options = DiscoveryOptions {
            includes: vec![root.join("nested")],
            excludes: vec![Utf8PathBuf::from("nested/ignore.graphql")],
            ..Default::default()
        };

        let files = discover_files(&options, &root, |p| p.extension() == Some("graphql")).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file_b);
    }

    #[test]
    fn skips_ignored_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().join("root")).unwrap();
        let ignored = root.join("node_modules");
        let nested = ignored.join("c.graphql");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(&nested, "query Ignore { x }").unwrap();

        let options = DiscoveryOptions::default();
        let files = discover_files(&options, &root, |p| p.extension() == Some("graphql")).unwrap();
        assert!(files.is_empty());
    }
}
