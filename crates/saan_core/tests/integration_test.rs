//! Integration tests for the saan_core pipeline.
//!
//! These tests exercise the intended end-to-end lineage flow:
//!
//!   prepare → interlace → apply → inspect
//!
//! Most stages are not yet implemented; tests here serve as living
//! documentation of the expected behaviour and will be expanded as each
//! stage is built out.

use saan_core::{Edge, Graph, Node};

/// Simulate the "prepare" step: build a graph from raw metadata nodes.
/// In production this will read from SQL files, dbt manifests, or CSVs.
fn prepare_sample_graph() -> Graph {
    let mut g = Graph::new();
    g.add_node(Node::new("raw.orders", "Raw Orders", "sql"));
    g.add_node(Node::new("raw.customers", "Raw Customers", "sql"));
    g.add_node(Node::new("stg.orders", "Staged Orders", "sql"));
    g.add_node(Node::new("marts.order_summary", "Order Summary", "sql"));
    g
}

/// Simulate the "interlace" step: define edges (lineage connections).
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
    // A valid lineage graph must be a DAG — no circular dependencies.
    let mut g = prepare_sample_graph();
    interlace(&mut g);
    assert!(!g.has_cycle(), "lineage graph must not contain cycles");
}

#[test]
fn introducing_circular_lineage_is_detected() {
    // Verify that `inspect` would catch a bad interlace introducing a cycle.
    let mut g = prepare_sample_graph();
    interlace(&mut g);
    // Introduce a back-edge to simulate a misconfigured lineage definition.
    g.add_edge(Edge::new("marts.order_summary", "raw.orders"));
    assert!(g.has_cycle(), "circular lineage must be detected by inspect");
}
