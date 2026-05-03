use std::collections::HashMap;
use std::path::Path;

use sqlparser::ast::{
    Ident, ObjectName, Query, Select, SetExpr, Statement, TableFactor, TableWithJoins,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use crate::graph::{Edge, Node};
use crate::shaver::{Shaver, ShaverError};
use crate::strand::Strand;

pub struct SqlShaver;

impl Shaver for SqlShaver {
    fn name(&self) -> &str {
        "sql"
    }

    fn extensions(&self) -> &[&str] {
        &["sql"]
    }

    fn shave(&self, input: &Path) -> Result<Strand, ShaverError> {
        let sql = std::fs::read_to_string(input).map_err(|e| ShaverError::Io {
            path: input.to_path_buf(),
            source: e,
        })?;

        let statements = Parser::parse_sql(&GenericDialect {}, &sql).map_err(|e| {
            ShaverError::Parse {
                path: input.to_path_buf(),
                message: e.to_string(),
            }
        })?;

        let mut strand = Strand::new(input.to_path_buf());
        for stmt in &statements {
            extract_statement(stmt, &mut strand);
        }
        Ok(strand)
    }
}

fn extract_statement(stmt: &Statement, strand: &mut Strand) {
    match stmt {
        Statement::CreateTable(ct) => {
            if let Some(query) = &ct.query {
                let target = object_name_to_id(&ct.name);
                add_lineage(&target, query, strand);
            }
        }
        Statement::CreateView { name, query, .. } => {
            let target = object_name_to_id(name);
            add_lineage(&target, query, strand);
        }
        Statement::Insert(insert) => {
            if let Some(source) = &insert.source {
                let target = object_name_to_id(&insert.table_name);
                add_lineage(&target, source, strand);
            }
        }
        Statement::Query(query) => {
            let cte_map = resolve_cte_sources(query);
            for src in extract_query_sources(query, &cte_map) {
                push_node(strand, &src);
            }
        }
        _ => {}
    }
}

fn add_lineage(target: &str, query: &Query, strand: &mut Strand) {
    let cte_map = resolve_cte_sources(query);
    let sources = extract_query_sources(query, &cte_map);
    push_node(strand, target);
    for src in sources {
        push_node(strand, &src);
        strand.edges.push(Edge::new(src, target));
    }
}

fn push_node(strand: &mut Strand, id: &str) {
    strand.nodes.push(Node::new(id, id, "sql"));
}

// ── CTE resolution ────────────────────────────────────────────────────────────

/// Returns a map from CTE alias (lowercased) → its real upstream table ids.
fn resolve_cte_sources(query: &Query) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let name = cte.alias.name.value.to_lowercase();
            let sources = extract_query_sources(&cte.query, &map);
            map.insert(name, sources);
        }
    }
    map
}

// ── Source extraction ─────────────────────────────────────────────────────────

fn extract_query_sources(query: &Query, cte_map: &HashMap<String, Vec<String>>) -> Vec<String> {
    extract_setexpr_sources(&query.body, cte_map)
}

fn extract_setexpr_sources(expr: &SetExpr, cte_map: &HashMap<String, Vec<String>>) -> Vec<String> {
    match expr {
        SetExpr::Select(select) => extract_select_sources(select, cte_map),
        SetExpr::SetOperation { left, right, .. } => {
            let mut out = extract_setexpr_sources(left, cte_map);
            out.extend(extract_setexpr_sources(right, cte_map));
            out
        }
        SetExpr::Query(q) => extract_query_sources(q, cte_map),
        _ => vec![],
    }
}

fn extract_select_sources(select: &Select, cte_map: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut out = vec![];
    for twj in &select.from {
        out.extend(extract_twj_sources(twj, cte_map));
    }
    out
}

fn extract_twj_sources(
    twj: &TableWithJoins,
    cte_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut out = extract_factor_sources(&twj.relation, cte_map);
    for join in &twj.joins {
        out.extend(extract_factor_sources(&join.relation, cte_map));
    }
    out
}

fn extract_factor_sources(
    factor: &TableFactor,
    cte_map: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    match factor {
        TableFactor::Table { name, .. } => {
            let id = object_name_to_id(name);
            let key = id.to_lowercase();
            cte_map.get(&key).cloned().unwrap_or_else(|| vec![id])
        }
        TableFactor::Derived { subquery, .. } => extract_query_sources(subquery, cte_map),
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => extract_twj_sources(table_with_joins, cte_map),
        _ => vec![],
    }
}

// ── Name normalisation ────────────────────────────────────────────────────────

fn object_name_to_id(name: &ObjectName) -> String {
    name.0
        .iter()
        .map(normalize_ident)
        .collect::<Vec<_>>()
        .join(".")
}

