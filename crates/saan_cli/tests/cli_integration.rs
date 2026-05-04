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
    saan().arg("init").arg(dir.path()).assert().success();
    assert!(
        dir.path().join(".saan").exists(),
        ".saan file must be created"
    );
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
fn interlace_adds_transitive_edge_end_to_end() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    // a → b → c; interlace should compute a → c
    std::fs::write(
        &sql_path,
        b"CREATE TABLE b AS SELECT * FROM a;\nCREATE TABLE c AS SELECT * FROM b;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("interlace")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("1 computed edge"));

    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("3 edge(s)"));
}

#[test]
fn inspect_reports_node_and_edge_counts() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("inspect")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("Nodes:"))
        .stdout(contains("Edges:"));
}

#[test]
fn view_writes_html_file_with_svg() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");
    let out_path = dir.path().join("lineage.html");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("view")
        .arg("--store").arg(&store_path)
        .arg("--out").arg(&out_path)
        .assert()
        .success()
        .stdout(contains("Written:"))
        .stdout(contains("node(s)"));

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"), "output must be HTML");
    assert!(content.contains("<svg"), "output must contain SVG");
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
        .stdout(contains("Staged:"))
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

#[test]
fn query_returns_table_output() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("query")
        .arg("SELECT COUNT(*) AS cnt FROM nodes")
        .arg("--store").arg(&store_path)
        .assert()
        .success()
        .stdout(contains("cnt"))
        .stdout(contains("2"));
}

#[test]
fn query_csv_format() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("query")
        .arg("SELECT id FROM nodes ORDER BY id")
        .arg("--store").arg(&store_path)
        .arg("--format").arg("csv")
        .assert()
        .success()
        .stdout(contains("id\n"));
}

#[test]
fn query_json_format() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("pipeline.sql");

    std::fs::write(
        &sql_path,
        b"CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
    )
    .unwrap();

    saan().arg("init").arg(dir.path()).assert().success();
    saan()
        .arg("prepare").arg(dir.path())
        .arg("--store").arg(&store_path)
        .assert().success();
    saan()
        .arg("apply")
        .arg("--store").arg(&store_path)
        .assert().success();

    saan()
        .arg("query")
        .arg("SELECT id FROM nodes ORDER BY id")
        .arg("--store").arg(&store_path)
        .arg("--format").arg("json")
        .assert()
        .success()
        .stdout(contains("[{"))
        .stdout(contains("\"id\""));
}

#[test]
fn query_missing_store_exits_nonzero() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");

    saan()
        .arg("query")
        .arg("SELECT 1")
        .arg("--store").arg(&store_path)
        .assert()
        .failure()
        .stderr(contains("store not found"));
}

#[test]
fn prepare_with_postgres_dialect_parses_cast_syntax() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let sql_path = dir.path().join("cast.sql");

    std::fs::write(&sql_path, b"CREATE TABLE t AS SELECT id::text FROM src").unwrap();

    saan().arg("init").arg(dir.path()).assert().success();

    saan()
        .arg("prepare")
        .arg(dir.path())
        .arg("--store").arg(&store_path)
        .arg("--dialect").arg("postgres")
        .assert()
        .success();
}
