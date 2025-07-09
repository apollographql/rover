use crate::command::init::options::SchemaName;
use crate::command::init::template_operations::PrintMode::{Confirmation, Normal};
use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
use crate::composition::supergraph::config::SupergraphConfigYaml;
use crate::{RoverError, RoverResult};
use anyhow::format_err;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use rover_std::successln;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct TemplateOperations;

#[derive(Debug)]
struct FileNode {
    children: BTreeMap<String, FileNode>,
    is_file: bool,
}

const DEFAULT_PRINT_LEVEL: u8 = 5;

/// Recursively prints the file tree structure up to a given depth.
fn print_node(
    node: &FileNode,
    depth: Option<u8>,
    current_level: u8,
    parent_has_sibling: &[bool],
    print_mode: PrintMode,
) {
    let max_depth = depth.unwrap_or(DEFAULT_PRINT_LEVEL);
    if current_level >= max_depth {
        return;
    }

    let mut entries: Vec<_> = node.children.iter().collect();
    // Sort files so that directories are first, then sort alphabetically
    entries.sort_by_key(|(_, child)| (!child.is_file, child.is_file));

    for (i, (name, child)) in entries.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == entries.len() - 1;
        let prefix = build_prefix(parent_has_sibling, is_first, is_last, current_level);
        let display_name = if !child.is_file {
            format!("{name}/")
        } else {
            name.to_string()
        };
        match print_mode {
            Normal => println!("{prefix}{display_name}"),
            Confirmation => successln!("{}{}", prefix, &display_name),
        }
        if !child.is_file {
            let mut new_parent = parent_has_sibling.to_vec();
            new_parent.push(!is_last);
            print_node(child, depth, current_level + 1, &new_parent, print_mode);
        }
    }
}

fn build_prefix(
    parent_has_sibling: &[bool],
    _is_first: bool,
    _is_last: bool,
    _current_level: u8,
) -> String {
    let mut prefix = String::new();
    for &has_sibling in parent_has_sibling {
        if has_sibling {
            prefix.push(' ');
            // prefix.push('│'); TODO: Add back in once we have accessibility mode
        } else {
            prefix.push(' ');
        }
        prefix.push(' ');
    }
    // TODO: Add back in once we have accessibility mode
    // if current_level == 0 {
    //     if is_first {
    //         prefix.push_str("┌ ");
    //     } else if is_last {
    //         prefix.push_str("└ ");
    //     } else {
    //         prefix.push_str("├ ");
    //     }
    // } else {
    //     prefix.push_str(if is_last { "└ " } else { "├ " });
    // }
    prefix
}

#[derive(Clone, Copy)]
pub enum PrintMode {
    Normal,
    Confirmation,
}

pub fn print_grouped_files(
    artifacts: Vec<Utf8PathBuf>,
    depth: Option<u8>,
    confirmation: PrintMode,
) {
    use std::collections::BTreeMap;

    let mut root = FileNode {
        children: BTreeMap::new(),
        is_file: false,
    };

    for artifact in artifacts {
        let components = artifact
            .components()
            .map(|c| c.as_str().to_string())
            .collect::<Vec<_>>();
        if components.is_empty() {
            continue;
        }
        let mut node = &mut root;
        for (i, comp) in components.iter().enumerate() {
            node = node
                .children
                .entry(comp.clone())
                .or_insert_with(|| FileNode {
                    children: BTreeMap::new(),
                    is_file: i == components.len() - 1,
                });
        }
    }

    print_node(&root, depth, 0, &[], confirmation);
}

impl TemplateOperations {
    pub fn prompt_creation(
        artifacts: Vec<Utf8PathBuf>,
        print_depth: Option<u8>,
    ) -> io::Result<bool> {
        println!();
        infoln!("You’re about to add the following files to your local directory:");
        println!();
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        print_grouped_files(artifacts_sorted, print_depth, Normal);

        println!();
        prompt_confirm_default_yes("? Proceed with creation?")
    }
}

pub struct SupergraphBuilder {
    directory: Utf8PathBuf,
    routing_url: String,
    max_depth: usize,
    schema_name: Option<SchemaName>,
    graph_ref: GraphRef,
}

impl SupergraphBuilder {
    pub fn new(
        directory: Utf8PathBuf,
        max_depth: usize,
        routing_url: String,
        schema_name: Option<SchemaName>,
        graph_ref: GraphRef,
    ) -> Self {
        Self {
            directory,
            routing_url,
            max_depth,
            schema_name,
            graph_ref,
        }
    }

    fn strip_base_prefix(&self, path: &Path, base_prefix: &Path) -> PathBuf {
        let canonical_base = base_prefix.canonicalize().unwrap();
        let canonical_path = path.canonicalize().unwrap();
        canonical_path
            .strip_prefix(canonical_base.clone())
            .unwrap()
            .to_owned()
    }

