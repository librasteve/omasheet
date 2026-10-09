// Copyright (c) 2026 Stephen Roe

//! Dependency analysis: calculation order and cycle detection.
//!
//! Nodes are constants and columns. An edge `a -> b` means "a reads b", with a
//! [`DepKind`] saying which rows of `b` a row of `a` reads. A cycle is only a
//! real circular reference if it can lead a cell back to itself: a running
//! balance that reads its own previous row is fine.

use crate::types::DepKind;
use std::collections::VecDeque;

/// One strongly connected group of nodes, in an order that is safe to
/// calculate: every group comes after the groups it reads.
#[derive(Debug)]
pub struct Scc {
    pub nodes: Vec<usize>,
    /// The group reads later rows, so rows must be calculated last to first.
    pub reverse: bool,
    /// A circular reference, as the list of nodes around the cycle.
    pub cycle: Option<Vec<usize>>,
}

pub fn analyse(edges: &[Vec<(usize, DepKind)>]) -> Vec<Scc> {
    let n = edges.len();
    let mut t = Tarjan {
        edges,
        index: vec![usize::MAX; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if t.index[v] == usize::MAX {
            t.visit(v);
        }
    }
    t.out
        .into_iter()
        .map(|mut nodes| {
            nodes.sort_unstable();
            let internal: Vec<(usize, usize, DepKind)> = nodes
                .iter()
                .flat_map(|&u| {
                    let nodes = &nodes;
                    edges[u]
                        .iter()
                        .filter(move |(v, _)| nodes.contains(v))
                        .map(move |&(v, k)| (u, v, k))
                })
                .collect();
            let has = |kind: DepKind| internal.iter().find(|e| e.2 == kind).copied();
            let cycle = if let Some((u, v, _)) = has(DepKind::Whole) {
                Some(close(&internal, u, v, |_| true))
            } else if let (Some(_), Some((u, v, _))) = (has(DepKind::Back), has(DepKind::Fwd)) {
                Some(close(&internal, u, v, |_| true))
            } else {
                // Only a cycle made entirely of same-row reads is circular.
                internal
                    .iter()
                    .filter(|e| e.2 == DepKind::Same)
                    .find_map(|&(u, v, _)| {
                        let path = close(&internal, u, v, |k| k == DepKind::Same);
                        (!path.is_empty()).then_some(path)
                    })
            };
            Scc {
                reverse: has(DepKind::Fwd).is_some(),
                cycle: cycle.filter(|c| !c.is_empty()),
                nodes,
            }
        })
        .collect()
}

/// The cycle formed by the edge `u -> v` and the shortest path `v ~> u` over
/// edges whose kind passes `allow`. Empty if there is no such path.
fn close(
    edges: &[(usize, usize, DepKind)],
    u: usize,
    v: usize,
    allow: impl Fn(DepKind) -> bool,
) -> Vec<usize> {
    if u == v {
        return vec![u];
    }
    let mut prev: Vec<(usize, usize)> = vec![(v, v)];
    let mut queue = VecDeque::from([v]);
    while let Some(x) = queue.pop_front() {
        if x == u {
            let mut path = vec![u];
            let mut at = u;
            while at != v {
                at = prev.iter().find(|p| p.0 == at).unwrap().1;
                path.push(at);
            }
            path.reverse();
            // path is v .. u; the cycle reads u -> v -> .. -> u
            let mut cycle = vec![u];
            cycle.extend(&path[..path.len() - 1]);
            return cycle;
        }
        for &(a, b, k) in edges {
            if a == x && allow(k) && !prev.iter().any(|p| p.0 == b) {
                prev.push((b, x));
                queue.push_back(b);
            }
        }
    }
    Vec::new()
}

struct Tarjan<'a> {
    edges: &'a [Vec<(usize, DepKind)>],
    index: Vec<usize>,
    low: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next: usize,
    out: Vec<Vec<usize>>,
}

impl Tarjan<'_> {
    fn visit(&mut self, v: usize) {
        self.index[v] = self.next;
        self.low[v] = self.next;
        self.next += 1;
        self.stack.push(v);
        self.on_stack[v] = true;
        for i in 0..self.edges[v].len() {
            let w = self.edges[v][i].0;
            if self.index[w] == usize::MAX {
                self.visit(w);
                self.low[v] = self.low[v].min(self.low[w]);
            } else if self.on_stack[w] {
                self.low[v] = self.low[v].min(self.index[w]);
            }
        }
        if self.low[v] == self.index[v] {
            let mut scc = Vec::new();
            loop {
                let w = self.stack.pop().unwrap();
                self.on_stack[w] = false;
                scc.push(w);
                if w == v {
                    break;
                }
            }
            self.out.push(scc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DepKind::*;

    #[test]
    fn order_puts_dependencies_first() {
        // 0 reads 1, 1 reads 2
        let sccs = analyse(&[vec![(1, Same)], vec![(2, Same)], vec![]]);
        let order: Vec<_> = sccs.iter().map(|s| s.nodes[0]).collect();
        assert_eq!(order, [2, 1, 0]);
        assert!(sccs.iter().all(|s| s.cycle.is_none()));
    }

    #[test]
    fn same_row_cycle_is_circular() {
        let sccs = analyse(&[vec![(1, Same)], vec![(0, Same)]]);
        assert_eq!(sccs[0].cycle.as_deref(), Some(&[0, 1][..]));
    }

    #[test]
    fn previous_row_self_reference_is_fine() {
        let sccs = analyse(&[vec![(0, Back), (1, Same)], vec![]]);
        assert!(sccs.iter().all(|s| s.cycle.is_none()));
    }

    #[test]
    fn whole_column_self_reference_is_circular() {
        let sccs = analyse(&[vec![(0, Whole)]]);
        assert_eq!(sccs[0].cycle.as_deref(), Some(&[0][..]));
    }
}
