use crate::formula::{AExpr, ExprKind, Formula, RelOp};
use crate::sat::config::SolverStrategy;
use crate::sat::tableau::node::Node;
use crate::sat::tableau::solver::z3::Z3RealSolver;

use std::collections::HashMap;

use std::sync::Arc;

#[cfg(test)]
mod tests;

mod z3;

#[derive(Clone, Debug)]
pub struct Assertion {
    pub id: usize,
    pub expr: ExprKind,
    pub negated: bool,
}

impl PartialEq for Assertion {
    fn eq(&self, other: &Self) -> bool {
        self.expr == other.expr && self.negated == other.negated
    }
}

impl Eq for Assertion {}

impl std::hash::Hash for Assertion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.expr.hash(state);
        self.negated.hash(state);
    }
}

pub struct Solver {
    unsat_core_extraction: bool,

    boolean_solver: BooleanSolver,
    real_solver: RealSolver,
}

impl Solver {
    #[must_use]
    fn new(unsat_core_extraction: bool, real_solver: RealSolver) -> Self {
        Solver {
            boolean_solver: BooleanSolver::new(unsat_core_extraction),
            real_solver,
            unsat_core_extraction,
        }
    }

    #[must_use]
    pub fn factory(
        unsat_core_extraction: bool,
        mltl: bool,
        strategy: SolverStrategy,
        _root: &Node,
    ) -> Self {
        Solver::new(
            unsat_core_extraction,
            match (mltl, strategy) {
                (true, _) => RealSolver::Empty,
                (false, SolverStrategy::Z3) => {
                    RealSolver::Z3(Z3RealSolver::new(unsat_core_extraction))
                }
                (false, SolverStrategy::Auto) => {
                    RealSolver::Z3(Z3RealSolver::new(unsat_core_extraction))
                }
            },
        )
    }

    #[must_use]
    pub fn empty_solver(&self) -> Self {
        Solver {
            boolean_solver: BooleanSolver::new(self.unsat_core_extraction),
            real_solver: self.real_solver.empty_solver(),
            unsat_core_extraction: self.unsat_core_extraction,
        }
    }

    pub fn push(&mut self) {
        self.boolean_solver.push();
        self.real_solver.push();
    }

    pub fn pop(&mut self) {
        self.boolean_solver.pop();
        self.real_solver.pop();
    }

    fn add_constraints(&mut self, node: &Node) {
        fn get_assertion(formula: &Formula) -> Option<Assertion> {
            match &formula {
                Formula::Prop(expr) => Some(Assertion {
                    id: expr.id,
                    expr: expr.kind.clone(),
                    negated: false,
                }),
                Formula::Not(inner) => get_assertion(inner).map(|mut ass| {
                    ass.negated = !ass.negated;
                    ass
                }),
                _ => None,
            }
        }
        node.operands
            .iter()
            .map(|f| &f.kind)
            .filter_map(get_assertion)
            .for_each(|ass| match &ass.expr {
                ExprKind::Atom(var) => {
                    self.boolean_solver.add_constraint(ass.negated, var, ass.id);
                }
                ExprKind::Rel { left, right, op } => {
                    self.real_solver.add_constraint(
                        ass.negated,
                        op.clone(),
                        left.clone(),
                        right.clone(),
                        ass.id,
                    );
                }
                _ => {}
            });
    }

    pub fn check(&mut self, node: &Node) -> bool {
        for f in &node.operands {
            match &f.kind {
                Formula::Prop(expr) if matches!(expr.kind, ExprKind::False) => {
                    if self.unsat_core_extraction {
                        self.boolean_solver.unsat_core = Some(vec![expr.id]);
                    }
                    return false;
                }
                Formula::Not(inner) => {
                    if let Formula::Prop(expr) = &**inner
                        && matches!(expr.kind, ExprKind::True)
                    {
                        if self.unsat_core_extraction {
                            self.boolean_solver.unsat_core = Some(vec![expr.id]);
                        }
                        return false;
                    }
                }
                _ => {}
            }
        }
        self.add_constraints(node);
        let bool_ok = self.boolean_solver.check();
        let real_ok = self.real_solver.check();

        bool_ok && real_ok
    }

