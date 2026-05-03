use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

fn saan() -> Command {
    Command::cargo_bin("saan_cli").expect("saan_cli binary not found")
}

#[test]
fn no_args_exits_nonzero() {
    saan().assert().failure();
}

#[test]
fn no_args_prints_usage_to_stderr() {
    saan().assert().failure().stderr(contains("Usage"));
}

#[test]
fn help_lists_subcommands() {
    saan()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("init"))
        .stdout(contains("prepare"))
        .stdout(contains("apply"))
        .stdout(contains("interlace"))
        .stdout(contains("inspect"))
        .stdout(contains("view"));
}

#[test]
fn unknown_command_exits_nonzero() {
    saan().arg("notacommand").assert().failure();
}

#[test]
fn init_creates_saan_file() {
    let dir = tempdir().unwrap();
    saan()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success();
    assert!(dir.path().join(".saan").exists(), ".saan file must be created");
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let dir = tempdir().unwrap();
    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("init")
        .arg(dir.path())
        .assert()
        .failure()
        .stderr(contains("already exists"));
}

#[test]
fn init_force_overwrites_existing_store() {
    let dir = tempdir().unwrap();
    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("init")
        .arg("--force")
        .arg(dir.path())
        .assert()
        .success();
}

#[test]
fn prepare_requires_input_arg() {
    saan().arg("prepare").assert().failure();
}

#[test]
fn interlace_exits_nonzero_with_not_implemented() {
    saan()
        .arg("interlace")
        .assert()
        .failure()
        .stderr(contains("not implemented"));
}

#[test]
fn inspect_exits_nonzero_with_not_implemented() {
    saan()
        .arg("inspect")
        .assert()
        .failure()
        .stderr(contains("not implemented"));
}

#[test]
fn view_exits_nonzero_with_not_implemented() {
    saan()
        .arg("view")
        .assert()
        .failure()
        .stderr(contains("not implemented"));
}

/// Full init → prepare → apply pipeline with a fixture SQL file.
#[test]
fn full_pipeline_end_to_end() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        "CREATE TABLE stg.orders AS SELECT * FROM raw.orders;\n\
         CREATE VIEW marts.summary AS SELECT * FROM stg.orders;\n",
    )
    .unwrap();

    // init
    saan().arg("init").arg(dir.path()).assert().success();

    // prepare
    saan()
        .arg("prepare")
        .arg(dir.path())
        .arg("--store")
        .arg(&store_path)
        .assert()
        .success()
        .stdout(contains("node(s)"));

    // apply
    saan()
        .arg("apply")
        .arg("--store")
        .arg(&store_path)
        .assert()
        .success()
        .stdout(contains("node(s)"));
}

/// Applying the same SQL twice must not duplicate nodes or edges.
#[test]
fn prepare_apply_idempotent() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("idempotent.sql");

    std::fs::write(
        &sql_path,
        "CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();

    for _ in 0..2 {
        saan()
            .arg("prepare")
            .arg(dir.path())
            .arg("--store")
            .arg(&store_path)
            .assert()
            .success();
        saan()
            .arg("apply")
            .arg("--store")
            .arg(&store_path)
            .assert()
            .success();
    }

    // After two rounds, the graph must still report the same totals.
    saan()
        .arg("apply")
        .arg("--store")
        .arg(&store_path)
        .assert()
        .success()
        .stdout(contains("2 node(s), 1 edge(s)"));
}
