use assert_cmd::Command;
use std::fs;

#[test]
fn client_extract_kotlin_conflict_suffix() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path();
    let src_dir = project_root.join("android");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("Query.kt");
    fs::write(
        &file,
        r#"
        val query = """
          query Hello { hello }
        """
        "#,
    )
    .unwrap();
    let out_dir = project_root.join("graphql");
    let generated = out_dir.join("android").join("Query.graphql");
    fs::create_dir_all(generated.parent().unwrap()).unwrap();
    fs::write(&generated, "existing").unwrap();

    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.current_dir(project_root)
        .arg("client")
        .arg("extract")
        .arg("--include")
        .arg(file.to_str().unwrap())
        .arg("--out-dir")
        .arg(out_dir.to_str().unwrap())
        .assert()
        .success();

    let suffixed = out_dir.join("android").join("Query.generated.graphql");
    assert!(suffixed.exists());
    let contents = fs::read_to_string(suffixed).unwrap();
    assert!(contents.contains("query Hello"));
}
