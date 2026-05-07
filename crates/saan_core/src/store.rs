use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use duckdb::Connection;
use duckdb::types::Value;
use thiserror::Error;

use crate::graph::{Edge, Graph, Node};
use crate::strand::Strand;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] duckdb::Error),
    #[error("{0}")]
    QueryNotAllowed(String),
}

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InspectReport {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub orphan_nodes: Vec<String>,
    pub cycle_detected: bool,
    pub external_refs: Vec<String>,
}

pub struct Store {
    conn: Connection,
}

fn value_to_string(v: Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::TinyInt(n) => n.to_string(),
        Value::SmallInt(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::BigInt(n) => n.to_string(),
        Value::HugeInt(n) => n.to_string(),
        Value::UTinyInt(n) => n.to_string(),
        Value::USmallInt(n) => n.to_string(),
        Value::UInt(n) => n.to_string(),
        Value::UBigInt(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::Text(s) => s,
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
        Value::Date32(d) => d.to_string(),
        Value::Time64(_, t) => t.to_string(),
        Value::Timestamp(_, ts) => ts.to_string(),
        Value::Interval {
            months,
            days,
            nanos,
        } => {
            format!("{months}mo {days}d {nanos}ns")
        }
        Value::List(items) => {
            let parts: Vec<String> = items.into_iter().map(value_to_string).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Enum(s) => s,
        Value::Struct(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", value_to_string(v.clone())))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.into_iter().map(value_to_string).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(pairs) => {
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}: {}",
                        value_to_string(k.clone()),
                        value_to_string(v.clone())
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Union(inner) => value_to_string(*inner),
    }
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS saan_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO saan_meta (key, value) VALUES ('schema_version', '1')
                ON CONFLICT (key) DO NOTHING;

            CREATE TABLE IF NOT EXISTS nodes (
                id            TEXT PRIMARY KEY,
                label         TEXT NOT NULL,
                source_type   TEXT NOT NULL,
                first_seen_at TIMESTAMPTZ NOT NULL,
                last_seen_at  TIMESTAMPTZ NOT NULL
            );

            CREATE TABLE IF NOT EXISTS edges (
                from_id TEXT NOT NULL,
                to_id   TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id)
            );

            CREATE TABLE IF NOT EXISTS staging_nodes (
                id          TEXT NOT NULL,
                label       TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_path TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS staging_edges (
                from_id     TEXT NOT NULL,
                to_id       TEXT NOT NULL,
                source_path TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    pub fn write_strands_to_staging(&self, strands: &[Strand]) -> Result<(), StoreError> {
        self.conn.execute_batch("BEGIN")?;
        let result: Result<(), StoreError> = (|| {
            let mut node_stmt = self.conn.prepare(
                "INSERT INTO staging_nodes (id, label, source_type, source_path) VALUES (?, ?, ?, ?)",
            )?;
            let mut edge_stmt = self.conn.prepare(
                "INSERT INTO staging_edges (from_id, to_id, source_path) VALUES (?, ?, ?)",
            )?;
            for strand in strands {
                let path = strand.source_path.to_string_lossy();
                for node in &strand.nodes {
                    node_stmt.execute(duckdb::params![
                        node.id,
                        node.label,
                        node.source_type,
                        path
                    ])?;
                }
                for edge in &strand.edges {
                    edge_stmt.execute(duckdb::params![edge.from, edge.to, path])?;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK");
        } else {
            self.conn.execute_batch("COMMIT")?;
        }
        result
    }

    pub fn apply_staging(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "
            INSERT INTO nodes (id, label, source_type, first_seen_at, last_seen_at)
            SELECT DISTINCT id, label, source_type, now(), now()
            FROM staging_nodes
            ON CONFLICT (id) DO UPDATE SET
                label        = excluded.label,
                source_type  = excluded.source_type,
                last_seen_at = now();

            INSERT INTO edges (from_id, to_id)
            SELECT DISTINCT from_id, to_id FROM staging_edges
            ON CONFLICT (from_id, to_id) DO NOTHING;

            DELETE FROM staging_nodes;
            DELETE FROM staging_edges;
            ",
        )?;
        Ok(())
    }

    pub fn interlace_staging(&self) -> Result<usize, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT from_id, to_id FROM staging_edges")?;
        let pairs: Vec<(String, String)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if pairs.is_empty() {
            return Ok(0);
        }

        let existing: HashSet<(String, String)> = pairs.iter().cloned().collect();

        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for (from, to) in &pairs {
            adj.entry(from.clone()).or_default().push(to.clone());
        }

        let sources: Vec<String> = adj.keys().cloned().collect();
        let mut new_edges: HashSet<(String, String)> = HashSet::new();

        for start in &sources {
            let mut visited: HashSet<String> = HashSet::new();
            let mut queue: VecDeque<String> = VecDeque::new();
            visited.insert(start.clone());
            queue.push_back(start.clone());

            while let Some(current) = queue.pop_front() {
                if let Some(neighbors) = adj.get(&current) {
                    for neighbor in neighbors {
                        let pair = (start.clone(), neighbor.clone());
                        if neighbor != start && !existing.contains(&pair) {
                            new_edges.insert(pair);
                        }
                        if visited.insert(neighbor.clone()) {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        let count = new_edges.len();
        if count == 0 {
            return Ok(0);
        }

        self.conn.execute_batch("BEGIN")?;
        let result: Result<(), StoreError> = (|| {
            let mut stmt = self.conn.prepare(
                "INSERT INTO staging_edges (from_id, to_id, source_path) VALUES (?, ?, ?)",
            )?;
            for (from, to) in &new_edges {
                stmt.execute(duckdb::params![from, to, "<interlaced>"])?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK");
        } else {
            self.conn.execute_batch("COMMIT")?;
        }
        result?;

        Ok(count)
    }

    pub fn load_graph(&self) -> Result<Graph, StoreError> {
        let mut g = Graph::new();

        let mut stmt = self
            .conn
            .prepare("SELECT id, label, source_type FROM nodes")?;
        let nodes: Vec<Node> = stmt
            .query_map([], |row| {
                Ok(Node::new(
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for node in nodes {
            g.add_node(node);
        }

        let mut stmt = self.conn.prepare("SELECT from_id, to_id FROM edges")?;
        let edges: Vec<Edge> = stmt
            .query_map([], |row| {
                Ok(Edge::new(
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for edge in edges {
            g.add_edge(edge);
        }

        Ok(g)
    }

    pub fn query(&self, sql: &str) -> Result<QueryResult, StoreError> {
        use sqlparser::ast::Statement;
        use sqlparser::dialect::DuckDbDialect;
        use sqlparser::parser::Parser;

        // Guard against DDL/DML that could corrupt the store.
        // If sqlparser can parse the input and finds a non-SELECT statement, reject it.
        // Unknown syntax (DuckDB-specific) passes through and is validated by DuckDB itself.
        if let Ok(stmts) = Parser::parse_sql(&DuckDbDialect {}, sql) {
            for stmt in &stmts {
                if !matches!(stmt, Statement::Query(_)) {
                    return Err(StoreError::QueryNotAllowed(
                        "only SELECT statements are permitted".into(),
                    ));
                }
            }
        }

        let mut stmt = self.conn.prepare(sql)?;
        // query([]) executes the statement and populates schema metadata on Rows.
        let mut result = stmt.query([])?;
        let columns: Vec<String> = result
            .as_ref()
            .map(|s| s.column_names())
            .unwrap_or_default();
        let col_count = columns.len();
        let mut rows: Vec<Vec<String>> = Vec::new();
        while let Some(row) = result.next()? {
            let vals: Vec<String> = (0..col_count)
                .map(|i| value_to_string(row.get::<_, Value>(i).unwrap_or(Value::Null)))
                .collect();
            rows.push(vals);
        }
        Ok(QueryResult { columns, rows })
    }

    pub fn inspect(&self) -> Result<InspectReport, StoreError> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM nodes")?;
        let total_nodes: usize = stmt
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|n| n as usize)?;

        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM edges")?;
        let total_edges: usize = stmt
            .query_row([], |row| row.get::<_, i64>(0))
            .map(|n| n as usize)?;

        let mut stmt = self.conn.prepare(
            "SELECT id FROM nodes
             WHERE id NOT IN (SELECT from_id FROM edges)
               AND id NOT IN (SELECT to_id   FROM edges)
             ORDER BY id",
        )?;
        let orphan_nodes: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT id FROM (
                 SELECT from_id AS id FROM edges
                  WHERE from_id NOT IN (SELECT id FROM nodes)
                 UNION ALL
                 SELECT to_id AS id FROM edges
                  WHERE to_id NOT IN (SELECT id FROM nodes)
             ) t ORDER BY id",
        )?;
        let external_refs: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let graph = self.load_graph()?;
        let cycle_detected = graph.has_cycle();

        Ok(InspectReport {
            total_nodes,
            total_edges,
            orphan_nodes,
            cycle_detected,
            external_refs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strand::Strand;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_store() -> (Store, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".saan");
        let store = Store::open(&path).unwrap();
        store.init_schema().unwrap();
        (store, dir)
    }

    fn strand_with(nodes: &[(&str, &str)], edges: &[(&str, &str)]) -> Strand {
        let mut s = Strand::new(PathBuf::from("test.sql"));
        for (id, label) in nodes {
            s.nodes.push(Node::new(*id, *label, "sql"));
        }
        for (from, to) in edges {
            s.edges.push(Edge::new(*from, *to));
        }
        s
    }

    #[test]
    fn init_schema_is_idempotent() {
        let (store, _dir) = make_store();
        store.init_schema().unwrap(); // second call must not error
    }

    #[test]
    fn write_and_apply_round_trips() {
        let (store, _dir) = make_store();

        let strands = vec![strand_with(
            &[
                ("raw.orders", "Raw Orders"),
                ("stg.orders", "Staged Orders"),
            ],
            &[("raw.orders", "stg.orders")],
        )];
        store.write_strands_to_staging(&strands).unwrap();
        store.apply_staging().unwrap();

        let g = store.load_graph().unwrap();
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 1);
    }

    #[test]
    fn apply_twice_is_idempotent() {
        let (store, _dir) = make_store();

        let strands = vec![strand_with(&[("raw.orders", "Raw Orders")], &[])];
        store.write_strands_to_staging(&strands).unwrap();
        store.apply_staging().unwrap();

        // Prepare + apply the same data again.
        store.write_strands_to_staging(&strands).unwrap();
        store.apply_staging().unwrap();

        let g = store.load_graph().unwrap();
        assert_eq!(g.node_count(), 1, "idempotent: must not duplicate nodes");
    }

    #[test]
    fn staging_cleared_after_apply() {
        let (store, _dir) = make_store();

        let strands = vec![strand_with(&[("raw.orders", "Raw Orders")], &[])];
        store.write_strands_to_staging(&strands).unwrap();
        store.apply_staging().unwrap();

        // Apply again with no new strands — node count must not change.
        store.apply_staging().unwrap();
        let g = store.load_graph().unwrap();
        assert_eq!(g.node_count(), 1);
    }

    #[test]
    fn inspect_detects_orphan_node() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(&[("orphan", "Orphan")], &[])])
            .unwrap();
        store.apply_staging().unwrap();

        let report = store.inspect().unwrap();
        assert!(report.orphan_nodes.contains(&"orphan".to_string()));
        assert!(!report.cycle_detected);
        assert!(report.external_refs.is_empty());
    }

    #[test]
    fn inspect_reports_external_ref() {
        let (store, _dir) = make_store();
        // raw.orders used as a source edge endpoint but has no node row
        store
            .write_strands_to_staging(&[strand_with(
                &[("stg.orders", "Staged")],
                &[("raw.orders", "stg.orders")],
            )])
            .unwrap();
        store.apply_staging().unwrap();

        let report = store.inspect().unwrap();
        assert!(report.external_refs.contains(&"raw.orders".to_string()));
    }

    #[test]
    fn interlace_adds_transitive_edge() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(
                &[("a", "A"), ("b", "B"), ("c", "C")],
                &[("a", "b"), ("b", "c")],
            )])
            .unwrap();

        let added = store.interlace_staging().unwrap();
        assert_eq!(added, 1);

        store.apply_staging().unwrap();
        let g = store.load_graph().unwrap();
        assert_eq!(g.edge_count(), 3); // a→b, b→c, a→c
    }

    #[test]
    fn interlace_single_hop_adds_nothing() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(&[("a", "A"), ("b", "B")], &[("a", "b")])])
            .unwrap();

        let added = store.interlace_staging().unwrap();
        assert_eq!(added, 0);
    }

    #[test]
    fn interlace_empty_staging_is_noop() {
        let (store, _dir) = make_store();
        assert_eq!(store.interlace_staging().unwrap(), 0);
    }

    #[test]
    fn interlace_is_idempotent() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(
                &[("a", "A"), ("b", "B"), ("c", "C")],
                &[("a", "b"), ("b", "c")],
            )])
            .unwrap();

        let first = store.interlace_staging().unwrap();
        let second = store.interlace_staging().unwrap();
        assert_eq!(first, 1);
        assert_eq!(second, 0, "second call must not add duplicate edges");
    }

    #[test]
    fn query_returns_columns_and_rows() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(
                &[("raw.orders", "Raw"), ("stg.orders", "Staged")],
                &[("raw.orders", "stg.orders")],
            )])
            .unwrap();
        store.apply_staging().unwrap();

        let result = store.query("SELECT COUNT(*) AS cnt FROM nodes").unwrap();
        assert_eq!(result.columns, vec!["cnt"]);
        assert_eq!(result.rows, vec![vec!["2"]]);
    }

    #[test]
    fn query_empty_store_returns_zero_rows_with_column_name() {
        let (store, _dir) = make_store();
        let result = store.query("SELECT id FROM nodes").unwrap();
        assert_eq!(result.columns, vec!["id"]);
        assert!(result.rows.is_empty());
    }

    #[test]
    fn inspect_clean_graph_has_no_issues() {
        let (store, _dir) = make_store();
        store
            .write_strands_to_staging(&[strand_with(
                &[("raw.orders", "Raw"), ("stg.orders", "Staged")],
                &[("raw.orders", "stg.orders")],
            )])
            .unwrap();
        store.apply_staging().unwrap();

        let report = store.inspect().unwrap();
        assert!(report.orphan_nodes.is_empty());
        assert!(report.external_refs.is_empty());
        assert!(!report.cycle_detected);
        assert_eq!(report.total_nodes, 2);
        assert_eq!(report.total_edges, 1);
    }
}
