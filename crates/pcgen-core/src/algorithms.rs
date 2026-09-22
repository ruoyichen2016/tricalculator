//! Graph algorithms used by the rules engine.
//!
//! # CLRS alignment
//!
//! Only the graph routines that the domain genuinely needs are kept here:
//!
//! | Operation | CLRS reference | Implementation |
//! | --- | --- | --- |
//! | Topological sort | 22.4 (DFS topological sort; Kahn variant) | stable Kahn with an ordered ready set |
//! | Strongly connected components | 22.5 (Kosaraju) | two-pass DFS with transpose |
//!
//! Sorting, searching, priority queues, and text algorithms use the Rust
//! standard library.  No CLRS routine is implemented unless a production
//! call site exists.
//!
//! ## Complexity
//!
//! Let `V` be the number of vertices and `E` the number of adjacency
//! entries.
//!
//! * [`topological_sort`]: `O(V log V + E)` because the ready set is
//!   ordered; this buys reproducible tie-breaking, which matters for
//!   rule/data ordering.  Without the ordering it would be `O(V + E)`.
//! * [`strongly_connected_components`]: `Θ(V + E)` time and space, matching
//!   CLRS 22.5.
//!
//! ## Invariants
//!
//! * Topological sort: before a node is emitted, all of its predecessors
//!   have already been emitted.
//! * SCC: the second DFS runs on the transpose in decreasing finish time,
//!   so every depth-first tree is exactly one strongly connected component.
//!
//! Both functions ignore adjacency entries outside `0..V` rather than
//! panicking; data-driven graphs may contain references to optional or
//! filtered entities.

use std::collections::BTreeSet;

/// Stable topological sort.
///
/// This is the Kahn formulation of CLRS topological sort (CLRS 22.4 also
/// presents the DFS formulation).  Ready nodes are emitted in ascending
/// index order, which makes the output reproducible across platforms and
/// hash seeds.
///
/// Returns `None` when the graph contains a directed cycle.
pub fn topological_sort(adjacency: &[Vec<usize>]) -> Option<Vec<usize>> {
    let vertex_count = adjacency.len();
    let mut indegree = vec![0usize; vertex_count];
    for edges in adjacency {
        for &target in edges {
            if target < vertex_count {
                indegree[target] += 1;
            }
        }
    }

    // CLRS 22.4 uses a set of vertices with no incoming edges; the ordered
    // ready set additionally gives stable tie-breaking.
    let mut ready: BTreeSet<usize> = (0..vertex_count)
        .filter(|&vertex| indegree[vertex] == 0)
        .collect();
    let mut order = Vec::with_capacity(vertex_count);

    while let Some(vertex) = ready.pop_first() {
        order.push(vertex);
        for &target in &adjacency[vertex] {
            if target >= vertex_count {
                continue;
            }
            indegree[target] = indegree[target].saturating_sub(1);
            if indegree[target] == 0 {
                ready.insert(target);
            }
        }
    }

    (order.len() == vertex_count).then_some(order)
}

/// Strongly connected components via Kosaraju's algorithm (CLRS 22.5).
///
/// Pass 1 computes finish times on `G`; pass 2 explores the transpose in
/// decreasing finish order.  The result is deterministic: each component's
/// vertices are sorted ascending, and components are ordered by their
/// smallest vertex.
pub fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let vertex_count = adjacency.len();
    let transpose = transpose_graph(adjacency);

    // Pass 1: depth-first search on G, recording finish order.
    let mut visited = vec![false; vertex_count];
    let mut finish_order = Vec::with_capacity(vertex_count);
    for vertex in 0..vertex_count {
        if !visited[vertex] {
            dfs_finish(vertex, adjacency, &mut visited, &mut finish_order);
        }
    }

    // Pass 2: explore transpose in decreasing finish time.  Each DFS tree is
    // one SCC; this is the CLRS 22.5 correctness property.
    let mut assigned = vec![false; vertex_count];
    let mut components = Vec::new();
    for &vertex in finish_order.iter().rev() {
        if !assigned[vertex] {
            let mut component = Vec::new();
            dfs_collect(vertex, &transpose, &mut assigned, &mut component);
            component.sort_unstable();
            components.push(component);
        }
    }

    components.sort_by_key(|component| component.first().copied().unwrap_or(usize::MAX));
    components
}

fn transpose_graph(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let vertex_count = adjacency.len();
    let mut transpose = vec![Vec::new(); vertex_count];
    for (source, edges) in adjacency.iter().enumerate() {
        for &target in edges {
            if target < vertex_count {
                transpose[target].push(source);
            }
        }
    }
    transpose
}

fn dfs_finish(
    vertex: usize,
    adjacency: &[Vec<usize>],
    visited: &mut [bool],
    finish_order: &mut Vec<usize>,
) {
    visited[vertex] = true;
    for &target in &adjacency[vertex] {
        if target < visited.len() && !visited[target] {
            dfs_finish(target, adjacency, visited, finish_order);
        }
    }
    finish_order.push(vertex);
}

fn dfs_collect(
    vertex: usize,
    transpose: &[Vec<usize>],
    assigned: &mut [bool],
    component: &mut Vec<usize>,
) {
    assigned[vertex] = true;
    component.push(vertex);
    for &target in &transpose[vertex] {
        if target < assigned.len() && !assigned[target] {
            dfs_collect(target, transpose, assigned, component);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topological_sort_is_stable() {
        let graph = vec![vec![2], vec![2], vec![]];
        assert_eq!(topological_sort(&graph), Some(vec![0, 1, 2]));
    }

    #[test]
    fn topological_sort_rejects_cycles() {
        let graph = vec![vec![1], vec![0]];
        assert_eq!(topological_sort(&graph), None);
    }

    #[test]
    fn topological_sort_ignores_out_of_range_edges() {
        let graph = vec![vec![1, 99], vec![]];
        assert_eq!(topological_sort(&graph), Some(vec![0, 1]));
    }

    #[test]
    fn scc_finds_cycles_and_singletons() {
        // 0 -> 1 -> 2 -> 0, 3 singleton, 2 -> 3
        let graph = vec![vec![1], vec![2], vec![3, 0], vec![]];
        let components = strongly_connected_components(&graph);
        assert_eq!(components, vec![vec![0, 1, 2], vec![3]]);
    }

    #[test]
    fn scc_of_dag_is_singletons_in_ascending_order() {
        let graph = vec![vec![1], vec![2], vec![]];
        let components = strongly_connected_components(&graph);
        assert_eq!(components, vec![vec![0], vec![1], vec![2]]);
    }
}
