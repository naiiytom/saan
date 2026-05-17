pub mod sql;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use thiserror::Error;

use crate::strand::Strand;

#[derive(Debug, Error)]
pub enum ShaverError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{0}")]
    Other(String),
}

pub trait Shaver: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn shave(&self, input: &Path) -> Result<Strand, ShaverError>;
}

pub struct ShaverRegistry {
    shavers: HashMap<String, Arc<dyn Shaver>>,
}

impl ShaverRegistry {
    pub fn new() -> Self {
        Self {
            shavers: HashMap::new(),
        }
    }

    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        r.register(Arc::new(sql::SqlShaver::new()));
        r
    }

    pub fn with_sql_dialect(mut self, dialect: sql::SqlDialect) -> Self {
        self.register(Arc::new(sql::SqlShaver::with_dialect(dialect)));
        self
    }

    pub fn register(&mut self, shaver: Arc<dyn Shaver>) -> &mut Self {
        for ext in shaver.extensions() {
            self.shavers.insert(ext.to_string(), Arc::clone(&shaver));
        }
        self
    }

    pub fn for_extension(&self, ext: &str) -> Option<&Arc<dyn Shaver>> {
        self.shavers.get(ext)
    }

    /// Walk `input` recursively and shave every file whose extension is registered.
    pub fn shave_path(&self, input: &Path) -> Result<Vec<Strand>, ShaverError> {
        let mut strands = Vec::new();
        for entry in walkdir::WalkDir::new(input) {
            let entry = entry.map_err(|e| {
                let io_err = e
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("directory traversal error"));
                ShaverError::Io {
                    path: input.to_path_buf(),
                    source: io_err,
                }
            })?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if let Some(shaver) = self.for_extension(ext) {
                strands.push(shaver.shave(path)?);
            }
        }
        Ok(strands)
    }
}

impl Default for ShaverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    struct DummyShaver;

    impl Shaver for DummyShaver {
        fn name(&self) -> &str {
            "dummy"
        }
        fn extensions(&self) -> &[&str] {
            &["dummy"]
        }
        fn shave(&self, input: &Path) -> Result<Strand, ShaverError> {
            Ok(Strand::new(input.to_path_buf()))
        }
    }

    #[test]
    fn register_and_lookup_by_extension() {
        let mut r = ShaverRegistry::new();
        r.register(Arc::new(DummyShaver));
        assert!(r.for_extension("dummy").is_some());
        assert!(r.for_extension("sql").is_none());
    }

    #[test]
    fn with_builtins_registers_sql_extension() {
        let r = ShaverRegistry::with_builtins();
        assert!(r.for_extension("sql").is_some());
    }

    #[test]
    fn unregistered_extension_returns_none() {
        let r = ShaverRegistry::new();
        assert!(r.for_extension("py").is_none());
    }

    #[test]
    fn shave_path_empty_directory_returns_empty_vec() {
        let dir = tempdir().unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert!(strands.is_empty());
    }

    #[test]
    fn shave_path_skips_unrecognised_extensions() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "# hello").unwrap();
        std::fs::write(dir.path().join("config.yaml"), "key: val").unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert!(strands.is_empty());
    }

    #[test]
    fn shave_path_processes_matching_file_at_root() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("pipeline.sql"),
            "CREATE TABLE b AS SELECT * FROM a;",
        )
        .unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert_eq!(strands.len(), 1);
        assert_eq!(strands[0].nodes.len(), 2);
    }

    #[test]
    fn shave_path_recurses_into_subdirectories() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("models").join("staging");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("orders.sql"),
            "CREATE TABLE stg.orders AS SELECT * FROM raw.orders;",
        )
        .unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert_eq!(strands.len(), 1);
    }

    #[test]
    fn shave_path_processes_multiple_sql_files() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.sql"),
            "CREATE TABLE stg.a AS SELECT * FROM raw.a;",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.sql"),
            "CREATE TABLE stg.b AS SELECT * FROM raw.b;",
        )
        .unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert_eq!(strands.len(), 2);
    }

    #[test]
    fn shave_path_mixes_sql_and_ignored_files() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("pipeline.sql"),
            "CREATE TABLE b AS SELECT * FROM a;",
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "some notes").unwrap();
        let r = ShaverRegistry::with_builtins();
        let strands = r.shave_path(dir.path()).unwrap();
        assert_eq!(strands.len(), 1, "only the .sql file should be shaved");
    }

    #[test]
    fn with_sql_dialect_replaces_default_sql_shaver() {
        use crate::shaver::sql::SqlDialect;
        let dir = tempdir().unwrap();
        // A query using PostgreSQL :: cast syntax
        std::fs::write(
            dir.path().join("cast.sql"),
            "CREATE TABLE t AS SELECT id::text FROM src",
        )
        .unwrap();

        let postgres = ShaverRegistry::with_builtins().with_sql_dialect(SqlDialect::Postgres);
        let strands = postgres.shave_path(dir.path()).unwrap();
        assert_eq!(strands.len(), 1, "postgres dialect must parse :: cast file");
        assert!(
            strands[0].nodes.iter().any(|n| n.id == "t"),
            "postgres dialect must produce node 't' from :: cast query"
        );
        // Verify the registered shaver is still named "sql"
        assert_eq!(
            postgres.for_extension("sql").expect("sql must be registered").name(),
            "sql"
        );
    }

    #[test]
    fn shave_path_nonexistent_directory_returns_error() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does_not_exist");
        let r = ShaverRegistry::with_builtins();
        assert!(r.shave_path(&missing).is_err());
    }
}
