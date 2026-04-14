/// A metadata asset node in the lineage graph (e.g. a table, view, or file).
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Unique identifier for the asset (e.g. "schema.table_name").
    pub id: String,
    /// Human-readable label shown in the visualizer.
    pub label: String,
    /// Source type (e.g. "sql", "csv", "parquet", "dbt").
    pub source_type: String,
}

impl Node {
    pub fn new(id: impl Into<String>, label: impl Into<String>, source_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            source_type: source_type.into(),
        }
    }
}

/// A directed lineage connection between two nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// The upstream (source) node id.
    pub from: String,
    /// The downstream (target) node id.
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

/// A directed acyclic graph of metadata assets and their lineage connections.
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a node to the graph. Returns the index of the inserted node.
    pub fn add_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Add a directed edge between two node ids.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Returns `true` if the graph contains a cycle.
    pub fn has_cycle(&self) -> bool {
        // Build an adjacency list keyed by node id.
        let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for node in &self.nodes {
            adj.entry(node.id.as_str()).or_default();
        }
        for edge in &self.edges {
            adj.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
        }

        // DFS-based cycle detection with three-colour marking.
        #[derive(PartialEq)]
        enum Mark { Unvisited, InStack, Done }

        let mut marks: std::collections::HashMap<&str, Mark> = adj.keys().map(|&k| (k, Mark::Unvisited)).collect();

        fn dfs<'a>(
            node: &'a str,
            adj: &std::collections::HashMap<&'a str, Vec<&'a str>>,
            marks: &mut std::collections::HashMap<&'a str, Mark>,
        ) -> bool {
            marks.insert(node, Mark::InStack);
            if let Some(neighbours) = adj.get(node) {
                for &next in neighbours {
                    match marks.get(next) {
                        Some(Mark::InStack) => return true,
                        Some(Mark::Done) => {}
                        _ => {
                            if dfs(next, adj, marks) {
                                return true;
                            }
                        }
                    }
                }
            }
            marks.insert(node, Mark::Done);
            false
        }

        let keys: Vec<&str> = marks.keys().copied().collect();
        for key in keys {
            if marks.get(key) == Some(&Mark::Unvisited) && dfs(key, &adj, &mut marks) {
                return true;
            }
        }
        false
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
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

    // --- Node tests ---

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

    // --- Edge tests ---

    #[test]
    fn edge_stores_fields() {
        let edge = Edge::new("raw.orders", "marts.order_summary");
        assert_eq!(edge.from, "raw.orders");
        assert_eq!(edge.to, "marts.order_summary");
    }

    // --- Graph tests ---

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
        // raw.orders → stg.orders → marts.summary
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
        // a → b → a
        let mut g = Graph::new();
        g.add_node(Node::new("a", "A", "sql"));
        g.add_node(Node::new("b", "B", "sql"));
        g.add_edge(Edge::new("a", "b"));
        g.add_edge(Edge::new("b", "a"));
        assert!(g.has_cycle());
    }

    #[test]
    fn graph_detects_indirect_cycle() {
        // a → b → c → a
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
