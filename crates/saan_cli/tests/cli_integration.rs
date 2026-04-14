use assert_cmd::Command;
use predicates::str::contains;

/// Helper that returns a `Command` pointing at the `saan_cli` binary.
fn saan() -> Command {
    Command::cargo_bin("saan_cli").expect("saan_cli binary not found")
}

#[test]
fn no_args_exits_nonzero() {
    saan().assert().failure();
}

#[test]
fn no_args_prints_usage() {
    saan()
        .assert()
        .failure()
        .stderr(contains("Usage: saan <command>"));
}

#[test]
fn no_args_lists_commands_in_stderr() {
    saan()
        .assert()
        .failure()
        .stderr(contains("init"))
        .stderr(contains("prepare"))
        .stderr(contains("interlace"))
        .stderr(contains("apply"))
        .stderr(contains("inspect"))
        .stderr(contains("view"));
}

#[test]
fn init_command_exits_zero() {
    saan().arg("init").assert().success();
}

#[test]
fn prepare_command_exits_zero() {
    saan().arg("prepare").assert().success();
}

#[test]
fn unknown_command_exits_nonzero() {
    saan().arg("notacommand").assert().failure();
}
