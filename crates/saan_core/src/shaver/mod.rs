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
