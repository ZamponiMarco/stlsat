//! This module implements a solver for difference logic (DL) constraints.
//!
//! The solver is based on the Bellman-Ford algorithm to detect negative cycles
//! in a constraint graph. It supports strict and non-strict difference constraints.
//! Strict constraints are handled by treating them as non-strict constraints
//! with an infinitesimally small epsilon subtracted from the constant term. The
//! value of epsilon is not explicitly calculated, but the solver keeps track
//! of whether a constraint is strict or non-strict and adjusts the cycle finding
//! algorithm accordingly. This approach is based on the technique in
//!
//! Dutertre, Bruno, and Leonardo De Moura.
//! "A fast linear-arithmetic solver for DPLL (T)."
//! International Conference on Computer Aided Verification.
//! Berlin, Heidelberg: Springer Berlin Heidelberg, 2006.
//! <https://leodemoura.github.io/files/cav06.pdf>
//!
//! The solver also supports equality and disequality constraints, as well as
//! constraints involving absolute values, by reducing them to difference
//! constraints. For example
//!
//! ```txt
//! |x - y| <= c
//! x - y <= c && y - x <= c
//! ```
//!
//! and
//!
//! ```txt
//! x - y != c
//! x - y < c || x - y > c
//! x - y <= c - ε || y - x <= -c - ε
//! ```
//!
//! This, however, is limited to inputs that lead to conjunctions of
//! disjunctions of at most two difference constraints. For example, the input
//! constraint `|x| != c` is not supported, since it leads to disjunctions of three
//! difference constraints:
//!
//! ```txt
//! |x| != c
//! |x| < c || |x| > c
//! (x < c && x > -c) || (x > c || x < -c)
//! (x < c || x > c || x < -c) && (x > -c || x > c || x < -c)
//! ```
//!
//! If the input constraints are unsatisfiable, the solver can extract an unsatisfiable
//! core.
//!
//! The solver panics if it is used with unsupported constraints.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Add;
use std::rc::Rc;

use num_rational::Ratio;

use crate::formula::{AExpr, ArithOp, ExprKind, Formula, RelOp, VariableName};
use crate::sat::tableau::node::Node;

#[cfg(test)]
mod tests;

/// Name of the artificial variable representing zero that is used
/// to handle constraints with only one variable.
const X_ZERO_NAME: &str = "__dl_zero";

/// Represents a difference constraint of the form:
///
/// ```txt
/// x - y <= c
/// ```
///
/// or
///
/// ```txt
/// x - y < c
/// ```
///
/// depending on the value of the `strict` field.
///
/// The `id` field contains the id of the input constraint. Note that two
/// difference constraints can have the same `id` if they originate
/// from the same input constraint that was split into multiple difference
/// constraints (e.g., equality constraints).
///
/// The `id` field is optional because it is only needed for unsat core extraction.
/// Using `None` when unsat core extraction is not enabled improves performance
/// because it leads to better deduplication of constraints in the `ClauseSet` and
/// improved cache hit rate in the solver's result cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DifferenceConstraint {
    x: VariableName,
    y: VariableName,
    c: Ratio<i64>,
    strict: bool,
    id: Option<usize>,
}

