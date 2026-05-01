use std::collections::HashMap;

use anyhow::{Result, bail};
use petgraph::algo::tarjan_scc;
use petgraph::graph::DiGraph;
use petgraph::prelude::*;

/// Directed dependency graph for `#include` relationships.
///
/// Edge direction: `includer -> includee` (the includer depends on the includee).
/// Expansion order is reverse topological order — dependencies come first.
pub struct DependencyGraph {
    graph: DiGraph<String, ()>,
    indices: HashMap<String, NodeIndex>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            indices: HashMap::new(),
        }
    }

    /// Ensure a file path exists as a node in the graph, returning its index.
    pub fn add_file(&mut self, path: &str) -> NodeIndex {
        if let Some(&idx) = self.indices.get(path) {
            return idx;
        }
        let idx = self.graph.add_node(path.to_string());
        self.indices.insert(path.to_string(), idx);
        idx
    }

    /// Record that `from` depends on `to` (i.e., `from` includes `to`).
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        let from_idx = self.add_file(from);
        let to_idx = self.add_file(to);
        self.graph.add_edge(from_idx, to_idx, ());
    }

    /// Check for cycles. Returns the first cycle path found, if any.
    pub fn detect_cycle(&self) -> Option<Vec<String>> {
        let sccs = tarjan_scc(&self.graph);
        for scc in &sccs {
            if scc.len() > 1 || (scc.len() == 1 && self.has_self_loop(scc[0])) {
                return Some(scc.iter().map(|&n| self.graph[n].clone()).collect());
            }
        }
        None
    }

    /// Return files in expansion order (dependencies first, dependents last).
    ///
    /// Returns an error if the graph contains a cycle.
    pub fn expansion_order(&self) -> Result<Vec<String>> {
        if let Some(cycle) = self.detect_cycle() {
            bail!("circular dependency detected: {}", cycle.join(" -> "));
        }

        // petgraph::algo::toposort puts sources before sinks.
        // We want sinks (leaf dependencies) first, so reverse the result.
        let mut order: Vec<String> = petgraph::algo::toposort(&self.graph, None)
            .expect("cycle should have been caught above")
            .into_iter()
            .map(|n| self.graph[n].clone())
            .collect();
        order.reverse();
        Ok(order)
    }

    fn has_self_loop(&self, node: NodeIndex) -> bool {
        self.graph.neighbors(node).any(|n| n == node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_chain() {
        let mut g = DependencyGraph::new();
        g.add_dependency("main.cpp", "header.h");
        let order = g.expansion_order().unwrap();
        assert_eq!(order, vec!["header.h", "main.cpp"]);
    }

    #[test]
    fn test_transitive() {
        let mut g = DependencyGraph::new();
        g.add_dependency("main.cpp", "a.h");
        g.add_dependency("a.h", "b.h");
        let order = g.expansion_order().unwrap();
        assert_eq!(order, vec!["b.h", "a.h", "main.cpp"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = DependencyGraph::new();
        g.add_dependency("a.h", "b.h");
        g.add_dependency("b.h", "a.h");
        assert!(g.detect_cycle().is_some());
        assert!(g.expansion_order().is_err());
    }

    #[test]
    fn test_no_deps() {
        let mut g = DependencyGraph::new();
        g.add_file("standalone.cpp");
        let order = g.expansion_order().unwrap();
        assert_eq!(order, vec!["standalone.cpp"]);
    }
}
