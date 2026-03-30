use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use globwalk::GlobWalkerBuilder;

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

/// Discovers files with the given extensions under the provided roots.
pub fn discover_files(
    options: &DiscoveryOptions,
    default_root: &Utf8Path,
    extensions: &[&str],
) -> RoverResult<Vec<Utf8PathBuf>> {
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

    let mut patterns: Vec<String> = extensions
        .iter()
        .map(|ext| format!("**/*.{ext}"))
        .collect();
    for dir in &options.ignore_dirs {
        patterns.push(format!("!**/{dir}/**"));
    }

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

    let mut results = BTreeSet::new();
    for root in &roots {
        // If the root is a file itself, check it directly.
        if root.is_file() {
            if extensions.iter().any(|ext| root.extension() == Some(ext))
                && !excludes.iter().any(|ex| root.starts_with(ex))
            {
                results.insert(root.clone());
            }
            continue;
        }

        let walker = GlobWalkerBuilder::from_patterns(root, &patterns)
            .follow_links(false)
            .build()?;

        for entry in walker.filter_map(|e| e.ok()) {
            let Ok(path) = Utf8PathBuf::from_path_buf(entry.into_path()) else {
                continue;
            };
            if !excludes.iter().any(|ex| path.starts_with(ex)) {
                results.insert(path);
            }
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

        let files = discover_files(&options, &root, &["graphql"]).unwrap();
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
        let files = discover_files(&options, &root, &["graphql"]).unwrap();
        assert!(files.is_empty());
    }
}