    #[must_use]
    pub fn extract_unsat_core(&self) -> Option<Vec<usize>> {
        if let Some(vec) = self.boolean_solver.unsat_core.clone() {
            return Some(vec);
        }
        self.real_solver.extract_unsat_core()
    }
}

struct BooleanSolver {
    pos_props: HashMap<Arc<str>, usize>,
    neg_props: HashMap<Arc<str>, usize>,
    constraint_stack: Vec<Vec<(bool, Arc<str>)>>,

    result_cache: Option<bool>,

    unsat_core_extraction: bool,
    unsat_core: Option<Vec<usize>>,
}

impl BooleanSolver {
    fn new(unsat_core_extraction: bool) -> Self {
        BooleanSolver {
            pos_props: HashMap::with_capacity(64),
            neg_props: HashMap::with_capacity(64),
            constraint_stack: Vec::new(),
            result_cache: Some(true),
            unsat_core_extraction,
            unsat_core: None,
        }
    }

    fn push(&mut self) {
        self.constraint_stack.push(Vec::new());
    }

    fn pop(&mut self) {
        if let Some(last) = self.constraint_stack.pop() {
            for (negated, prop) in last {
                self.remove_constraint(negated, &prop);
            }
        }
    }

    fn add_constraint(&mut self, negated: bool, prop: &Arc<str>, id: usize) {
        if negated {
            self.neg_props.insert(prop.clone(), id);
            if let Some(id_stored) = self.pos_props.get(&**prop) {
                self.result_cache = Some(false);
                if self.unsat_core_extraction {
                    self.unsat_core = Some(vec![*id_stored, id]);
                }
            }
        } else {
            self.pos_props.insert(prop.clone(), id);
            if let Some(id_stored) = self.neg_props.get(&**prop) {
                self.result_cache = Some(false);
                if self.unsat_core_extraction {
                    self.unsat_core = Some(vec![*id_stored, id]);
                }
            }
        }
        if let Some(last) = self.constraint_stack.last_mut() {
            last.push((negated, prop.clone()));
        }
    }

    fn remove_constraint(&mut self, negated: bool, prop: &str) {
        if negated {
            self.neg_props.remove(prop);
        } else {
            self.pos_props.remove(prop);
        }
        if self.result_cache == Some(false) {
            self.result_cache = None;
        }
    }

    fn check(&mut self) -> bool {
        if let Some(res) = self.result_cache {
            res
        } else {
            let res = !self
                .pos_props
                .keys()
                .any(|pos| self.neg_props.contains_key(pos));
            self.result_cache = Some(res);
            if self.unsat_core_extraction && !res {
                for (prop, id) in &self.pos_props {
                    if let Some(neg_id) = self.neg_props.get(prop) {
                        self.unsat_core = Some(vec![*id, *neg_id]);
                        break;
                    }
                }
            }
            res
        }
    }
}

enum RealSolver {
    Empty,
    Z3(Z3RealSolver),
}

impl RealSolver {
    fn push(&mut self) {
        match self {
            RealSolver::Empty => {}
            RealSolver::Z3(solver) => solver.push(),
        }
    }

    fn pop(&mut self) {
        match self {
            RealSolver::Empty => {}
            RealSolver::Z3(solver) => solver.pop(),
        }
    }

    fn add_constraint(&mut self, negated: bool, op: RelOp, left: AExpr, right: AExpr, id: usize) {
        match self {
            RealSolver::Empty => panic!("Attempted to add real constraint to empty real solver"),
            RealSolver::Z3(solver) => solver.add_constraint(negated, op, left, right, id),
        }
    }

    fn check(&mut self) -> bool {
        match self {
            RealSolver::Empty => true,
            RealSolver::Z3(solver) => solver.check(),
        }
    }

    fn extract_unsat_core(&self) -> Option<Vec<usize>> {
        match self {
            RealSolver::Empty => None,
            RealSolver::Z3(solver) => solver.extract_unsat_core(),
        }
    }

    fn empty_solver(&self) -> Self {
        match self {
            RealSolver::Empty => RealSolver::Empty,
            RealSolver::Z3(src) => RealSolver::Z3(src.empty_solver()),
        }
    }
}
