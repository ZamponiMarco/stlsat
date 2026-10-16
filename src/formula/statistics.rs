use std::collections::HashSet;

use crate::formula::{AExpr, ExprKind, Formula, VariableName};

#[cfg(test)]
mod tests;

impl Formula {
    #[must_use]
    pub fn temporal_operator_depth(&self) -> i32 {
        match self {
            Formula::And(ops) | Formula::Or(ops) => ops
                .iter()
                .map(super::Formula::temporal_operator_depth)
                .max()
                .unwrap_or(0),
            Formula::Not(f) => f.temporal_operator_depth(),
            Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => {
                1 + f.temporal_operator_depth()
            }
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                1 + left
                    .temporal_operator_depth()
                    .max(right.temporal_operator_depth())
            }
            Formula::Imply { left, right, .. } => left
                .temporal_operator_depth()
                .max(right.temporal_operator_depth()),
            _ => 0,
        }
    }

    #[must_use]
    pub fn depth(&self) -> i32 {
        match self {
            Formula::And(ops) | Formula::Or(ops) => {
                1 + ops.iter().map(super::Formula::depth).max().unwrap_or(0)
            }
            Formula::Not(f) | Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => {
                1 + f.depth()
            }
            Formula::U { left, right, .. }
            | Formula::R { left, right, .. }
            | Formula::Imply { left, right, .. } => 1 + left.depth().max(right.depth()),
            _ => 0,
        }
    }

    #[must_use]
    pub fn horizon(&self) -> i32 {
        match self {
            Formula::And(ops) | Formula::Or(ops) => {
                ops.iter().map(super::Formula::horizon).max().unwrap_or(0)
            }
            Formula::Not(f) => f.horizon(),
            Formula::Imply { left, right, .. } => left.horizon().max(right.horizon()),
            Formula::F { phi, interval, .. } | Formula::G { phi, interval, .. } => {
                interval.upper + phi.horizon()
            }
            Formula::U {
                left,
                right,
                interval,
                ..
            }
            | Formula::R {
                left,
                right,
                interval,
                ..
            } => interval.upper + left.horizon().max(right.horizon()),
            _ => 0,
        }
    }

    #[must_use]
    pub fn count_nodes<F>(&self, filter: F) -> i32
    where
        F: Fn(&Formula) -> bool,
    {
        let mut count = 0;
        self.inner_count_nodes(&filter, &mut count);
        count
    }

    fn inner_count_nodes<F>(&self, filter: &F, count: &mut i32)
    where
        F: Fn(&Formula) -> bool,
    {
        if filter(self) {
            *count += 1;
        }
        match self {
            Formula::And(ops) | Formula::Or(ops) => {
                for op in ops {
                    op.inner_count_nodes(filter, count);
                }
            }
            Formula::Not(f) => {
                f.inner_count_nodes(filter, count);
            }
            Formula::F { phi, .. } | Formula::G { phi, .. } => {
                phi.inner_count_nodes(filter, count);
            }
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.inner_count_nodes(filter, count);
                right.inner_count_nodes(filter, count);
            }
            Formula::Imply { left, right, .. } => {
                left.inner_count_nodes(filter, count);
                right.inner_count_nodes(filter, count);
            }
            Formula::Prop(_) => {}
        }
    }

    #[must_use]
    pub fn nodes(&self) -> i32 {
        self.count_nodes(|_| true)
    }

    #[must_use]
    pub fn boolean_variables(&self) -> i32 {
        fn inner_boolean_variables(formula: &Formula, boolean_vars: &mut HashSet<VariableName>) {
            match formula {
                Formula::And(ops) | Formula::Or(ops) => {
                    for f in ops {
                        inner_boolean_variables(f, boolean_vars);
                    }
                }
                Formula::Not(f) | Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => {
                    inner_boolean_variables(f, boolean_vars);
                }
                Formula::U { left, right, .. }
                | Formula::R { left, right, .. }
                | Formula::Imply { left, right, .. } => {
                    inner_boolean_variables(left, boolean_vars);
                    inner_boolean_variables(right, boolean_vars);
                }
                Formula::Prop(expr) => {
                    if let ExprKind::Atom(str) = &expr.kind {
                        boolean_vars.insert(str.clone());
                    }
                }
            }
        }
        let mut boolean_vars = HashSet::new();
        inner_boolean_variables(self, &mut boolean_vars);
        boolean_vars.len() as i32
    }

    #[must_use]
    pub fn real_variables(&self) -> i32 {
        fn inner_real_variables(formula: &Formula, real_vars: &mut HashSet<VariableName>) {
            match formula {
                Formula::And(ops) | Formula::Or(ops) => {
                    for f in ops {
                        inner_real_variables(f, real_vars);
                    }
                }
                Formula::Not(f) | Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => {
                    inner_real_variables(f, real_vars);
                }
                Formula::U { left, right, .. }
                | Formula::R { left, right, .. }
                | Formula::Imply { left, right, .. } => {
                    inner_real_variables(left, real_vars);
                    inner_real_variables(right, real_vars);
                }
                Formula::Prop(expr) => {
                    if let ExprKind::Rel { left, right, .. } = &expr.kind {
                        inner_real_variables_aexpr(left, real_vars);
                        inner_real_variables_aexpr(right, real_vars);
                    }
                }
            }
        }

        fn inner_real_variables_aexpr(
            aexpr: &crate::formula::AExpr,
            real_vars: &mut HashSet<VariableName>,
        ) {
            match aexpr {
                AExpr::Var(var_name, _) => {
                    real_vars.insert(var_name.clone());
                }
                AExpr::BinOp { left, right, .. } => {
                    inner_real_variables_aexpr(left, real_vars);
                    inner_real_variables_aexpr(right, real_vars);
                }
                AExpr::Abs(aexpr) => {
                    inner_real_variables_aexpr(aexpr, real_vars);
                }
                _ => {}
            }
        }

        let mut real_vars = HashSet::new();
        inner_real_variables(self, &mut real_vars);
        real_vars.len() as i32
    }

    #[must_use]
    pub fn variables(&self) -> i32 {
        self.boolean_variables() + self.real_variables()
    }

    #[must_use]
    pub fn boolean_constraints(&self) -> i32 {
        match self {
            Formula::And(ops) | Formula::Or(ops) => {
                ops.iter().map(super::Formula::boolean_constraints).sum()
            }
            Formula::Not(f) => f.boolean_constraints(),
            Formula::Imply { left, right, .. } => {
                left.boolean_constraints() + right.boolean_constraints()
            }
            Formula::F { phi, .. } | Formula::G { phi, .. } => phi.boolean_constraints(),
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.boolean_constraints() + right.boolean_constraints()
            }
            Formula::Prop(expr) => {
                if let ExprKind::Atom(_) = expr.kind {
                    1
                } else {
                    0
                }
            }
        }
    }

    #[must_use]
    pub fn real_constraints(&self) -> i32 {
        match self {
            Formula::And(ops) | Formula::Or(ops) => {
                ops.iter().map(super::Formula::real_constraints).sum()
            }
            Formula::Not(f) => f.real_constraints(),
            Formula::Imply { left, right, .. } => {
                left.real_constraints() + right.real_constraints()
            }
            Formula::F { phi, .. } | Formula::G { phi, .. } => phi.real_constraints(),
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.real_constraints() + right.real_constraints()
            }
            Formula::Prop(expr) => {
                if let ExprKind::Rel { .. } = expr.kind {
                    1
                } else {
                    0
                }
            }
        }
    }

    #[must_use]
    pub fn constraints(&self) -> i32 {
        self.boolean_constraints() + self.real_constraints()
    }

    #[must_use]
    pub fn branching_factor_avg(&self) -> f32 {
        // edges / parents
        assert!(self.is_flat());
        let edges = (self.nodes() - 1) as f32;
        let parents_count = self.count_nodes(|form| !matches!(form, Formula::Prop(_)));
        if parents_count == 0 {
            0.0
        } else {
            edges / (parents_count as f32)
        }
    }

    #[must_use]
    pub fn disjunction_max_width(&self) -> i32 {
        assert!(self.is_flat());
        match self {
            Formula::Or(ops) => {
                let inner = ops
                    .iter()
                    .map(super::Formula::disjunction_max_width)
                    .max()
                    .unwrap_or(0);
                (ops.len() as i32).max(inner)
            }
            Formula::And(ops) => ops
                .iter()
                .map(super::Formula::disjunction_max_width)
                .max()
                .unwrap_or(0),
            Formula::Not(f) => f.disjunction_max_width(),
            Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => f.disjunction_max_width(),
            Formula::U { left, right, .. }
            | Formula::R { left, right, .. }
            | Formula::Imply { left, right, .. } => left
                .disjunction_max_width()
                .max(right.disjunction_max_width()),
            _ => 0,
        }
    }

    #[must_use]
    pub fn disjunction_total_width(&self) -> i32 {
        assert!(self.is_flat());
        match self {
            Formula::Or(ops) => {
                let inner_sum: i32 = ops
                    .iter()
                    .map(super::Formula::disjunction_total_width)
                    .sum();
                (ops.len() as i32) + inner_sum
            }
            Formula::And(ops) => ops
                .iter()
                .map(super::Formula::disjunction_total_width)
                .sum(),
            Formula::Not(f) => f.disjunction_total_width(),
            Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => f.disjunction_total_width(),
            Formula::U { left, right, .. }
            | Formula::R { left, right, .. }
            | Formula::Imply { left, right, .. } => {
                left.disjunction_total_width() + right.disjunction_total_width()
            }
            _ => 0,
        }
    }

    #[must_use]
    pub fn combinatorial_branching_count(&self) -> i64 {
        assert!(self.is_flat());
        match self {
            Formula::Or(ops) => ops
                .iter()
                .map(super::Formula::combinatorial_branching_count)
                .sum(),
            Formula::And(ops) => ops
                .iter()
                .map(super::Formula::combinatorial_branching_count)
                .product(),
            Formula::Not(f) | Formula::F { phi: f, .. } | Formula::G { phi: f, .. } => {
                f.combinatorial_branching_count()
            }
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.combinatorial_branching_count() * right.combinatorial_branching_count()
            }
            Formula::Imply {
                not_left, right, ..
            } => not_left.combinatorial_branching_count() + right.combinatorial_branching_count(),
            _ => 1,
        }
    }
}
