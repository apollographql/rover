use assert_cmd::Command;

#[test]
fn client_extract_help_works() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("client")
        .arg("extract")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn client_check_help_works() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("client")
        .arg("check")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn client_extract_writes_graphql_files() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path();
    let source_dir = project_root.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source_file = source_dir.join("query.tsx");
    std::fs::write(
        &source_file,
        r#"import { gql } from "@apollo/client";
const query = gql`
  query Hello {
    hello
  }
`;"#,
    )
    .unwrap();

    let out_dir = project_root.join("graphql");
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.current_dir(project_root)
        .arg("client")
        .arg("extract")
        .arg("--include")
        .arg(source_file.to_str().unwrap())
        .arg("--out-dir")
        .arg(out_dir.to_str().unwrap())
        .assert()
        .success();

    let generated = out_dir.join("src").join("query.graphql");
    assert!(generated.exists());
    let contents = std::fs::read_to_string(generated).unwrap();
    assert!(contents.contains("query Hello"));
}
