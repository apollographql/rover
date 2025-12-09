use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn it_has_a_graph_fetch_command() {
    let mut cmd = cargo_bin_cmd!("rover");
    cmd.arg("graph")
        .arg("fetch")
        .arg("--help")
        .assert()
        .success();
}
