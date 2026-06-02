use std::collections::BTreeMap;

use z3::Solver;
use z3::ast::{Bool, Real};

use crate::formula::parser::parse_formula;
use crate::formula::{AExpr, ArithOp, Expr, ExprKind, Formula, RelOp, VariableName};
use crate::sat::config::GeneralOptions;

#[cfg(test)]
mod tests;

pub struct SmtSolver {
    pub options: GeneralOptions,
    bool_variables: BTreeMap<VariableName, Vec<Bool>>,
    real_variables: BTreeMap<VariableName, Vec<Real>>,
}

impl SmtSolver {
    #[must_use]
    pub fn new(general: GeneralOptions) -> Self {
        SmtSolver {
            options: general,
            bool_variables: BTreeMap::new(),
            real_variables: BTreeMap::new(),
        }
    }

    pub fn make_smt_from_str(&mut self, formula: &str) -> Option<bool> {
        // Parsing Stage
        let parsed = parse_formula(formula);
        let formula = match parsed {
            Ok((remaining, formula_ast)) => {
                if !remaining.trim().is_empty() {
                    panic!(
                        "Unparsed input remaining after parsing formula: {}",
                        remaining
                    );
                }
                formula_ast
            }
            Err(e) => {
                panic!("Failed to parse formula, parse error: {}", e);
            }
        };
        self.make_smt_from_formula(formula)
    }

    pub fn make_smt_from_formula(&mut self, formula: Formula) -> Option<bool> {
        let solver = Solver::new_for_logic("QF_LRA").unwrap_or_default();

        let time_horizon = formula.horizon() as usize;
        let smt_formula = self.encode_formula(formula, 0, time_horizon);

        solver.assert(smt_formula);

        match solver.check() {
            z3::SatResult::Sat => Some(true),
            z3::SatResult::Unsat => Some(false),
            z3::SatResult::Unknown => None,
        }
    }

    fn encode_formula(&mut self, formula: Formula, time: i32, time_horizon: usize) -> Bool {
        match formula {
            Formula::And(ops) => {
                let bools: Vec<Bool> = ops
                    .into_iter()
                    .map(|op| self.encode_formula(op, time, time_horizon))
                    .collect();
                Bool::and(&bools)
            }
            Formula::Or(ops) => {
                let bools: Vec<Bool> = ops
                    .into_iter()
                    .map(|op| self.encode_formula(op, time, time_horizon))
                    .collect();
                Bool::or(&bools)
            }
            Formula::Not(op) => {
                let b = self.encode_formula(*op, time, time_horizon);
                b.not()
            }
            Formula::Imply { left, right, .. } => {
                let left_b = self.encode_formula(*left, time, time_horizon);
                let right_b = self.encode_formula(*right, time, time_horizon);
                left_b.implies(right_b)
            }
            Formula::G { interval, phi, .. } => {
                let parts: Vec<Bool> = (time + interval.lower..=time + interval.upper)
                    .map(|i| self.encode_formula(*phi.clone(), i, time_horizon))
                    .collect();
                Bool::and(&parts)
            }
            Formula::F { interval, phi, .. } => {
                let parts: Vec<Bool> = (time + interval.lower..=time + interval.upper)
                    .map(|i| self.encode_formula(*phi.clone(), i, time_horizon))
                    .collect();
                Bool::or(&parts)
            }
            Formula::U {
                interval,
                left,
                right,
                ..
            } => {
                let witnesses: Vec<Bool> = (time + interval.lower..=time + interval.upper)
                    .map(|i| {
                        let right_at_i = self.encode_formula(*right.clone(), i, time_horizon);

                        let left_range = if self.options.mltl {
                            (time + interval.lower)..i // [time + a, i)
                        } else {
                            time..(i + 1) // [time, i]
                        };

                        let left_until_i: Vec<Bool> = left_range
                            .map(|j| self.encode_formula(*left.clone(), j, time_horizon))
                            .collect();

                        Bool::and(&[right_at_i, Bool::and(&left_until_i)])
                    })
                    .collect();

                Bool::or(&witnesses)
            }
            Formula::R {
                interval,
                left,
                right,
                ..
            } => {
                let obligations: Vec<Bool> = (time + interval.lower..=time + interval.upper)
                    .map(|i| {
                        let right_at_i = self.encode_formula(*right.clone(), i, time_horizon);

                        let left_range = if self.options.mltl {
                            (time + interval.lower)..i // [time + a, i)
                        } else {
                            time..(i + 1) // [time, i]
                        };

                        let released_before_i: Vec<Bool> = left_range
                            .map(|j| self.encode_formula(*left.clone(), j, time_horizon))
                            .collect();

                        Bool::or(&[right_at_i, Bool::or(&released_before_i)])
                    })
                    .collect();

                Bool::and(&obligations)
            }
            Formula::Prop(expr) => self.encode_expr(expr, time, time_horizon),
        }
    }

    fn encode_expr(&mut self, expr: Expr, time: i32, time_horizon: usize) -> Bool {
        match expr.kind {
            ExprKind::False => Bool::from_bool(false),
            ExprKind::True => Bool::from_bool(true),
            ExprKind::Atom(n) => {
                let array = if let Some(v) = self.bool_variables.get(&n) {
                    v
                } else {
                    self.bool_variables.insert(
                        n.clone(),
                        (0..=time_horizon)
                            .map(|t| Bool::new_const(format!("{n}_{t}")))
                            .collect(),
                    );
                    self.bool_variables.get(&n).unwrap()
                };
                array[time as usize].clone()
            }
            ExprKind::Rel { op, left, right } => {
                self.encode_rel(op, left, right, time, time_horizon)
            }
        }
    }

    fn encode_aexpr(&mut self, expr: AExpr, time: i32, time_horizon: usize) -> Real {
        match expr {
            AExpr::Var(n) => {
                let array = if let Some(v) = self.real_variables.get(&n) {
                    v
                } else {
                    self.real_variables.insert(
                        n.clone(),
                        (0..=time_horizon)
                            .map(|t| Real::new_const(format!("{n}_{t}")))
                            .collect(),
                    );
                    self.real_variables.get(&n).unwrap()
                };
                array[time as usize].clone()
            }
            AExpr::Num(r) => Real::from_rational(*r.numer(), *r.denom()),
            AExpr::Abs(inner) => {
                let x = self.encode_aexpr(*inner, time, time_horizon);
                let zero = Real::from_rational(0, 1);
                let cond = x.ge(&zero);
                let neg_x = &zero - &x;
                Bool::ite(&cond, &x, &neg_x)
            }
            AExpr::BinOp { op, left, right } => {
                let l = self.encode_aexpr(*left, time, time_horizon);
                let r = self.encode_aexpr(*right, time, time_horizon);
                match op {
                    ArithOp::Add => &l + &r,
                    ArithOp::Sub => &l - &r,
                }
            }
        }
    }

    fn encode_rel(
        &mut self,
        op: RelOp,
        left: AExpr,
        right: AExpr,
        time: i32,
        time_horizon: usize,
    ) -> Bool {
        let l = self.encode_aexpr(left, time, time_horizon);
        let r = self.encode_aexpr(right, time, time_horizon);

        match op {
            RelOp::Lt => l.lt(&r),
            RelOp::Le => l.le(&r),
            RelOp::Gt => l.gt(&r),
            RelOp::Ge => l.ge(&r),
            RelOp::Eq => l.eq(&r),
            RelOp::Ne => l.ne(&r),
        }
    }
}
