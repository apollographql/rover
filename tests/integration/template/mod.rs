use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn template_use_requires_template_when_not_a_tty() {
    let mut cmd = cargo_bin_cmd!("rover");
    // assert_cmd captures stderr, so rover is not attached to a TTY
    cmd.args(["template", "use", "some-path"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--template is required when not attached to a TTY",
        ));
}
