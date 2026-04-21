use std::collections::BTreeSet;

use bon::Builder;
use camino::Utf8PathBuf;
use globwalk::GlobWalkerBuilder;

use crate::RoverStdError;

fn default_ignore_dirs() -> Vec<String> {
    vec![
        ".git".into(),
        "node_modules".into(),
        "target".into(),
        "build".into(),
    ]
}

/// Options controlling filesystem discovery.
#[derive(Debug, Clone, Builder)]
pub struct FileSearch {
    /// Root directory to scan.
    pub root: Utf8PathBuf,
    /// Glob patterns to include (relative to root). If empty, defaults to `**/*.{ext}` for each
    /// requested extension.
    #[builder(default)]
    pub includes: Vec<String>,
    /// Glob patterns to exclude (relative to root).
    #[builder(default)]
    pub excludes: Vec<String>,
    /// Extra directory names to ignore by default (e.g., build outputs).
    #[builder(default = default_ignore_dirs())]
    pub ignore_dirs: Vec<String>,
}

impl FileSearch {
    /// Discovers files with the given extensions under `self.root`.
    pub fn find(&self, extensions: &[&str]) -> Result<Vec<Utf8PathBuf>, RoverStdError> {
        let mut patterns: Vec<String> = if self.includes.is_empty() {
            extensions.iter().map(|ext| format!("**/*.{ext}")).collect()
        } else {
            self.includes.clone()
        };

        for dir in &self.ignore_dirs {
            patterns.push(format!("!**/{dir}/**"));
        }
        for pat in &self.excludes {
            patterns.push(format!("!{pat}"));
        }

        let walker = GlobWalkerBuilder::from_patterns(&self.root, &patterns)
            .follow_links(false)
            .build()
            .map_err(|e| anyhow::anyhow!(e))?;

        let results: BTreeSet<Utf8PathBuf> = walker
            .filter_map(|e| e.ok())
            .filter_map(|e| Utf8PathBuf::from_path_buf(e.into_path()).ok())
            .collect();

        Ok(results.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Verifies that include and exclude patterns combine to select only the expected files.
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

        let search = FileSearch::builder()
            .root(root.clone())
            .includes(vec!["nested/**".to_string()])
            .excludes(vec!["nested/ignore.graphql".to_string()])
            .build();

        let files = search.find(&["graphql"]).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file_b);
    }

    /// Verifies that node_modules and other default-ignored directories are not scanned.
    #[test]
    fn skips_ignored_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().join("root")).unwrap();
        let ignored = root.join("node_modules");
        let nested = ignored.join("c.graphql");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(&nested, "query Ignore { x }").unwrap();

        let files = FileSearch::builder()
            .root(root.clone())
            .build()
            .find(&["graphql"])
            .unwrap();
        assert!(files.is_empty());
    }

    /// Verifies that a default scan (no includes specified) finds all files matching the requested
    /// extension in nested subdirectories.
    #[test]
    fn default_scan_finds_all_matching_extensions() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
        let nested = root.join("sub");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("a.graphql"), "query A { x }").unwrap();
        fs::write(nested.join("b.graphql"), "query B { x }").unwrap();
        fs::write(root.join("c.txt"), "not graphql").unwrap();

        let files = FileSearch::builder()
            .root(root)
            .build()
            .find(&["graphql"])
            .unwrap();
        assert_eq!(files.len(), 2);
    }

    /// Verifies that multiple extensions can be requested and all matching files are returned.
    #[test]
    fn finds_multiple_extensions() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
        fs::write(root.join("a.graphql"), "query A { x }").unwrap();
        fs::write(root.join("b.gql"), "query B { x }").unwrap();
        fs::write(root.join("c.txt"), "not graphql").unwrap();

        let files = FileSearch::builder()
            .root(root)
            .build()
            .find(&["graphql", "gql"])
            .unwrap();
        assert_eq!(files.len(), 2);
    }

    /// Verifies that an empty result is returned when no files match the requested extension.
    #[test]
    fn returns_empty_when_no_files_match() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
        fs::write(root.join("readme.txt"), "not graphql").unwrap();

        let files = FileSearch::builder()
            .root(root)
            .build()
            .find(&["graphql"])
            .unwrap();
        assert!(files.is_empty());
    }
}
