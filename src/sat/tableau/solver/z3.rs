use crate::formula::{AExpr, ArithOp, RelOp};

use std::collections::HashSet;
use std::collections::{BTreeMap, HashMap};
use z3::ast::Ast;
use z3::{
    Solver as Z3Solver,
    ast::{Bool, Real},
};

pub(super) struct Z3RealSolver {
    z3_solver: Z3Solver,
    z3_variables: BTreeMap<String, Real>,
    z3_ast_cache: HashMap<(bool, RelOp, AExpr, AExpr), Bool>,
    current_constraints: HashSet<(bool, RelOp, AExpr, AExpr)>,
    constraint_stack: Vec<Vec<(bool, RelOp, AExpr, AExpr)>>,
    result_cache: Option<bool>,
    unsat_core_extraction: bool,
    unsat_core: Option<Vec<usize>>,
}

impl Z3RealSolver {
    pub(super) fn new(unsat_core_extraction: bool) -> Self {
        Z3RealSolver {
            z3_solver: Z3Solver::new_for_logic("QF_LRA").unwrap_or_default(),
            z3_variables: BTreeMap::new(),
            z3_ast_cache: HashMap::new(),
            current_constraints: HashSet::new(),
            constraint_stack: Vec::new(),
            result_cache: Some(true),
            unsat_core_extraction,
            unsat_core: None,
        }
    }

    pub(super) fn push(&mut self) {
        tracing::Span::current().record("stack.size", self.constraint_stack.len());
        self.constraint_stack.push(Vec::new());
        self.z3_solver.push();
    }

    pub(super) fn pop(&mut self) {
        tracing::Span::current().record("stack.size", self.constraint_stack.len());
        if let Some(last) = self.constraint_stack.pop() {
            for key in last {
                self.current_constraints.remove(&key);
            }
        }
        self.z3_solver.pop(1);
        if self.result_cache == Some(false) {
            self.result_cache = None;
        }
    }

    pub(super) fn add_constraint(
        &mut self,
        negated: bool,
        op: RelOp,
        left: AExpr,
        right: AExpr,
        id: usize,
    ) {
        let key = (negated, op.clone(), left.clone(), right.clone());
        if self.current_constraints.insert(key.clone()) {
            let ast = if let Some(b) = self.z3_ast_cache.get(&key) {
                b.clone()
            } else {
                let value = self.rel_to_z3(negated, op, left, right);
                self.z3_ast_cache.insert(key.clone(), value.clone());
                value
            };

            if self.unsat_core_extraction {
                let p = z3::ast::Bool::new_const(format!("p_{id}").as_str());
                self.z3_solver.assert_and_track(ast, &p);
            } else {
                self.z3_solver.assert(ast);
            }
            self.result_cache = None;
            if let Some(last) = self.constraint_stack.last_mut() {
                last.push(key);
            }
        }
    }

    pub(super) fn check(&mut self) -> bool {
        tracing::Span::current().record("formula.size", self.current_constraints.len());

        tracing::trace!(
            "Z3 solver checking satisfiability of constraints: {:?}",
            self.current_constraints
        );

        if self.current_constraints.is_empty() {
            return true;
        }

        if let Some(res) = self.result_cache {
            tracing::Span::current().record("is_cached", true);
            res
        } else {
            let res = self.z3_solver.check();
            let sat = res == z3::SatResult::Sat;
            if self.unsat_core_extraction && !sat {
                let unsat_core = self.z3_solver.get_unsat_core();
                let mut core_ids = Vec::new();
                for expr in &unsat_core {
                    let name = expr.decl().name();
                    if name.starts_with("p_")
                        && let Ok(id) = name[2..].parse::<usize>()
                    {
                        core_ids.push(id);
                    }
                }
                self.unsat_core = Some(core_ids);
            }
            self.result_cache = Some(sat);
            sat
        }
    }

    pub(super) fn extract_unsat_core(&self) -> Option<Vec<usize>> {
        self.unsat_core.clone()
    }

    pub(super) fn empty_solver(&self) -> Self {
        let mut dst = Z3RealSolver::new(self.unsat_core_extraction);
        dst.z3_variables = self.z3_variables.clone();
        dst.z3_ast_cache = self.z3_ast_cache.clone();
        dst
    }

    fn aexpr_to_z3(&mut self, expr: &AExpr) -> Real {
        match expr {
            AExpr::Var(name) => {
                let name_str = name.to_string();
                if let Some(v) = self.z3_variables.get(&name_str) {
                    v.clone()
                } else {
                    let v = Real::new_const(name_str.as_str());
                    self.z3_variables.insert(name_str, v.clone());
                    v
                }
            }
            AExpr::Num(r) => Real::from_rational(*r.numer(), *r.denom()),
            AExpr::Abs(inner) => {
                let x = self.aexpr_to_z3(inner);
                let zero = Real::from_rational(0, 1);
                let cond = x.ge(&zero);
                let neg_x = &zero - &x;
                Bool::ite(&cond, &x, &neg_x)
            }
            AExpr::BinOp { op, left, right } => {
                let l = self.aexpr_to_z3(left);
                let r = self.aexpr_to_z3(right);
                match op {
                    ArithOp::Add => &l + &r,
                    ArithOp::Sub => &l - &r,
                }
            }
        }
    }

    fn rel_to_z3(&mut self, negated: bool, op: RelOp, left: AExpr, right: AExpr) -> Bool {
        let l = self.aexpr_to_z3(&left);
        let r = self.aexpr_to_z3(&right);
        let b = match op {
            RelOp::Lt => l.lt(&r),
            RelOp::Le => l.le(&r),
            RelOp::Gt => l.gt(&r),
            RelOp::Ge => l.ge(&r),
            RelOp::Eq => l.eq(&r),
            RelOp::Ne => l.eq(&r).not(),
        };
        if negated { b.not() } else { b }
    }
}
