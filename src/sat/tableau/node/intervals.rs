use std::collections::{HashMap, HashSet};

use crate::formula::{AExpr, Expr, ExprKind, Formula, Interval, VariableName};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PropositionValidityInterval {
    pub expr: Expr,
    pub negated: bool,
    pub interval: Interval,
}

#[derive(Default)]
pub struct ConflictCache {
    variables: HashMap<usize, HashSet<VariableName>>,
}

impl PropositionValidityInterval {
    #[must_use]
    pub fn conflicts_with(&self, other: &Self, cache: &mut ConflictCache) -> bool {
        let can_conflict = match (&self.expr.kind, &other.expr.kind) {
            (ExprKind::Atom(left), ExprKind::Atom(right)) => {
                left == right && self.negated != other.negated
            }
            (ExprKind::Rel { .. }, ExprKind::Rel { .. }) => {
                cache.shares_variable(&self.expr, &other.expr)
            }
            _ => false,
        };

        can_conflict && self.interval.intersects(&other.interval)
    }
}

impl ConflictCache {
    fn ensure_variables(&mut self, expr: &Expr) {
        self.variables.entry(expr.id).or_insert_with(|| {
            let mut variables = HashSet::new();

            fn collect(expr: &AExpr, variables: &mut HashSet<VariableName>) {
                match expr {
                    AExpr::Var(name) => {
                        variables.insert(name.clone());
                    }
                    AExpr::Num(_) => {}
                    AExpr::Abs(inner) => collect(inner, variables),
                    AExpr::BinOp { left, right, .. } => {
                        collect(left, variables);
                        collect(right, variables);
                    }
                }
            }

            if let ExprKind::Rel { left, right, .. } = &expr.kind {
                collect(left, &mut variables);
                collect(right, &mut variables);
            }

            variables
        });
    }

    fn shares_variable(&mut self, left: &Expr, right: &Expr) -> bool {
        self.ensure_variables(left);
        self.ensure_variables(right);

        let left_variables = self.variables.get(&left.id).unwrap();
        let right_variables = self.variables.get(&right.id).unwrap();
        left_variables
            .intersection(right_variables)
            .next()
            .is_some()
    }
}

impl Formula {
    pub fn proposition_full_interval(
        &self,
        interval: Interval,
    ) -> HashSet<PropositionValidityInterval> {
        fn inner_full(
            formula: &Formula,
            delta: Interval,
            set: &mut HashSet<PropositionValidityInterval>,
        ) {
            match formula {
                Formula::Prop(e) => {
                    set.insert(PropositionValidityInterval {
                        expr: e.clone(),
                        negated: false,
                        interval: delta,
                    });
                }
                Formula::Not(inner) => {
                    let Formula::Prop(e) = &**inner else {
                        panic!("Not operator should only be applied to propositions");
                    };
                    set.insert(PropositionValidityInterval {
                        expr: e.clone(),
                        negated: true,
                        interval: delta,
                    });
                }
                Formula::Or(operands) | Formula::And(operands) => {
                    for op in operands {
                        inner_full(op, delta.clone(), set);
                    }
                }
                Formula::Imply {
                    right, not_left, ..
                } => {
                    inner_full(not_left, delta.clone(), set);
                    inner_full(right, delta, set);
                }
                Formula::U {
                    left,
                    right,
                    interval,
                }
                | Formula::R {
                    interval,
                    left,
                    right,
                } => {
                    // When point interval, only the right operand appears
                    if interval.lower < interval.upper {
                        inner_full(
                            left,
                            Interval {
                                lower: delta.lower + interval.lower,
                                upper: delta.upper + interval.upper - 1,
                            },
                            set,
                        );
                    }
                    inner_full(
                        right,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
                Formula::G { interval, phi } | Formula::F { interval, phi } => {
                    inner_full(
                        phi,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
            }
        }
        let mut set = HashSet::new();
        inner_full(self, interval, &mut set);
        set
    }
}