impl DifferenceConstraint {
    fn new(constraint: &NormalizedConstraint) -> Result<Self, DifferenceConstraintError> {
        let left = &constraint.left;
        let right = &constraint.right;
        let op = &constraint.op;
        let id = constraint.id;

        match (left, right) {
            // x - y ? c
            (
                AExpr::BinOp {
                    op: ArithOp::Sub,
                    left: x,
                    right: y,
                },
                AExpr::Num(c),
            ) => match (&**x, &**y) {
                (AExpr::Var(x_name), AExpr::Var(y_name)) => {
                    assert_ne!(
                        **x_name, *X_ZERO_NAME,
                        "Variable name collision with reserved zero variable"
                    );
                    assert_ne!(
                        **y_name, *X_ZERO_NAME,
                        "Variable name collision with reserved zero variable"
                    );
                    Self::sub_op_num(op, x_name, y_name, *c, id)
                }
                _ => Err(DifferenceConstraintError::InvalidSubExpression),
            },
            // x ? c
            (AExpr::Var(x_name), AExpr::Num(c)) => {
                assert_ne!(
                    **x_name, *X_ZERO_NAME,
                    "Variable name collision with reserved zero variable"
                );
                Self::sub_op_num(op, x_name, &VariableName::from(X_ZERO_NAME), *c, id)
            }
            // x ? y
            (AExpr::Var(x_name), AExpr::Var(y_name)) => {
                assert_ne!(
                    **x_name, *X_ZERO_NAME,
                    "Variable name collision with reserved zero variable"
                );
                assert_ne!(
                    **y_name, *X_ZERO_NAME,
                    "Variable name collision with reserved zero variable"
                );

                Self::sub_op_num(op, x_name, y_name, Ratio::ZERO, id)
            }
            _ => Err(DifferenceConstraintError::InvalidExpression),
        }
    }

    fn sub_op_num(
        op: &RelOp,
        x: &VariableName,
        y: &VariableName,
        c: Ratio<i64>,
        id: Option<usize>,
    ) -> Result<Self, DifferenceConstraintError> {
        match op {
            // x - y <= c
            RelOp::Le => Ok(DifferenceConstraint {
                x: x.clone(),
                y: y.clone(),
                c,
                strict: false,
                id,
            }),
            // x - y < c  ==>  x - y <= c - ε
            RelOp::Lt => Ok(DifferenceConstraint {
                x: x.clone(),
                y: y.clone(),
                c,
                strict: true,
                id,
            }),
            // x - y >= c  ==>  y - x <= -c
            RelOp::Ge => Ok(DifferenceConstraint {
                x: y.clone(),
                y: x.clone(),
                c: -c,
                strict: false,
                id,
            }),
            // x - y > c  ==>  y - x <= -c - ε
            RelOp::Gt => Ok(DifferenceConstraint {
                x: y.clone(),
                y: x.clone(),
                c: -c,
                strict: true,
                id,
            }),
            _ => Err(DifferenceConstraintError::InvalidRelation),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Eq, PartialEq)]
enum DifferenceConstraintError {
    InvalidRelation,
    InvalidExpression,
    InvalidSubExpression,
}

/// The `NormalizedConstraint` ensures that the left expression
/// is not a number unless both expressions are numbers.
/// This reduces the number of cases that need to be handled
/// in `DifferenceConstraint` and `ClauseSet` functions.
/// It also handles negating the relation if necessary and
/// provides some helper functions that make other functions
/// easier to implement.
struct NormalizedConstraint {
    op: RelOp,
    left: AExpr,
    right: AExpr,
    id: Option<usize>,
}

impl NormalizedConstraint {
    #[must_use]
    fn new(negated: bool, op: &RelOp, left: &AExpr, right: &AExpr, id: Option<usize>) -> Self {
        if negated {
            let negated_op = match op {
                RelOp::Le => RelOp::Gt,
                RelOp::Lt => RelOp::Ge,
                RelOp::Ge => RelOp::Lt,
                RelOp::Gt => RelOp::Le,
                RelOp::Eq => RelOp::Ne,
                RelOp::Ne => RelOp::Eq,
            };
            NormalizedConstraint::new(false, &negated_op, left, right, id)
        } else {
            match (left, right) {
                (AExpr::Num(_), _) => NormalizedConstraint {
                    op: match op {
                        RelOp::Le => RelOp::Ge,
                        RelOp::Lt => RelOp::Gt,
                        RelOp::Ge => RelOp::Le,
                        RelOp::Gt => RelOp::Lt,
                        RelOp::Eq => RelOp::Eq, // Symmetric
                        RelOp::Ne => RelOp::Ne, // Symmetric
                    },
                    left: right.clone(),
                    right: left.clone(),
                    id,
                },
                _ => NormalizedConstraint {
                    op: op.clone(),
                    left: left.clone(),
                    right: right.clone(),
                    id,
                },
            }
        }
    }

