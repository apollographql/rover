use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use rover_std::Fs;

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
    let mut results = BTreeSet::new();
    let mut queue: Vec<Utf8PathBuf> = if options.includes.is_empty() {
        vec![default_root.to_path_buf()]
    } else {
        options.includes.clone()
    };

    let excludes: BTreeSet<Utf8PathBuf> = options
        .excludes
        .iter()
        .map(|path| absolutize(default_root, path))
        .collect::<RoverResult<_>>()?;

    while let Some(path) = queue.pop() {
        let abs_path = absolutize(default_root, &path)?;
        if is_excluded(&abs_path, &excludes) {
            continue;
        }

        let metadata = Fs::metadata(&abs_path)?;
        if metadata.is_dir() {
            let dir_entries = match Fs::get_dir_entries(&abs_path) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in dir_entries.flatten() {
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                let entry_path = entry.path().to_path_buf();
                if is_excluded(&entry_path, &excludes) {
                    continue;
                }
                if file_type.is_dir() {
                    if should_ignore_dir(&entry_path, &options.ignore_dirs) {
                        continue;
                    }
                    queue.push(entry_path);
                } else if file_type.is_file() {
                    if predicate(entry_path.as_path()) {
                        results.insert(entry_path);
                    }
                }
            }
        } else if metadata.is_file() && predicate(abs_path.as_path()) {
            results.insert(abs_path);
        }
    }

    Ok(results.into_iter().collect())
}

fn absolutize(root: &Utf8Path, path: &Utf8Path) -> RoverResult<Utf8PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(root.join(path))
    }
}

fn is_excluded(path: &Utf8Path, excludes: &BTreeSet<Utf8PathBuf>) -> bool {
    excludes.iter().any(|exclude| path.starts_with(exclude))
}

fn should_ignore_dir(path: &Utf8Path, ignore_dirs: &[String]) -> bool {
    if let Some(file_name) = path.file_name() {
        ignore_dirs.iter().any(|ignored| ignored == file_name)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn discovers_files_with_include_and_exclude() {
        let temp = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp.path().join("root")).unwrap();
        Fs::create_dir_all(&root).unwrap();

        let file_a = root.join("a.graphql");
        let file_b = root.join("nested").join("b.graphql");
        let file_c = root.join("nested").join("ignore.graphql");
        Fs::create_dir_all(file_b.parent().unwrap()).unwrap();
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
        Fs::create_dir_all(ignored).unwrap();
        std::fs::write(&nested, "query Ignore { x }").unwrap();

        let options = DiscoveryOptions::default();
        let files = discover_files(&options, &root, |p| p.extension() == Some("graphql")).unwrap();
        assert!(files.is_empty());
    }
}
