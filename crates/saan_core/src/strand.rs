use std::path::PathBuf;

use crate::graph::{Edge, Node};

pub struct Strand {
    pub source_path: PathBuf,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Strand {
    pub fn new(source_path: PathBuf) -> Self {
        Self {
            source_path,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}