    #[must_use]
    fn with(&self, op: &RelOp, left: &AExpr, right: &AExpr) -> Self {
        NormalizedConstraint::new(false, op, left, right, self.id)
    }

    fn to_diff(&self) -> Result<DifferenceConstraint, DifferenceConstraintError> {
        DifferenceConstraint::new(self)
    }
}

// #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
// enum Clause {
//     Unary(DifferenceConstraint),
//     Binary(DifferenceConstraint, DifferenceConstraint),
// }

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct UnaryClause(DifferenceConstraint);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct BinaryClause(DifferenceConstraint, DifferenceConstraint);

/// The `ClauseSet` stores clauses in a `BTreeSet` wrapped in `Rc` for efficient cloning.
/// The `BTreeSet` ensures that clauses are unique. We cannot use a `HashSet` here as it
/// does not implement `Hash`, which is needed because the `ClauseSet` is used as the
/// solver cache key.
/// The `Rc` allows for cheap cloning of the `ClauseSet` when pushing on the solver stack
/// and when using it to access the result cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ClauseSet {
    unary: Rc<BTreeSet<UnaryClause>>,
    binary: Rc<BTreeSet<BinaryClause>>,
}

impl ClauseSet {
    #[must_use]
    fn new() -> Self {
        ClauseSet {
            unary: Rc::new(BTreeSet::new()),
            binary: Rc::new(BTreeSet::new()),
        }
    }

    #[must_use]
    fn len(&self) -> usize {
        self.unary.len() + self.binary.len()
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.unary.is_empty() && self.binary.is_empty()
    }

    fn insert_unary(&mut self, clause: UnaryClause) {
        Rc::make_mut(&mut self.unary).insert(clause);
    }

    fn insert_binary(&mut self, clause: BinaryClause) {
        Rc::make_mut(&mut self.binary).insert(clause);
    }

    fn add_constraint(
        &mut self,
        constraint: &NormalizedConstraint,
    ) -> Result<(), DifferenceConstraintError> {
        let op = &constraint.op;
        let left = &constraint.left;
        let right = &constraint.right;

        // e1 = e2  ==>  e1 <= e2 && e1 >= e2
        if *op == RelOp::Eq {
            self.add_constraint(&constraint.with(&RelOp::Le, left, right))?;
            self.add_constraint(&constraint.with(&RelOp::Ge, left, right))?;
            return Ok(());
        }

        // e1 != e2  ==>  e1 < e2 || e1 > e2
        if *op == RelOp::Ne {
            self.insert_binary(BinaryClause(
                constraint.with(&RelOp::Lt, left, right).to_diff()?,
                constraint.with(&RelOp::Gt, left, right).to_diff()?,
            ));
            return Ok(());
        }

        // |e| < c  ==>  e < c && e > -c
        if *op == RelOp::Lt
            && let AExpr::Abs(expr) = &left
            && let AExpr::Num(c) = &right
        {
            self.add_constraint(&constraint.with(&RelOp::Lt, expr, right))?;
            self.add_constraint(&constraint.with(&RelOp::Gt, expr, &AExpr::Num(-*c)))?;
            return Ok(());
        }

        // |e| <= c  ==>  e <= c && e >= -c
        if *op == RelOp::Le
            && let AExpr::Abs(expr) = &left
            && let AExpr::Num(c) = &right
        {
            self.add_constraint(&constraint.with(&RelOp::Le, expr, right))?;
            self.add_constraint(&constraint.with(&RelOp::Ge, expr, &AExpr::Num(-*c)))?;
            return Ok(());
        }

        // |e| > c  ==>  e > c || e < -c
        if *op == RelOp::Gt
            && let AExpr::Abs(expr) = &left
            && let AExpr::Num(c) = &right
        {
            self.insert_binary(BinaryClause(
                constraint.with(&RelOp::Gt, expr, right).to_diff()?,
                constraint
                    .with(&RelOp::Lt, expr, &AExpr::Num(-*c))
                    .to_diff()?,
            ));
            return Ok(());
        }

        // |e| >= c  ==>  e >= c || e <= -c
        if *op == RelOp::Ge
            && let AExpr::Abs(expr) = &left
            && let AExpr::Num(c) = &right
        {
            self.insert_binary(BinaryClause(
                constraint.with(&RelOp::Ge, expr, right).to_diff()?,
                constraint
                    .with(&RelOp::Le, expr, &AExpr::Num(-*c))
                    .to_diff()?,
            ));
            return Ok(());
        }

        self.insert_unary(UnaryClause(constraint.to_diff()?));
        Ok(())
    }
}

