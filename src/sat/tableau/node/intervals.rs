use std::collections::HashSet;

use crate::formula::{Expr, Formula, Interval};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PropositionValidityInterval {
    pub expr: Expr,
    pub interval: Interval,
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
                        interval: delta,
                    });
                }
                Formula::Not(inner) => {
                    inner_full(inner, delta, set);
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
                    inner_full(
                        left,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
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
