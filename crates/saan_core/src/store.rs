use std::path::Path;

use duckdb::Connection;
use thiserror::Error;

use crate::graph::{Edge, Graph, Node};
use crate::strand::Strand;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] duckdb::Error),
}

pub struct Store {
    conn: Connection,
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
        let mut node_stmt = self.conn.prepare(
            "INSERT INTO staging_nodes (id, label, source_type, source_path) VALUES (?, ?, ?, ?)",
        )?;
        let mut edge_stmt = self.conn.prepare(
            "INSERT INTO staging_edges (from_id, to_id, source_path) VALUES (?, ?, ?)",
        )?;
        for strand in strands {
            let path = strand.source_path.to_string_lossy();
            for node in &strand.nodes {
                node_stmt.execute(duckdb::params![node.id, node.label, node.source_type, path])?;
            }
            for edge in &strand.edges {
                edge_stmt.execute(duckdb::params![edge.from, edge.to, path])?;
            }
        }
        Ok(())
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

    pub fn load_graph(&self) -> Result<Graph, StoreError> {
        let mut g = Graph::new();

        let mut stmt = self.conn.prepare("SELECT id, label, source_type FROM nodes")?;
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
            &[("raw.orders", "Raw Orders"), ("stg.orders", "Staged Orders")],
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

        let strands = vec![strand_with(
            &[("raw.orders", "Raw Orders")],
            &[],
        )];
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
}
