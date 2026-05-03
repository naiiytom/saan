use petgraph::algo::is_cyclic_directed;
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub source_type: String,
}

impl Node {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        source_type: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            source_type: source_type.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

impl Edge {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    inner: StableDiGraph<String, ()>,
    id_to_index: HashMap<String, NodeIndex>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            inner: StableDiGraph::new(),
            id_to_index: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) -> usize {
        let idx = self.inner.add_node(node.id.clone());
        self.id_to_index.insert(node.id.clone(), idx);
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub fn add_edge(&mut self, edge: Edge) {
        if let (Some(&from_idx), Some(&to_idx)) = (
            self.id_to_index.get(&edge.from),
            self.id_to_index.get(&edge.to),
        ) {
            self.inner.add_edge(from_idx, to_idx, ());
        }
        self.edges.push(edge);
    }

    pub fn has_cycle(&self) -> bool {
        is_cyclic_directed(&self.inner)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_stores_fields() {
        let node = Node::new("raw.orders", "Orders", "sql");
        assert_eq!(node.id, "raw.orders");
        assert_eq!(node.label, "Orders");
        assert_eq!(node.source_type, "sql");
    }

    #[test]
    fn node_equality() {
        let a = Node::new("raw.orders", "Orders", "sql");
        let b = Node::new("raw.orders", "Orders", "sql");
        assert_eq!(a, b);
    }

    #[test]
    fn edge_stores_fields() {
        let edge = Edge::new("raw.orders", "marts.order_summary");
        assert_eq!(edge.from, "raw.orders");
        assert_eq!(edge.to, "marts.order_summary");
    }

    #[test]
    fn graph_add_node_returns_index() {
        let mut g = Graph::new();
        let idx = g.add_node(Node::new("raw.orders", "Orders", "sql"));
        assert_eq!(idx, 0);
        let idx2 = g.add_node(Node::new("marts.summary", "Summary", "sql"));
        assert_eq!(idx2, 1);
    }

    #[test]
    fn graph_add_edge_increases_edge_count() {
        let mut g = Graph::new();
        g.add_node(Node::new("raw.orders", "Orders", "sql"));
        g.add_node(Node::new("marts.summary", "Summary", "sql"));
        g.add_edge(Edge::new("raw.orders", "marts.summary"));
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn graph_no_cycle_for_simple_dag() {
        let mut g = Graph::new();
        g.add_node(Node::new("raw.orders", "Orders", "sql"));
        g.add_node(Node::new("stg.orders", "Staged Orders", "sql"));
        g.add_node(Node::new("marts.summary", "Summary", "sql"));
        g.add_edge(Edge::new("raw.orders", "stg.orders"));
        g.add_edge(Edge::new("stg.orders", "marts.summary"));
        assert!(!g.has_cycle());
    }

    #[test]
    fn graph_detects_direct_cycle() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        g.add_node(Node::new("b", "B", "sql"));
        g.add_edge(Edge::new("a", "b"));
        g.add_edge(Edge::new("b", "a"));
        assert!(g.has_cycle());
    }

    #[test]
    fn graph_detects_indirect_cycle() {
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        g.add_node(Node::new("b", "B", "sql"));
        g.add_node(Node::new("c", "C", "sql"));
        g.add_edge(Edge::new("a", "b"));
        g.add_edge(Edge::new("b", "c"));
        g.add_edge(Edge::new("c", "a"));
        assert!(g.has_cycle());
    }

    #[test]
    fn empty_graph_has_no_cycle() {
        let g = Graph::new();
        assert!(!g.has_cycle());
    }
}