pub(super) struct DifferenceLogicSolver {
    /// Whether unsat core extraction is enabled.
    unsat_core_extraction: bool,
    /// The current set of clauses.
    clause_set: ClauseSet,
    /// Stack of clause sets for push/pop.
    stack: Vec<ClauseSet>,
    /// The unsat core of the last check, if available.
    unsat_core: Option<Vec<usize>>,
    /// Cache of previous results. The first element in the value tuple
    /// is the result, the second is the unsat core if available.
    result_cache: HashMap<ClauseSet, (bool, Option<Vec<usize>>)>,
}

impl DifferenceLogicSolver {
    pub(super) fn new(unsat_core_extraction: bool) -> Self {
        DifferenceLogicSolver {
            unsat_core_extraction,
            clause_set: ClauseSet::new(),
            stack: Vec::new(),
            unsat_core: None,
            result_cache: HashMap::new(),
        }
    }

    pub(super) fn empty_solver(&self) -> Self {
        DifferenceLogicSolver {
            unsat_core_extraction: self.unsat_core_extraction,
            clause_set: ClauseSet::new(),
            stack: Vec::new(),
            unsat_core: None,
            result_cache: self.result_cache.clone(),
        }
    }

    pub(super) fn push(&mut self) {
        tracing::Span::current().record("stack.size", self.stack.len());
        self.stack.push(self.clause_set.clone());
    }

    pub(super) fn pop(&mut self) {
        tracing::Span::current().record("stack.size", self.stack.len());
        if let Some(prev) = self.stack.pop() {
            self.clause_set = prev;
        } else {
            panic!("DifferenceLogicSolver pop called on empty stack");
        }
    }

    pub(super) fn add_constraint(
        &mut self,
        negated: bool,
        op: &RelOp,
        left: &AExpr,
        right: &AExpr,
        id: usize,
    ) {
        self.clause_set
            .add_constraint(&NormalizedConstraint::new(
                negated,
                op,
                left,
                right,
                if self.unsat_core_extraction {
                    Some(id)
                } else {
                    None
                },
            ))
            .expect("DifferenceLogicSolver received unsupported constraint");
    }