    /*
       In this fn we collect all graphql schemas found in the directory,
       also try to disambiguate names in case that they end up being duplicate
       by counting all resolved names to make sure there are no duplicates,
       depending on the structure of the graph, there is a chance that if we only use
       the parent for naming, there might be duplicates. for example
       /root
         /products
           /model
             /schema.graphql
         /services
           /model
             /schema.graphql
    */
    pub fn generate_subgraphs(&self) -> RoverResult<BTreeMap<String, SubgraphConfig>> {
        let mut subgraphs = BTreeMap::new();

        // Collect all graphql schemas
        let graphql_files = self.find_graphql_files(&self.directory, self.max_depth)?;

        if graphql_files.is_empty() {
            return Err(RoverError::from(format_err!(
                "No graphql files found in the directory"
            )));
        }

        for file_path in graphql_files {
            let mut name = self.determine_subgraph_name(&file_path, self.schema_name.clone())?;

            // Check if the name already exists amd disambiguate if so
            if subgraphs.contains_key(&name) {
                name = self.disambiguate_name(&file_path, &name)?;
            }

            let file = file_path.to_string_lossy().to_string();
            let subgraph = LazilyResolvedSubgraph::builder()
                .name(name.clone())
                .routing_url(self.routing_url.as_str())
                .schema(SchemaSource::File {
                    file: file.parse()?,
                })
                .build();

            subgraphs.insert(name, SubgraphConfig::from(subgraph));
        }

        Ok(subgraphs)
    }

    fn find_graphql_files(&self, dir: &Utf8PathBuf, max_depth: usize) -> RoverResult<Vec<PathBuf>> {
        let mut graphql_files = Vec::new();
        self.visit_dirs(dir.as_std_path(), 0, max_depth, &mut graphql_files)?;
        Ok(graphql_files)
    }

    fn visit_dirs(
        &self,
        dir: &Path,
        current_depth: usize,
        max_depth: usize,
        result: &mut Vec<PathBuf>,
    ) -> RoverResult<()> {
        if current_depth > max_depth {
            return Ok(());
        }

        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    self.visit_dirs(&path, current_depth + 1, max_depth, result)?;
                } else if self.is_graphql_file(&path) {
                    let path = self.strip_base_prefix(path.as_path(), self.directory.as_std_path());
                    result.push(path);
                }
            }
        }

        Ok(())
    }

    fn is_graphql_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            ext == "graphql" || ext == "graphqls"
        } else {
            false
        }
    }

    // If the file is named "schema", use parent directory name
    fn determine_subgraph_name(
        &self,
        file_path: &Path,
        schema_name: Option<SchemaName>,
    ) -> RoverResult<String> {
        let file_stem = file_path.file_stem().unwrap().to_string_lossy();
        if file_stem == "schema" {
            let parent = file_path.parent().unwrap();
            let parent_name = parent.file_name();
            if parent_name.is_none() && schema_name.is_some() {
                let schema_name = schema_name.unwrap();
                if !schema_name.to_string().is_empty() {
                    return Ok(schema_name.to_string());
                }
            }
            Ok(parent_name.unwrap().to_string_lossy().to_string())
        } else {
            Ok(file_stem.to_string())
        }
    }

    // Use parent's parent directory name for disambiguation
    fn disambiguate_name(&self, file_path: &Path, base_name: &str) -> RoverResult<String> {
        let parent_parent = file_path.parent().and_then(|p| p.parent()).unwrap();
        let parent_parent_name = parent_parent.file_name().unwrap().to_string_lossy();
        Ok(format!("{parent_parent_name}_{base_name}"))
    }

    pub fn build_supergraph(&self) -> RoverResult<SupergraphConfigYaml> {
        let subgraphs = self.generate_subgraphs()?;
        Ok(SupergraphConfigYaml {
            graph_ref: Some(self.graph_ref.clone()),
            subgraphs,
            federation_version: None,
        })
    }

    pub fn build_and_write(&self) -> RoverResult<()> {
        let supergraph = self.build_supergraph()?;
        let output_path = self.directory.join("rover.yaml");
        let mut file = File::create(output_path)?;
        serde_yaml::to_writer(&mut file, &supergraph)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    // Helper function to create a GraphQL file in a temp directory
    fn create_graphql_file(base_dir: &Path, rel_path: &str, content: &str) -> io::Result<()> {
        let file_path = base_dir.join(rel_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(file_path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    #[test]
    fn test_root_level_files() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        // Create a single root level GraphQL file
        create_graphql_file(
            temp_dir.path(),
            "services.graphql",
            "type Service { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(
            path,
            5,
            "http://ignore".to_string(),
            None,
            GraphRef {
                name: "name".to_string(),
                variant: "variant".to_string(),
            },
        );

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfigYaml = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_deep_nested_file() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/schema.graphql",
            "type Product { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(
            path,
            5,
            "http://ignore".to_string(),
            None,
            GraphRef {
                name: "name".to_string(),
                variant: "variant".to_string(),
            },
        );

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfigYaml = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);
    #[test]
    fn test_multiple_deep_nested_file() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/schema.graphql",
            "type Product { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "services/schema.graphql",
            "type Service { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(
            path,
            5,
            "http://ignore".to_string(),
            None,
            GraphRef {
                name: "name".to_string(),
                variant: "variant".to_string(),
            },
        );

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfigYaml = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_disambiguation() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/model/schema.graphql",
            "type Product { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "services/model/schema.graphql",
            "type Service { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "billing/billing.graphql",
            "type Billing { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(
            path,
            5,
            "http://ignore".to_string(),
            None,
            GraphRef {
                name: "name".to_string(),
                variant: "variant".to_string(),
            },
        );

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfigYaml = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }
}
