use core::fmt;
use std::{
    collections::{HashSet, VecDeque},
    fmt::Display,
};

use crate::{formula::Formula, sat::tableau::node::Node, util::join_with};

#[cfg(test)]
mod tests;

pub struct TraceBuilder {
    nodes: VecDeque<(Vec<Formula>, i32)>,
}

pub struct Trace {
    nodes: Vec<(Vec<Formula>, i32)>,
}

impl Default for TraceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: VecDeque::new(),
        }
    }

    pub fn add_node(&mut self, node: &Node) {
        let mut seen = HashSet::new();
        let formulas: Vec<_> = node
            .operands
            .iter()
            .map(|f| &f.kind)
            .filter(|f| matches!(f, Formula::Prop(_) | Formula::Not(_)))
            .filter(|&f| {
                let (negated, inner) = match f {
                    Formula::Not(inner) => (true, inner.as_ref()),
                    _ => (false, f),
                };
                match inner {
                    Formula::Prop(expr) => seen.insert((negated, &expr.kind)),
                    _ => true,
                }
            })
            .cloned()
            .collect();
        self.nodes.push_front((formulas, node.current_time));
    }

    pub fn reset(&mut self) {
        self.nodes.clear();
    }

    #[must_use]
    pub fn freeze(self) -> Trace {
        Trace::new(self)
    }
}

impl Trace {
    fn new(builder: TraceBuilder) -> Self {
        Self {
            nodes: builder.nodes.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn length(&self) -> i32 {
        self.nodes[self.nodes.len() - 1].1
    }

    #[must_use]
    pub fn eval(&self, time: i32) -> Vec<Formula> {
        assert!(time >= 0);
        for (formulas, t) in self.nodes.iter().rev() {
            if *t <= time {
                return formulas.clone();
            }
        }
        Vec::new()
    }

    #[must_use]
    pub fn full_trace(&self) -> Vec<Vec<Formula>> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let size = (self.length() as usize) + 1;
        let mut result = vec![Vec::new(); size];

        for w in self.nodes.windows(2) {
            let (formulas, t_start) = (&w[0].0, w[0].1.max(0) as usize);
            let t_end = w[1].1.max(0) as usize;
            let end_exclusive = t_end.min(size);

            if t_start < end_exclusive {
                for slot in &mut result[t_start..end_exclusive] {
                    *slot = formulas.clone();
                }
            }
        }

        if let Some((formulas, t_last)) = self.nodes.last() {
            let start = *t_last.max(&0) as usize;
            if start < size {
                for slot in &mut result[start..] {
                    *slot = formulas.clone();
                }
            }
        }

        result
    }
}

impl Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (formulas, time) in &self.nodes {
            writeln!(f, "Time {}: {}", time, join_with(formulas.as_slice(), ", "))?;
        }
        Ok(())
    }
}
