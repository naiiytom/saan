use saan_core::{Edge, Graph, Node, ShaverRegistry, Store};

// ── Graph API (prepare / interlace / inspect simulation) ─────────────────────

fn prepare_sample_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(Node::new("raw.orders", "Raw Orders", "sql"));
    g.add_node(Node::new("raw.customers", "Raw Customers", "sql"));
    g.add_node(Node::new("stg.orders", "Staged Orders", "sql"));
    g.add_node(Node::new("marts.order_summary", "Order Summary", "sql"));
    g
}

fn interlace(g: &mut Graph) {
    g.add_edge(Edge::new("raw.orders", "stg.orders"));
    g.add_edge(Edge::new("raw.customers", "stg.orders"));
    g.add_edge(Edge::new("stg.orders", "marts.order_summary"));
}

#[test]
fn prepare_produces_correct_node_count() {
    let g = prepare_sample_graph();
    assert_eq!(g.node_count(), 4);
}

#[test]
fn interlace_produces_correct_edge_count() {
    let mut g = prepare_sample_graph();
    interlace(&mut g);
    assert_eq!(g.edge_count(), 3);
}

#[test]
fn full_pipeline_graph_is_acyclic() {
    let mut g = prepare_sample_graph();
    interlace(&mut g);
    assert!(!g.has_cycle(), "lineage graph must not contain cycles");
}

#[test]
fn introducing_circular_lineage_is_detected() {
    let mut g = prepare_sample_graph();
    interlace(&mut g);
    g.add_edge(Edge::new("marts.order_summary", "raw.orders"));
    assert!(g.has_cycle(), "circular lineage must be detected by inspect");
}

// ── SqlShaver fixture tests ───────────────────────────────────────────────────

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sql")
        .join(name)
}

#[test]
fn orders_pipeline_fixture_extracts_lineage() {
    let registry = ShaverRegistry::with_builtins();
    let strands = registry
        .shave_path(&fixture("orders_pipeline.sql"))
        .unwrap();

    let all_nodes: Vec<_> = strands.iter().flat_map(|s| s.nodes.iter()).collect();
    let all_edges: Vec<_> = strands.iter().flat_map(|s| s.edges.iter()).collect();

    let node_ids: Vec<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"raw.orders"), "raw.orders missing");
    assert!(node_ids.contains(&"stg.orders"), "stg.orders missing");
    assert!(node_ids.contains(&"marts.order_summary"), "marts.order_summary missing");

    assert!(
        all_edges.iter().any(|e| e.from == "stg.orders" && e.to == "marts.order_summary"),
        "stg.orders → marts.order_summary edge missing"
    );
}

#[test]
fn cte_fixture_cte_name_not_a_node() {
    let registry = ShaverRegistry::with_builtins();
    let strands = registry
        .shave_path(&fixture("with_cte.sql"))
        .unwrap();

    let all_nodes: Vec<_> = strands.iter().flat_map(|s| s.nodes.iter()).collect();
    let node_ids: Vec<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();

    assert!(!node_ids.contains(&"active"), "CTE 'active' must not appear as a node");
    assert!(!node_ids.contains(&"counts"), "CTE 'counts' must not appear as a node");
    assert!(node_ids.contains(&"raw.customers"), "raw.customers must be an upstream");
    assert!(node_ids.contains(&"raw.orders"), "raw.orders must be an upstream");
    assert!(node_ids.contains(&"marts.customer_stats"), "target must be present");
}

// ── Store round-trip ──────────────────────────────────────────────────────────

#[test]
fn store_prepare_apply_round_trip() {
    use saan_core::Strand;
    use std::path::PathBuf;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let store = Store::open(&store_path).unwrap();
    store.init_schema().unwrap();

    let mut strand = Strand::new(PathBuf::from("fixture.sql"));
    strand.nodes.push(Node::new("raw.orders", "Raw Orders", "sql"));
    strand.nodes.push(Node::new("stg.orders", "Staged Orders", "sql"));
    strand.edges.push(Edge::new("raw.orders", "stg.orders"));

    store.write_strands_to_staging(&[strand]).unwrap();
    store.apply_staging().unwrap();

    let g = store.load_graph().unwrap();
    assert_eq!(g.node_count(), 2);
    assert_eq!(g.edge_count(), 1);
    assert!(!g.has_cycle());
}

#[test]
fn store_apply_is_idempotent() {
    use saan_core::Strand;
    use std::path::PathBuf;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let store_path = dir.path().join(".saan");
    let store = Store::open(&store_path).unwrap();
    store.init_schema().unwrap();

    let make_strand = || {
        let mut s = Strand::new(PathBuf::from("fixture.sql"));
        s.nodes.push(Node::new("raw.orders", "Raw Orders", "sql"));
        s
    };

    store.write_strands_to_staging(&[make_strand()]).unwrap();
    store.apply_staging().unwrap();
    store.write_strands_to_staging(&[make_strand()]).unwrap();
    store.apply_staging().unwrap();

    let g = store.load_graph().unwrap();
    assert_eq!(g.node_count(), 1, "same node applied twice must not duplicate");
}
