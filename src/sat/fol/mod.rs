use std::collections::BTreeMap;

use z3::ast::{Bool, Int, Real, exists_const, forall_const};
use z3::{FuncDecl, Solver, Sort};

use crate::formula::parser::parse_formula;
use crate::formula::{AExpr, ArithOp, Expr, ExprKind, Formula, RelOp, VariableName};
use crate::sat::config::GeneralOptions;

#[cfg(test)]
mod tests;

pub struct FolSolver {
    pub options: GeneralOptions,
    bool_variables: BTreeMap<VariableName, FuncDecl>,
    real_variables: BTreeMap<VariableName, FuncDecl>,
}

impl FolSolver {
    #[must_use]
    pub fn new(general: GeneralOptions) -> Self {
        FolSolver {
            options: general,
            bool_variables: BTreeMap::new(),
            real_variables: BTreeMap::new(),
        }
    }

    pub fn make_fol_from_str(&mut self, formula: &str) -> Option<bool> {
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
        self.make_fol_from_formula(formula)
    }

    pub fn make_fol_from_formula(&mut self, formula: Formula) -> Option<bool> {
        let solver = Solver::new_for_logic("LRA").unwrap_or_default();
        let smt_formula = self.encode_formula(formula, &Int::from_i64(0));
        solver.assert(smt_formula);

        match solver.check() {
            z3::SatResult::Sat => Some(true),
            z3::SatResult::Unsat => Some(false),
            z3::SatResult::Unknown => None,
        }
    }

    fn encode_formula(&mut self, formula: Formula, time: &Int) -> Bool {
        match formula {
            Formula::And(ops) => {
                let bools: Vec<Bool> = ops
                    .into_iter()
                    .map(|op| self.encode_formula(op, time))
                    .collect();
                Bool::and(&bools)
            }
            Formula::Or(ops) => {
                let bools: Vec<Bool> = ops
                    .into_iter()
                    .map(|op| self.encode_formula(op, time))
                    .collect();
                Bool::or(&bools)
            }
            Formula::Not(op) => {
                let b = self.encode_formula(*op, time);
                b.not()
            }
            Formula::Imply { left, right, .. } => {
                let left_b = self.encode_formula(*left, time);
                let right_b = self.encode_formula(*right, time);
                left_b.implies(right_b)
            }
            Formula::G { interval, phi, .. } => {
                let i = Int::fresh_const("G_i");
                let lower = time + interval.lower;
                let upper = time + interval.upper;
                let range = Bool::and(&[&lower.le(&i), &i.le(&upper)]);
                let sub_bool = self.encode_formula(*phi, &i);
                forall_const(&[&i], &[], &range.implies(sub_bool))
            }
            Formula::F { interval, phi, .. } => {
                let i = Int::fresh_const("F_i");
                let lower = time + interval.lower;
                let upper = time + interval.upper;
                let sub_bool = self.encode_formula(*phi, &i);
                let constraint = Bool::and(&[&lower.le(&i), &i.le(&upper), &sub_bool]);
                exists_const(&[&i], &[], &constraint)
            }
            Formula::U {
                interval,
                left,
                right,
                ..
            } => {
                let (i, j) = (Int::fresh_const("U_i"), Int::fresh_const("U_j"));
                let forall_range = if self.options.mltl {
                    Bool::and(&[(time + interval.lower).le(&j), j.lt(&i)])
                } else {
                    Bool::and(&[time.le(&j), j.le(&i)])
                };
                let left_b = self.encode_formula(*left, &j);
                let right_b = self.encode_formula(*right, &i);
                exists_const(
                    &[&i],
                    &[],
                    &Bool::and(&[
                        (time + interval.lower).le(&i),
                        i.le(time + interval.upper),
                        right_b,
                        forall_const(&[&j], &[], &forall_range.implies(&left_b)),
                    ]),
                )
            }
            Formula::R {
                interval,
                left,
                right,
                ..
            } => {
                let (i, j) = (Int::fresh_const("R_i"), Int::fresh_const("R_j"));
                let exists_range = if self.options.mltl {
                    Bool::and(&[(time + interval.lower).le(&i), i.lt(&j)])
                } else {
                    Bool::and(&[time.le(&i), i.le(&j)])
                };
                let forall_range =
                    Bool::and(&[(time + interval.lower).le(&j), j.le(time + interval.upper)]);
                let left_b = self.encode_formula(*left, &i);
                let right_b = self.encode_formula(*right, &j);
                forall_const(
                    &[&j],
                    &[],
                    &forall_range.implies(Bool::or(&[
                        right_b,
                        exists_const(&[&i], &[], &Bool::and(&[exists_range, left_b])),
                    ])),
                )
            }
            Formula::Prop(expr) => self.encode_expr(expr, time),
        }
    }

    fn encode_expr(&mut self, expr: Expr, time: &Int) -> Bool {
        match expr.kind {
            ExprKind::False => Bool::from_bool(false),
            ExprKind::True => Bool::from_bool(true),
            ExprKind::Atom(n) => {
                let func = if let Some(v) = self.bool_variables.get(&n) {
                    v
                } else {
                    let func = FuncDecl::new(n.to_string(), &[&Sort::int()], &Sort::bool());
                    self.bool_variables.insert(n.clone(), func);
                    self.bool_variables.get(&n).unwrap()
                };
                func.apply(&[time]).try_into().unwrap()
            }
            ExprKind::Rel { op, left, right } => self.encode_rel(op, left, right, time),
        }
    }

    fn encode_aexpr(&mut self, expr: AExpr, time: &Int) -> Real {
        match expr {
            AExpr::Var(n) => {
                let func = if let Some(v) = self.real_variables.get(&n) {
                    v
                } else {
                    let func = FuncDecl::new(n.to_string(), &[&Sort::int()], &Sort::real());
                    self.real_variables.insert(n.clone(), func);
                    self.real_variables.get(&n).unwrap()
                };
                func.apply(&[time]).try_into().unwrap()
            }
            AExpr::Num(r) => Real::from_rational(*r.numer(), *r.denom()),
            AExpr::Abs(inner) => {
                let x = self.encode_aexpr(*inner, time);
                let zero = Real::from_rational(0, 1);
                let cond = x.ge(&zero);
                let neg_x = &zero - &x;
                Bool::ite(&cond, &x, &neg_x)
            }
            AExpr::BinOp { op, left, right } => {
                let l = self.encode_aexpr(*left, time);
                let r = self.encode_aexpr(*right, time);
                match op {
                    ArithOp::Add => &l + &r,
                    ArithOp::Sub => &l - &r,
                }
            }
        }
    }

    fn encode_rel(&mut self, op: RelOp, left: AExpr, right: AExpr, time: &Int) -> Bool {
        let l = self.encode_aexpr(left, time);
        let r = self.encode_aexpr(right, time);

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