fn normalize_ident(ident: &Ident) -> String {
    if ident.quote_style.is_some() {
        ident.value.clone()
    } else {
        ident.value.to_lowercase()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::NamedTempFile;

    fn shave_sql(sql: &str) -> Strand {
        let mut f = NamedTempFile::with_suffix(".sql").unwrap();
        f.write_all(sql.as_bytes()).unwrap();
        SqlShaver.shave(f.path()).unwrap()
    }

    fn node_ids(strand: &Strand) -> Vec<String> {
        strand.nodes.iter().map(|n| n.id.clone()).collect()
    }

    fn edge_pairs(strand: &Strand) -> Vec<(String, String)> {
        strand
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect()
    }

    #[test]
    fn create_table_as_select() {
        let strand = shave_sql(
            "CREATE TABLE stg.orders AS SELECT * FROM raw.orders",
        );
        assert!(node_ids(&strand).contains(&"stg.orders".to_string()));
        assert!(node_ids(&strand).contains(&"raw.orders".to_string()));
        assert!(edge_pairs(&strand).contains(&("raw.orders".into(), "stg.orders".into())));
    }

    #[test]
    fn create_view_as_select() {
        let strand = shave_sql(
            "CREATE VIEW marts.summary AS SELECT * FROM stg.orders",
        );
        assert!(node_ids(&strand).contains(&"marts.summary".to_string()));
        assert!(edge_pairs(&strand).contains(&("stg.orders".into(), "marts.summary".into())));
    }

    #[test]
    fn insert_into_select() {
        let strand = shave_sql(
            "INSERT INTO stg.orders SELECT * FROM raw.orders",
        );
        assert!(edge_pairs(&strand).contains(&("raw.orders".into(), "stg.orders".into())));
    }

    #[test]
    fn bare_select_adds_source_nodes_only() {
        let strand = shave_sql("SELECT * FROM raw.orders");
        assert!(node_ids(&strand).contains(&"raw.orders".to_string()));
        assert!(strand.edges.is_empty());
    }

    #[test]
    fn join_both_sides_captured() {
        let strand = shave_sql(
            "CREATE TABLE stg.orders AS \
             SELECT * FROM raw.orders o JOIN raw.customers c ON o.cid = c.id",
        );
        let ids = node_ids(&strand);
        assert!(ids.contains(&"raw.orders".to_string()));
        assert!(ids.contains(&"raw.customers".to_string()));
        assert!(ids.contains(&"stg.orders".to_string()));
    }

    #[test]
    fn cte_not_exposed_as_upstream_node() {
        let strand = shave_sql(
            "CREATE TABLE marts.summary AS \
             WITH cte AS (SELECT * FROM raw.orders) \
             SELECT * FROM cte",
        );
        let ids = node_ids(&strand);
        assert!(!ids.contains(&"cte".to_string()), "CTE name must not appear as a node");
        assert!(ids.contains(&"raw.orders".to_string()));
        assert!(ids.contains(&"marts.summary".to_string()));
        assert!(strand
            .edges
            .iter()
            .any(|e| e.from == "raw.orders" && e.to == "marts.summary"));
    }

    #[test]
    fn subquery_sources_propagate() {
        let strand = shave_sql(
            "CREATE TABLE marts.summary AS \
             SELECT * FROM (SELECT * FROM raw.orders) sub",
        );
        let ids = node_ids(&strand);
        assert!(ids.contains(&"raw.orders".to_string()));
        assert!(ids.contains(&"marts.summary".to_string()));
    }

    #[test]
    fn qualified_name_preserved() {
        let strand = shave_sql(
            "CREATE TABLE prod.raw.orders AS SELECT * FROM staging.src.raw_orders",
        );
        let ids = node_ids(&strand);
        assert!(ids.contains(&"prod.raw.orders".to_string()));
        assert!(ids.contains(&"staging.src.raw_orders".to_string()));
    }

    #[test]
    fn quoted_identifier_preserves_case() {
        let strand = shave_sql(
            r#"CREATE TABLE "MySchema"."MyTable" AS SELECT * FROM raw.orders"#,
        );
        let ids = node_ids(&strand);
        assert!(
            ids.contains(&"MySchema.MyTable".to_string()),
            "got: {ids:?}"
        );
    }

    #[test]
    fn unquoted_identifier_lowercased() {
        let strand = shave_sql("CREATE TABLE STG.ORDERS AS SELECT * FROM RAW.ORDERS");
        let ids = node_ids(&strand);
        assert!(ids.contains(&"stg.orders".to_string()));
        assert!(ids.contains(&"raw.orders".to_string()));
    }

    #[test]
    fn union_both_branches_captured() {
        let strand = shave_sql(
            "CREATE TABLE stg.all_events AS \
             SELECT * FROM raw.events_a \
             UNION ALL \
             SELECT * FROM raw.events_b",
        );
        let ids = node_ids(&strand);
        assert!(ids.contains(&"raw.events_a".to_string()));
        assert!(ids.contains(&"raw.events_b".to_string()));
    }
}
