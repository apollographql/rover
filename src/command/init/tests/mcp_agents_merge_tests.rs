#[cfg(test)]
mod mcp_agents_merge_tests {
    use crate::command::init::options::ProjectType;
    use crate::command::init::states::MCPComposedTemplate;
    use crate::command::init::template_fetcher::{Template, TemplateId};
    use camino::Utf8PathBuf;
    use std::collections::HashMap;

    fn create_test_template() -> Template {
        Template {
            id: TemplateId("mcp-typescript".to_string()),
            display_name: "TypeScript + MCP".to_string(),
            path: "typescript".to_string(),
            language: "TypeScript".to_string(),
            federation_version: "=2.10.0".to_string(),
            max_schema_depth: 5,
            routing_url: "http://localhost:4001".to_string(),
            commands: None,
            start_point_file: "README.md".to_string(),
            print_depth: None,
        }
    }

    #[test]
    fn test_agents_merged_into_readme_for_new_project() {
        let template = create_test_template();

        // Create test files
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# My Project\n\nThis is the main README content.".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("AGENTS.md"),
            b"\n## AI Agents\n\nThis is the agents documentation.".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("other.md"),
            b"Other file content".to_vec(),
        );

        let composed_template =
            MCPComposedTemplate::new_with_agents_merge(template, files, ProjectType::CreateNew);

        // AGENTS.md should remain in file list (no longer removed after merging)
        let file_list = composed_template.list_files();
        assert!(file_list.contains(&Utf8PathBuf::from("AGENTS.md")));
        assert!(file_list.contains(&Utf8PathBuf::from("README.md")));
        assert!(file_list.contains(&Utf8PathBuf::from("other.md")));

        // README.md should contain both contents
        let readme_content = composed_template
            .merged_files
            .get(&Utf8PathBuf::from("README.md"))
            .unwrap();
        let readme_str = String::from_utf8_lossy(readme_content);

        assert!(readme_str.contains("# My Project"));
        assert!(readme_str.contains("This is the main README content."));
        assert!(readme_str.contains("## AI Agents"));
        assert!(readme_str.contains("This is the agents documentation."));

