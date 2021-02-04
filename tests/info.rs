use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn it_prints_info() {
    let mut cmd = Command::cargo_bin("rover").unwrap();
    let result = cmd.arg("info").assert().success();

    println!("{:?}", &result.get_output());

    //result.stdout(predicate::str::contains("Info"));
}
