use apollo_parser::ast;
use apollo_parser::Parser as ApolloParser;
use clap::Parser;
use serde::Serialize;

use std::fs;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

use crate::options::ToolsMergeOpt;
use crate::{RoverOutput, RoverResult};

#[derive(Clone, Debug, Parser, Serialize)]
pub struct Merge {
    #[clap(flatten)]
    options: ToolsMergeOpt,
}

impl Merge {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        // find files by extension
        let schemas = self.find_files_by_extensions(self.options.schemas.clone(), &["graphql", "gql"])?;
        // merge schemas into one
        let schema = self.merge_schemas_into_one(schemas)?;
        Ok(RoverOutput::ToolsSchemaMerge(schema))
    }

    fn find_files_by_extensions<P: AsRef<Path>>(&self, folder: P, extensions: &'_ [&str]) -> std::io::Result<Vec<PathBuf>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(folder.as_ref())? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let subfolder_result = self.find_files_by_extensions(&path, extensions);
                if let Ok(subfolder_paths) = subfolder_result {
                    result.extend(subfolder_paths);
                }
            } else if let Some(file_ext) = path.extension().and_then(OsStr::to_str) {
                if extensions.contains(&file_ext) {
                    result.push(path);
                }
            }
        }
    
        Ok(result)
    }

    fn merge_schemas_into_one(&self, schemas: Vec<PathBuf>) -> RoverResult<String> {
        let mut schema = apollo_encoder::Document::new();
        for schema_path in schemas {
            let schema_content = fs::read_to_string(schema_path)?;
            let parser = ApolloParser::new(&schema_content);
            let ast = parser.parse();
            let doc = ast.document();
            
            for def in doc.definitions() {
                match def {
                    ast::Definition::SchemaDefinition(schema_def) => {
                        schema.schema(schema_def.try_into()?);
                    }
                    ast::Definition::OperationDefinition(op_def) => {
                        schema.operation(op_def.try_into()?);
                    }
                    ast::Definition::FragmentDefinition(frag_def) => {
                        schema.fragment(frag_def.try_into()?);
                    }
                    ast::Definition::DirectiveDefinition(dir_def) => {
                        schema.directive(dir_def.try_into()?);
                    }
                    ast::Definition::ScalarTypeDefinition(scalar_type_def) => {
                        schema.scalar(scalar_type_def.try_into()?);
                    }
                    ast::Definition::ObjectTypeDefinition(object_type_def) => {
                        schema.object(object_type_def.try_into()?);
                    }
                    ast::Definition::InterfaceTypeDefinition(interface_type_def) => {
                        schema.interface(interface_type_def.try_into()?);
                    }
                    ast::Definition::UnionTypeDefinition(union_type_def) => {
                        schema.union(union_type_def.try_into()?);
                    }
                    ast::Definition::EnumTypeDefinition(enum_type_def) => {
                        schema.enum_(enum_type_def.try_into()?);
                    }
                    ast::Definition::InputObjectTypeDefinition(input_object_type_def) => {
                        schema.input_object(input_object_type_def.try_into()?);
                    }
                    ast::Definition::SchemaExtension(schema_extension_def) => {
                        schema.schema(schema_extension_def.try_into()?);
                    }
                    ast::Definition::ScalarTypeExtension(scalar_type_extension_def) => {
                        schema.scalar(scalar_type_extension_def.try_into()?);
                    }
                    ast::Definition::ObjectTypeExtension(object_type_extension_def) => {
                        schema.object(object_type_extension_def.try_into()?);
                    }
                    ast::Definition::InterfaceTypeExtension(interface_type_extension_def) => {
                        schema.interface(interface_type_extension_def.try_into()?);
                    }
                    ast::Definition::UnionTypeExtension(union_type_extension_def) => {
                        schema.union(union_type_extension_def.try_into()?);
                    }
                    ast::Definition::EnumTypeExtension(enum_type_extension_def) => {
                        schema.enum_(enum_type_extension_def.try_into()?);
                    }
                    ast::Definition::InputObjectTypeExtension(input_object_type_extension_def) => {
                        schema.input_object(input_object_type_extension_def.try_into()?);
                    }
                }
            }
        }
        Ok(schema.to_string())
    }
}
