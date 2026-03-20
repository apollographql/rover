use std::{
    io::Write,
    process::{Command, Stdio},
};

use assert_cmd::cargo;
use rstest::*;
use serde_json::Value;
use speculoos::prelude::*;
use tempfile::NamedTempFile;


/// A small but representative schema used for file-based tests.
const TEST_SDL: &str = r#"
type Query {
  "Get a user by ID"
  user(id: ID!): User
  "Get a post by ID"
  post(id: ID!): Post
}

type Mutation {
  createPost(input: CreatePostInput!): Post
}

"A registered user"
type User {
  id: ID!
  name: String!
  email: String!
  posts(limit: Int, offset: Int): [Post!]
  legacyId: String @deprecated(reason: "Use id instead")
}

type Post {
  id: ID!
  "The post title"
  title: String!
  body: String!
  author: User!
  oldSlug: String @deprecated(reason: "Use slug instead")
  slug: String!
}

input CreatePostInput {
  title: String!
  body: String!
}

enum Status {
  ACTIVE
  INACTIVE
}

union Content = Post | User
"#;

/// Creates a temp file containing `TEST_SDL` and returns it.
/// The file stays alive as long as the returned handle is in scope.
fn schema_file() -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("could not create temp file");
    f.write_all(TEST_SDL.as_bytes())
        .expect("could not write SDL to temp file");
    f
}

fn rover(args: &[&str]) -> std::process::Output {
    Command::new(cargo::cargo_bin!("rover"))
        .args(args)
        .output()
        .expect("could not run rover")
}

// ---------------------------------------------------------------------------
// File-source tests — no network, no credentials required
// ---------------------------------------------------------------------------

#[rstest]
fn file_overview_contains_schema_header() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--view", "description"]);

    assert!(output.status.success(), "command failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("SCHEMA");
}

#[rstest]
fn file_overview_lists_type_counts() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--view", "description"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Overview subheader has "N types" and "N fields"
    assert_that!(stdout).contains("types");
    assert_that!(stdout).contains("fields");
}

#[rstest]
fn file_overview_shows_operations_table() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--view", "description"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("Operations");
    assert_that!(stdout).contains("Query");
}

#[rstest]
fn file_type_detail_shows_type_header() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "User"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("TYPE User (object)");
}

#[rstest]
fn file_type_detail_shows_field_names() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Post"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("title");
    assert_that!(stdout).contains("body");
    assert_that!(stdout).contains("author");
}

#[rstest]
fn file_type_detail_shows_deprecated_field_with_reason() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Post", "--include-deprecated"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("deprecated: Use slug instead");
}

#[rstest]
fn file_type_detail_enum_shows_values() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Status"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("TYPE Status (enum)");
    assert_that!(stdout).contains("ACTIVE");
    assert_that!(stdout).contains("INACTIVE");
}

#[rstest]
fn file_type_detail_union_shows_members() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Content"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("TYPE Content (union)");
    assert_that!(stdout).contains("Members:");
    assert_that!(stdout).contains("Post");
    assert_that!(stdout).contains("User");
}

#[rstest]
fn file_type_detail_input_shows_fields() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "CreatePostInput"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("TYPE CreatePostInput (input)");
    assert_that!(stdout).contains("title");
    assert_that!(stdout).contains("body");
}

#[rstest]
fn file_field_detail_shows_field_header() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Query.user"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("FIELD Query.user:");
}

#[rstest]
fn file_field_detail_shows_args() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "User.posts"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("Args");
    assert_that!(stdout).contains("limit");
    assert_that!(stdout).contains("offset");
}

#[rstest]
fn file_field_detail_deprecated_shows_notice() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Post.oldSlug"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("DEPRECATED: Use slug instead");
}

#[rstest]
fn file_field_detail_shows_return_type_expansion() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Query.post"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("Return type:");
}

#[rstest]
fn file_field_detail_shows_input_expansion_for_mutation() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Mutation.createPost"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("Input types");
    assert_that!(stdout).contains("CreatePostInput");
}

#[rstest]
fn file_view_sdl_outputs_raw_sdl() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--view", "sdl"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("type Query");
}

#[rstest]
fn file_view_sdl_with_coord_outputs_filtered_sdl() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "User", "--view", "sdl"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("type User");
    // Should not include unrelated types in filtered output
    assert_that!(stdout).does_not_contain("type Post");
}

#[rstest]
fn file_format_json_outputs_valid_json() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--format", "json"]);

    assert!(output.status.success());
    let parsed: Result<Value, _> = serde_json::from_slice(&output.stdout);
    assert_that!(parsed).is_ok();
}

#[rstest]
fn file_format_json_with_coord_outputs_valid_json() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "Post", "--format", "json"]);

    assert!(output.status.success());
    let parsed: Result<Value, _> = serde_json::from_slice(&output.stdout);
    assert_that!(parsed).is_ok();
}

#[rstest]
fn file_nonexistent_coord_exits_nonzero() {
    let file = schema_file();
    let path = file.path().to_str().unwrap();

    let output = rover(&["schema", "describe", path, "--coord", "NonExistentType"]);

    assert!(!output.status.success());
}

#[rstest]
fn nonexistent_file_exits_nonzero() {
    let output = rover(&["schema", "describe", "/tmp/this_file_does_not_exist_rover.graphql"]);
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// Stdin tests — pipes SDL via stdin
// ---------------------------------------------------------------------------

fn rover_with_stdin(args: &[&str], stdin_content: &str) -> std::process::Output {
    let mut child = Command::new(cargo::cargo_bin!("rover"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("could not spawn rover");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_content.as_bytes())
        .expect("could not write to stdin");

    child.wait_with_output().expect("could not wait for rover")
}

#[rstest]
fn stdin_overview_contains_schema_header() {
    let output = rover_with_stdin(
        &["schema", "describe", "--view", "description"],
        TEST_SDL,
    );

    assert!(output.status.success(), "command failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("SCHEMA");
    assert_that!(stdout).contains("<stdin>");
}

#[rstest]
fn stdin_dash_arg_reads_from_stdin() {
    let output = rover_with_stdin(
        &["schema", "describe", "-", "--view", "description"],
        TEST_SDL,
    );

    assert!(output.status.success(), "command failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).contains("SCHEMA");
}

#[rstest]
fn stdin_type_detail_with_coord() {
    let output = rover_with_stdin(
        &["schema", "describe", "--coord", "User"],
        TEST_SDL,
    );

    assert!(output.status.success(), "command failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(stdout).starts_with("TYPE User (object)");
}

#[rstest]
fn stdin_format_json_outputs_valid_json() {
    let output = rover_with_stdin(
        &["schema", "describe", "--format", "json"],
        TEST_SDL,
    );

    assert!(output.status.success(), "command failed: {:?}", output);
    let parsed: Result<Value, _> = serde_json::from_slice(&output.stdout);
    assert_that!(parsed).is_ok();
}