        // Other files should remain unchanged
        assert_eq!(file_list.len(), 3); // README.md + other.md + AGENTS.md (all preserved)
    }

    #[test]
    fn test_agents_not_merged_for_existing_project() {
        let template = create_test_template();

        // Create test files
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# Existing Project README".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("AGENTS.md"),
            b"# Agents Documentation".to_vec(),
        );

        let composed_template = MCPComposedTemplate::new_with_agents_merge(
            template,
            files.clone(),
            ProjectType::AddSubgraph, // Existing project
        );

        // For existing projects, files should remain unchanged
        let file_list = composed_template.list_files();
        assert!(file_list.contains(&Utf8PathBuf::from("AGENTS.md")));
        assert!(file_list.contains(&Utf8PathBuf::from("README.md")));
        assert_eq!(file_list.len(), 2);

        // Content should be unchanged
        let readme_content = composed_template
            .merged_files
            .get(&Utf8PathBuf::from("README.md"))
            .unwrap();
        assert_eq!(readme_content, b"# Existing Project README");

        let agents_content = composed_template
            .merged_files
            .get(&Utf8PathBuf::from("AGENTS.md"))
            .unwrap();
        assert_eq!(agents_content, b"# Agents Documentation");
    }

    #[test]
    fn test_no_agents_file_leaves_readme_unchanged() {
        let template = create_test_template();

        // Create test files without AGENTS.md
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# My Project\n\nNo agents file here.".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("other.md"),
            b"Other file content".to_vec(),
        );

        let composed_template = MCPComposedTemplate::new_with_agents_merge(
            template,
            files.clone(),
            ProjectType::CreateNew,
        );

        // Files should remain unchanged when no AGENTS.md exists
        let file_list = composed_template.list_files();
        assert!(!file_list.contains(&Utf8PathBuf::from("AGENTS.md")));
        assert!(file_list.contains(&Utf8PathBuf::from("README.md")));
        assert_eq!(file_list.len(), 2);

        let readme_content = composed_template
            .merged_files
            .get(&Utf8PathBuf::from("README.md"))
            .unwrap();
        assert_eq!(readme_content, b"# My Project\n\nNo agents file here.");
    }

    #[test]
    fn test_creates_readme_if_missing_when_agents_exists() {
        let template = create_test_template();

        // Create test files with AGENTS.md but no README.md
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("AGENTS.md"),
            b"# Agents Documentation\n\nAgent content here.".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("other.md"),
            b"Other file content".to_vec(),
        );

        let composed_template =
            MCPComposedTemplate::new_with_agents_merge(template, files, ProjectType::CreateNew);

        // AGENTS.md should remain in file list
        let file_list = composed_template.list_files();
        assert!(file_list.contains(&Utf8PathBuf::from("AGENTS.md")));

        // README.md should be created with AGENTS content
        assert!(file_list.contains(&Utf8PathBuf::from("README.md")));
        let readme_content = composed_template
            .merged_files
            .get(&Utf8PathBuf::from("README.md"))
            .unwrap();
        let readme_str = String::from_utf8_lossy(readme_content);

        assert_eq!(readme_str, "# Agents Documentation\n\nAgent content here.");
    }

    #[test]
    fn test_proper_newline_spacing_when_merging() {
        let template = create_test_template();

        // Test various README endings
        let test_cases = vec![
            ("No newline at end", "No newline at end\n\n"),
            ("One newline at end\n", "One newline at end\n\n"),
            ("Two newlines at end\n\n", "Two newlines at end\n\n"),
            ("", ""), // Empty README case
        ];

        for (readme_ending, expected_prefix) in test_cases {
            let mut files = HashMap::new();
            files.insert(
                Utf8PathBuf::from("README.md"),
                readme_ending.as_bytes().to_vec(),
            );
            files.insert(Utf8PathBuf::from("AGENTS.md"), b"Agent content".to_vec());

            let composed_template = MCPComposedTemplate::new_with_agents_merge(
                template.clone(),
                files,
                ProjectType::CreateNew,
            );

            let readme_content = composed_template
                .merged_files
                .get(&Utf8PathBuf::from("README.md"))
                .unwrap();
            let readme_str = String::from_utf8_lossy(readme_content);

            let expected = format!("{}Agent content", expected_prefix);
            assert_eq!(
                readme_str, expected,
                "Failed for README ending: {:?}",
                readme_ending
            );
        }
    }

    #[test]
    fn test_mcp_composed_template_tracks_agents_merge_status() {
        let template = create_test_template();

        // Test case 1: New project with AGENTS.md should track that it was merged
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# My Project\n\nThis is the main README content.".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("AGENTS.md"),
            b"\n## AI Agents\n\nThis is the agents documentation.".to_vec(),
        );

        let composed_template = MCPComposedTemplate::new_with_agents_merge(
            template.clone(),
            files,
            ProjectType::CreateNew,
        );

        // Should track that agents were merged
        assert!(composed_template.agents_merged_into_readme);
        // AGENTS.md should remain in the file list (not removed after merging)
        assert!(
            composed_template
                .list_files()
                .contains(&Utf8PathBuf::from("AGENTS.md"))
        );
        // README.md should contain merged content
        assert!(
            composed_template
                .merged_files
                .contains_key(&Utf8PathBuf::from("README.md"))
        );

        // Test case 2: Existing project with AGENTS.md should NOT merge
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# Existing Project README".to_vec(),
        );
        files.insert(
            Utf8PathBuf::from("AGENTS.md"),
            b"# Agents Documentation".to_vec(),
        );

        let composed_template = MCPComposedTemplate::new_with_agents_merge(
            template.clone(),
            files,
            ProjectType::AddSubgraph,
        );

        // Should NOT track agents merge for existing projects
        assert!(!composed_template.agents_merged_into_readme);
        // AGENTS.md should remain in the file list
        assert!(
            composed_template
                .list_files()
                .contains(&Utf8PathBuf::from("AGENTS.md"))
        );

        // Test case 3: New project without AGENTS.md should not track merge
        let mut files = HashMap::new();
        files.insert(
            Utf8PathBuf::from("README.md"),
            b"# My Project\n\nNo agents here.".to_vec(),
        );

        let composed_template =
            MCPComposedTemplate::new_with_agents_merge(template, files, ProjectType::CreateNew);

        // Should NOT track agents merge when no AGENTS.md exists
        assert!(!composed_template.agents_merged_into_readme);
    }
}