    pub(super) fn check(&mut self) -> bool {
        tracing::Span::current().record("formula.size", self.clause_set.len());

        tracing::trace!(
            "DL Solver checking satisfiability of clause set: {:?}",
            self.clause_set
        );

        // An empty clause set is always satisfiable.
        if self.clause_set.is_empty() {
            return true;
        }

        // Check cache for previous results.
        if let Some((sat, unsat_core)) = self.result_cache.get(&self.clause_set) {
            tracing::Span::current().record("is_cached", true);
            self.unsat_core = unsat_core.clone();
            return *sat;
        }

        // Build a base constraint graph with all unary clauses and collect all binary clauses for later processing.
        let mut base_graph = ConstraintGraph::with_capacity(self.clause_set.unary.len());
        for UnaryClause(constraint) in self.clause_set.unary.as_ref() {
            base_graph.add_edge(
                &constraint.y,
                &constraint.x,
                EdgeWeight(constraint.c, usize::from(constraint.strict)),
                constraint.id,
            );
        }

        // Check base graph for negative cycles.
        // If the base graph is unsat, we can return immediately.
        // If the base graph is sat and there are no binary clauses, we can also return immediately.
        match base_graph.find_negative_cycle(self.unsat_core_extraction) {
            NegativeCycleResult::CycleWithoutCore => {
                self.unsat_core = None;
                self.result_cache
                    .insert(self.clause_set.clone(), (false, None));
                return false;
            }
            NegativeCycleResult::CycleWithCore(cycle) => {
                self.unsat_core = Some(base_graph.build_unsat_core(&cycle));
                self.result_cache
                    .insert(self.clause_set.clone(), (false, self.unsat_core.clone()));
                return false;
            }
            NegativeCycleResult::NoCycle => {
                if self.clause_set.binary.is_empty() {
                    // If there are no binary clauses and the base graph has no negative cycles,
                    // then the whole formula is sat.
                    self.result_cache
                        .insert(self.clause_set.clone(), (true, None));
                    return true;
                }
            }
        }

        // Ensure we have enough bits to handle all combinations of binary clauses.
        // `usize` is probably 64 bits, so this limits us to 64 binary clauses.
        // In practice, this should not be a problem, because 64 binary clauses lead to 2^64
        // formulas to check, which is not feasible anyway.
        assert!(
            usize::BITS as usize >= self.clause_set.binary.len(),
            "DifferenceLogicSolver can only handle up to {} binary clauses, but there are {}",
            usize::BITS,
            self.clause_set.binary.len(),
        );

        // Iterate over all combinations of constraints in the binary clauses and check
        // the resulting graphs for negative cycles.
        let total_combinations = 1usize << self.clause_set.binary.len();
        for mask in 0..total_combinations {
            // Extend the base graph with the selected binary clauses.
            let mut graph = base_graph.clone();
            for (j, BinaryClause(c1, c2)) in self.clause_set.binary.iter().enumerate() {
                let constraint = if (mask & (1 << j)) == 0 { c1 } else { c2 };
                graph.add_edge(
                    &constraint.y,
                    &constraint.x,
                    EdgeWeight(constraint.c, usize::from(constraint.strict)),
                    constraint.id,
                );
            }

            // If the graph has no negative cycles, the clause set is sat and we can return.
            // Otherwise, continue with the next combination, but keep the unsat core (if enabled)
            // in case this is the last combination.
            match graph.find_negative_cycle(self.unsat_core_extraction) {
                NegativeCycleResult::NoCycle => {
                    self.unsat_core = None;
                    self.result_cache
                        .insert(self.clause_set.clone(), (true, None));
                    return true;
                }
                NegativeCycleResult::CycleWithoutCore => {
                    self.unsat_core = None;
                }
                NegativeCycleResult::CycleWithCore(cycle) => {
                    self.unsat_core = Some(graph.build_unsat_core(&cycle));
                }
            }
        }

        // All combinations of binary clauses lead to graphs with negative cycles,
        // so the clause set is unsat.
        self.result_cache
            .insert(self.clause_set.clone(), (false, self.unsat_core.clone()));
        false
    }

    /// Returns the unsat core of last checked clause set as a list of constraint ids.
    /// Panics if unsat core extraction was not enabled.
    /// Returns `None` if no unsat core is available.
    /// Must only be called after `check()` returned `false`.
    pub(super) fn extract_unsat_core(&self) -> Option<Vec<usize>> {
        assert!(
            self.unsat_core_extraction,
            "DifferenceLogicSolver extract_unsat_core called without enabling unsat core extraction"
        );

        self.unsat_core.clone()
    }

    /// Checks whether the given formula tree can be handled by the DL solver.
    pub(super) fn supports(root: &Node) -> bool {
        let mut stack: Vec<Formula> = root.operands.iter().map(|f| f.kind.clone()).collect();
        let mut clauses = ClauseSet::new();

        while let Some(formula) = stack.pop() {
            match &formula {
                Formula::Prop(expr) => {
                    if let ExprKind::Rel { op, left, right } = &expr.kind
                        && clauses
                            .add_constraint(&NormalizedConstraint::new(
                                false, op, left, right, None,
                            ))
                            .is_err()
                    {
                        log::debug!("DL does not support formula: {:?}", &formula);
                        return false;
                    }
                }
                Formula::And(ops) | Formula::Or(ops) => {
                    for op in ops {
                        stack.push(op.clone());
                    }
                }
                Formula::Imply {
                    left,
                    right,
                    not_left,
                } => {
                    stack.push(*left.clone());
                    stack.push(*right.clone());
                    stack.push(*not_left.clone());
                }
                Formula::Not(inner) => {
                    stack.push(*inner.clone());
                }
                Formula::G { phi, .. } | Formula::F { phi, .. } => {
                    stack.push(*phi.clone());
                }
                Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                    stack.push(*left.clone());
                    stack.push(*right.clone());
                }
            }
        }

