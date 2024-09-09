use assert_cmd::Command;

#[test]
fn it_has_a_graph_fetch_command() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    cmd.arg("graph")
        .arg("fetch")
        .arg("--help")
        .assert()
        .success();
}