        true
    }
}

/// Struct to represent edge weights in the constraint graph.
/// The second field indicates strictness. An edge weight (c, k)
/// can be interpreted as c - k * ε, where ε is an infinitesimally
/// small positive number. Thus, higher values of k indicate
/// a "smaller" weight. If k is zero, the constraint is non-strict.
/// This approach allows us to use the standard Bellman-Ford algorithm
/// to find negative cycles without having to explicitly compute ε.
/// This approach is based on the technique in:
///
/// Dutertre, Bruno, and Leonardo De Moura.
/// "A fast linear-arithmetic solver for DPLL (T)."
/// International Conference on Computer Aided Verification.
/// Berlin, Heidelberg: Springer Berlin Heidelberg, 2006.
/// <https://leodemoura.github.io/files/cav06.pdf>
#[derive(Debug, Clone, PartialEq, Copy)]
struct EdgeWeight(Ratio<i64>, usize);

impl Add for EdgeWeight {
    type Output = EdgeWeight;

    /// (c, k) + (c', k') = (c + c', k + k')
    fn add(self, other: EdgeWeight) -> EdgeWeight {
        let new_c = self.0 + other.0;
        let new_strict = self
            .1
            .checked_add(other.1)
            .expect("Strictness counter overflow");
        EdgeWeight(new_c, new_strict)
    }
}

impl PartialOrd for EdgeWeight {
    /// (c, k) < (c', k')  iff  c < c'  or  (c == c'  and  k > k')
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.0.partial_cmp(&other.0) {
            Some(std::cmp::Ordering::Equal) => other.1.partial_cmp(&self.1),
            ord => ord,
        }
    }
}

/// A directed graph representing strict and non-strict difference
/// constraints.
/// Variable names are mapped to vertex ids using `vertex_ids`. Using
/// ids instead of names makes the Bellman-Ford implementation faster
/// because it can use vectors instead of hash maps for distances and
/// predecessors.
/// The `edges` are stored as a `HashMap` because we want to
/// keep only the edge with the smallest weight between two vertices.
/// Duplicate edges cause unsat core extraction to be unreliable, and
/// slow down the algorithm because more edges need to be processed.
#[derive(Clone)]
struct ConstraintGraph {
    edges: HashMap<(usize, usize), (EdgeWeight, Option<usize>)>,
    vertex_ids: HashMap<String, usize>,
}

#[derive(Debug)]
enum NegativeCycleResult {
    NoCycle,
    CycleWithoutCore,
    CycleWithCore(Vec<usize>),
}

impl ConstraintGraph {
    #[must_use]
    fn with_capacity(capacity: usize) -> Self {
        ConstraintGraph {
            edges: HashMap::with_capacity(capacity),
            vertex_ids: HashMap::with_capacity(capacity * 2),
        }
    }

    fn add_edge(&mut self, from: &str, to: &str, weight: EdgeWeight, constraint_id: Option<usize>) {
        let from_id = self.get_or_insert_vertex_id(from);
        let to_id = self.get_or_insert_vertex_id(to);

        self.edges
            .entry((from_id, to_id))
            .and_modify(|entry| {
                if weight < entry.0 {
                    entry.0 = weight;
                    entry.1 = constraint_id;
                }
            })
            .or_insert((weight, constraint_id));
    }

    fn get_or_insert_vertex_id(&mut self, vertex: &str) -> usize {
        let id = self.vertex_ids.len();
        self.vertex_ids
            .entry(vertex.to_string())
            .or_insert(id)
            .to_owned()
    }

    /// Find a negative cycle in the constraint graph using the Bellman-Ford algorithm.
    /// Returns `None` if no negative cycle is found.
    /// Returns `Some(None)` if a negative cycle is found but `extract_core` is false.
    /// Returns the cycle as a list of vertex ids if found, for example:
    /// `[1, 2, 3]` for a cycle `1 -> 2 -> 3 -> 1`.
    fn find_negative_cycle(&self, extract_core: bool) -> NegativeCycleResult {
        let edge_list = &self.edges.iter().collect::<Vec<_>>();

        // Step 1: Initialization
        // Normal Bellman-Ford initializes distances to infinity,
        // but we can initialize them to zero since we want to find cycles anywhere in
        // the graph, not only reachable from a specific source. This has the same effect
        // as adding an artificial root node connected to all nodes with zero-weight edges.
        let mut distances: Vec<EdgeWeight> =
            vec![EdgeWeight(Ratio::ZERO, 0); self.vertex_ids.len()];
        let mut predecessors: Vec<Option<&usize>> = vec![None; self.vertex_ids.len()];

        // Step 2: Relax edges
        for _ in 1..self.vertex_ids.len() {
            let mut updated = false;
            for ((from, to), (weight, _)) in edge_list {
                let dist_u = distances[*from];
                let dist_v = distances[*to];

                let new_dist = dist_u + *weight;
                if new_dist < dist_v {
                    distances[*to] = new_dist;
                    predecessors[*to] = Some(from);
                    updated = true;
                }
            }

            if !updated {
                return NegativeCycleResult::NoCycle;
            }
        }

        // Step 3: Check for negative cycles
        for ((from, to), (weight, _)) in edge_list {
            let dist_u = distances[*from];
            let dist_v = distances[*to];

            let new_dist = dist_u + *weight;
            if new_dist < dist_v {
                if !extract_core {
                    // No need to do any more work if we don't need the actual cycle.
                    return NegativeCycleResult::CycleWithoutCore;
                }
                predecessors[*to] = Some(from);
                // Find a vertex on the negative cycle.
                // The edge from `from` to `to` is certainly reachable from a negative cycle,
                // but not necessarily part of it.
                let mut visited = HashSet::new();
                let mut current = to;
                while !visited.contains(current) {
                    visited.insert(current);
                    current =
                        predecessors[*current].expect("Predecessor of visited vertex should exist");
                }

                // Reconstruct the negative cycle.
                let cycle_start = current;
                let mut cycle = Vec::new();
                cycle.push(*cycle_start);
                current =
                    predecessors[*cycle_start].expect("Predecessor of cycle start should exist");
                while current != cycle_start {
                    cycle.push(*current);
                    current =
                        predecessors[*current].expect("Predecessor of cycle vertex should exist");
                }
                cycle.reverse();

                return NegativeCycleResult::CycleWithCore(cycle);
            }
        }

        NegativeCycleResult::NoCycle
    }

    /// Maps a negative cycle (list of vertex ids) to the list of
    /// constraint ids that form the unsat core. The returned list is sorted
    /// in ascending order.
    fn build_unsat_core(&self, negative_cycle: &[usize]) -> Vec<usize> {
        let mut constraint_ids = vec![];

        // Iterate over consecutive pairs of nodes in the cycle, find the corresponding
        // edge and extract the constraint id.
        for edge_nodes in negative_cycle.windows(2) {
            let edge = self
                .edges
                .get(&(edge_nodes[0], edge_nodes[1]))
                .expect("Edge in negative cycle not found in constraint graph");
            constraint_ids.push(edge.1.expect("Edge has no constraint id"));
        }

        // Add the edge that closes the cycle.
        let edge = self
            .edges
            .get(&(negative_cycle[negative_cycle.len() - 1], negative_cycle[0]))
            .expect("Edge in negative cycle not found in constraint graph");
        constraint_ids.push(edge.1.expect("Edge has no constraint id"));

        constraint_ids.sort_unstable();
        constraint_ids
    }
}
