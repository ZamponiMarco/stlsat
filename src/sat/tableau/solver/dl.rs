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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    mod constraint_graph {
        use super::*;

        #[test]
        fn test_strict_zero_no_cycle() {
            CycleTest::new()
                .add("x1", "x0", -1)
                .add_strict("x2", "x1", 0)
                .add("x0", "x1", 1)
                .add("x2", "dl0", Ratio::new(-8, 25))
                .add("dl0", "x2", Ratio::new(8, 25))
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_two_overlapping_negative_cycles() {
            CycleTest::new()
                // u -> v
                .add("u", "v", 0)
                // v -> a -> b -> u
                .add("v", "a", -2)
                .add("a", "b", -2)
                .add("b", "u", -2)
                // v -> x -> y -> u
                .add("v", "x", -1)
                .add("x", "y", -1)
                .add("y", "u", -1)
                .should_have_one_of_negative_cycles(vec![
                    vec!["u", "v", "a", "b"],
                    vec!["u", "v", "x", "y"],
                ]);
        }

        #[test]
        fn test_two_overlapping_cycles_one_positive() {
            CycleTest::new()
                // u -> v
                .add("u", "v", 0)
                // v -> a -> b -> u
                .add("v", "a", 1)
                .add("a", "b", 1)
                .add("b", "u", 1)
                // v -> x -> y -> u
                .add("v", "x", -1)
                .add("x", "y", -1)
                .add("y", "u", -1)
                .should_have_negative_cycle(vec!["u", "v", "x", "y"]);
        }

        #[test]
        fn test_disconnected_one_negative() {
            CycleTest::new()
                // x -> y -> z -> x
                .add("x", "y", 0)
                .add("y", "z", 0)
                .add_strict("z", "x", 0)
                // a -> b -> c -> a
                .add("a", "b", -1)
                .add("b", "c", 1)
                .add("c", "a", Ratio::new(1, 10))
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_disconnected_both_negative() {
            CycleTest::new()
                // x -> y -> z -> x
                .add("x", "y", 0)
                .add("y", "z", 0)
                .add_strict("z", "x", 0)
                // a -> b -> c -> a
                .add("a", "b", -1)
                .add("b", "c", -1)
                .add("c", "a", -1)
                .should_have_one_of_negative_cycles(vec![vec!["a", "b", "c"], vec!["x", "y", "z"]]);
        }

        #[test]
        fn test_disconnected_none_negative() {
            CycleTest::new()
                // x -> y -> z -> x
                .add("x", "y", 0)
                .add("y", "z", 0)
                .add("z", "x", 1)
                // a -> b -> c -> a
                .add("a", "b", 0)
                .add("b", "c", 0)
                .add("c", "a", 2)
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_all_zero_all_non_strict_is_not_negative() {
            CycleTest::new()
                .add("x", "y", 0)
                .add("y", "z", 0)
                .add("z", "x", 0)
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_all_zero_all_strict_is_negative() {
            CycleTest::new()
                .add_strict("x", "y", 0)
                .add_strict("y", "z", 0)
                .add_strict("z", "x", 0)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_sum_zero_non_strict_is_not_negative() {
            CycleTest::new()
                .add("x", "y", 5)
                .add("y", "z", -2)
                .add("z", "x", -3)
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_all_negative_is_negative() {
            CycleTest::new()
                .add("x", "y", -1)
                .add("y", "z", -2)
                .add("z", "x", -3)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_all_negative_strict_is_negative() {
            CycleTest::new()
                .add_strict("x", "y", -1)
                .add_strict("y", "z", -2)
                .add_strict("z", "x", -3)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_all_zero_some_strict_is_negative() {
            CycleTest::new()
                .add("x", "y", 0)
                .add("y", "z", 0)
                .add_strict("z", "x", 0)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_sum_zero_some_strict_is_negative() {
            CycleTest::new()
                .add("x", "y", 5)
                .add("y", "z", -2)
                .add_strict("z", "x", -3)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_negative_non_cycle() {
            CycleTest::new()
                .add("x1", "x2", -5)
                .add("x2", "x3", -3)
                .add("x3", "x4", -2)
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_cse3220_sat() {
            // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
            CycleTest::new()
                .add("x1", "x3", -5)
                .add("x1", "x4", -3)
                .add("x2", "x1", 3)
                .add("x3", "x2", 2)
                .add("x3", "x4", -1)
                .add("x4", "x2", 5)
                .should_not_have_negative_cycle();
        }

        #[test]
        fn test_cse3220_unsat() {
            // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
            CycleTest::new()
                .add("x1", "x3", -6)
                .add("x1", "x4", -3)
                .add("x2", "x1", 3)
                .add("x3", "x2", 2)
                .add("x3", "x4", -1)
                .add("x4", "x2", 5)
                .should_have_negative_cycle(vec!["x3", "x2", "x1"]);
        }

        #[test]
        fn test_duplicate_edge_one_positive() {
            CycleTest::new()
                .add("x", "y", 0)
                .add("x", "y", 2)
                .add("y", "z", -1)
                .add("z", "x", -1)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_duplicate_edge_both_negative() {
            CycleTest::new()
                .add("x", "y", -1)
                .add("x", "y", -2)
                .add("y", "z", 0)
                .add("z", "x", 0)
                .should_have_negative_cycle(vec!["x", "y", "z"]);
        }

        #[test]
        fn test_positive_strict() {
            CycleTest::new()
                .add_strict("x", "y", 1)
                .add_strict("y", "z", 0)
                .add_strict("z", "x", 0)
                .should_not_have_negative_cycle();
        }

        struct CycleTest {
            graph: ConstraintGraph,
        }

        impl CycleTest {
            fn new() -> Self {
                CycleTest {
                    graph: ConstraintGraph::with_capacity(0),
                }
            }

            fn add(mut self, from: &str, to: &str, weight: impl Into<Ratio<i64>>) -> Self {
                self.graph
                    .add_edge(from, to, EdgeWeight(weight.into(), 0), None);
                self
            }

            fn add_strict(mut self, from: &str, to: &str, weight: impl Into<Ratio<i64>>) -> Self {
                self.graph
                    .add_edge(from, to, EdgeWeight(weight.into(), 1), None);
                self
            }

            fn should_have_negative_cycle(self, expected: Vec<&str>) {
                self.should_have_one_of_negative_cycles(vec![expected]);
            }

            fn should_have_one_of_negative_cycles(self, expecteds: Vec<Vec<&str>>) {
                let neg_cycle = self.graph.find_negative_cycle(true);
                let actual = match neg_cycle {
                    NegativeCycleResult::CycleWithCore(cycle) => cycle,
                    NegativeCycleResult::CycleWithoutCore => {
                        panic!("Expected negative cycle with core, but got without core")
                    }
                    NegativeCycleResult::NoCycle => {
                        panic!("Expected negative cycle, but none found")
                    }
                };

                for expected in &expecteds {
                    if self.cycle_equals_rotating(&actual, &expected) {
                        return;
                    }
                }

                panic!(
                    "Negative cycle {:?} does not match any of the expected cycles {:?} (even considering rotations)",
                    actual, expecteds
                );
            }

            fn cycle_equals_rotating(&self, actual: &[usize], expected: &[&str]) -> bool {
                if actual.len() != expected.len() {
                    return false;
                }

                let mut actual_vec: Vec<&str> = actual
                    .iter()
                    .map(|id| {
                        self.graph
                            .vertex_ids
                            .iter()
                            .find(|(_, v_id)| *v_id == id)
                            .map(|(name, _)| name.as_str())
                            .expect("Vertex id not found in graph")
                    })
                    .collect();

                for _ in 0..actual_vec.len() {
                    if actual_vec == expected {
                        return true;
                    }
                    actual_vec.rotate_left(1);
                }

                false
            }

            fn should_not_have_negative_cycle(self) {
                let neg_cycle = self.graph.find_negative_cycle(false);
                assert!(
                    matches!(neg_cycle, NegativeCycleResult::NoCycle),
                    "Expected no negative cycle, but found {:?}",
                    neg_cycle
                );
            }
        }
    }

    mod solver {
        use super::*;

        #[test]
        fn test_empty_solver() {
            // Build a solver which is not empty.
            let mut solver = DifferenceLogicSolver::new(true);
            solver.add_constraint(false, &RelOp::Le, &sub_expr("x", "y"), &num_expr(-1), 1);
            solver.add_constraint(false, &RelOp::Le, &sub_expr("y", "x"), &num_expr(-1), 2);
            solver.check();
            solver.push();

            // Check that the empty solver is indeed not empty.
            assert!(!solver.clause_set.is_empty());
            assert!(!solver.stack.is_empty());
            assert!(!solver.unsat_core.is_none());
            assert!(!solver.result_cache.is_empty());

            // Create an empty solver from it.
            let empty_solver = solver.empty_solver();

            // Check that the empty solver is indeed empty.
            assert!(empty_solver.unsat_core_extraction);
            assert!(empty_solver.clause_set.is_empty());
            assert!(empty_solver.stack.is_empty());
            assert!(empty_solver.unsat_core.is_none());
            // Cache is copied.
            assert!(!empty_solver.result_cache.is_empty());
        }

        #[test]
        fn test_empty_is_sat() {
            SolverTest::new(true)
                .should_be_sat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_simple_sat() {
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(0), 1)
                .should_be_sat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_simple_sat_with_binary_clause() {
            SolverTest::new(true)
                .add_constraint(RelOp::Ge, abs_sub_expr("x", "y"), num_expr(0), 1)
                .should_be_sat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_simple_unsat() {
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
                .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 2]));
        }

        #[test]
        fn test_simple_unsat_with_binary_clause() {
            SolverTest::new(true)
                .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(1), 1)
                .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(1), 2)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 2]));
        }

        #[test]
        #[should_panic]
        fn test_simple_unsat_with_binary_clause_no_core() {
            SolverTest::new(false)
                .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(1), 1)
                .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(1), 2)
                .should_be_unsat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_two_cycles_returns_correct_core() {
            // The graph for this problem contains two cycles which both contain edge u -> v.
            // One positive cycle: u -> v -> a -> u
            // One negative cycle: u -> v -> x -> u
            // The unsat core should contain the negative cycle.
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("u", "v"), num_expr(0), 1)
                .add_constraint(RelOp::Le, sub_expr("v", "a"), num_expr(1), 2)
                .add_constraint(RelOp::Le, sub_expr("a", "u"), num_expr(1), 3)
                .add_constraint(RelOp::Le, sub_expr("v", "x"), num_expr(-1), 4)
                .add_constraint(RelOp::Le, sub_expr("x", "u"), num_expr(-1), 5)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 4, 5]));
        }

        #[test]
        fn test_duplicate_edge_one_positive() {
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(0), 1)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(3), 2)
                .add_constraint(RelOp::Le, sub_expr("y", "z"), num_expr(-1), 3)
                .add_constraint(RelOp::Le, sub_expr("z", "x"), num_expr(-1), 4)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 3, 4]));
        }

        #[test]
        fn test_push_pop() {
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
                .should_be_sat()
                .should_have_unsat_core(None)
                .push()
                .should_be_sat()
                .should_have_unsat_core(None)
                .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 2]))
                .pop()
                .should_be_sat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_cse3220_unsat() {
            // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x1", "x3"), num_expr(-6), 1)
                .add_constraint(RelOp::Le, sub_expr("x1", "x4"), num_expr(-3), 2)
                .add_constraint(RelOp::Le, sub_expr("x2", "x1"), num_expr(3), 3)
                .add_constraint(RelOp::Le, sub_expr("x3", "x2"), num_expr(2), 4)
                .add_constraint(RelOp::Le, sub_expr("x3", "x4"), num_expr(-1), 5)
                .add_constraint(RelOp::Le, sub_expr("x4", "x2"), num_expr(5), 6)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 3, 4]));
        }

        #[test]
        fn test_cse3220_sat() {
            // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
            SolverTest::new(true)
                .add_constraint(RelOp::Le, sub_expr("x1", "x3"), num_expr(-5), 1)
                .add_constraint(RelOp::Le, sub_expr("x1", "x4"), num_expr(-3), 2)
                .add_constraint(RelOp::Le, sub_expr("x2", "x1"), num_expr(3), 3)
                .add_constraint(RelOp::Le, sub_expr("x3", "x2"), num_expr(2), 4)
                .add_constraint(RelOp::Le, sub_expr("x3", "x4"), num_expr(-1), 5)
                .add_constraint(RelOp::Le, sub_expr("x4", "x2"), num_expr(5), 6)
                .should_be_sat()
                .should_have_unsat_core(None);
        }

        #[test]
        fn test_cached_unsat_core() {
            SolverTest::new(true)
                .push()
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
                .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![1, 2]))
                .pop()
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 3)
                .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 4)
                .should_be_unsat()
                .should_have_unsat_core(Some(vec![3, 4]));
        }

        #[test]
        #[should_panic]
        fn test_pop_on_empty_panics() {
            let mut solver = DifferenceLogicSolver::new(false);
            solver.pop();
        }

        #[test]
        #[should_panic]
        fn test_extract_unsat_core_panics_when_not_enabled() {
            SolverTest::new(false)
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
                .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
                .should_be_unsat()
                .should_have_unsat_core(None);
        }

        struct SolverTest {
            solver: DifferenceLogicSolver,
        }

        impl SolverTest {
            fn new(unsat_core_extraction: bool) -> Self {
                SolverTest {
                    solver: DifferenceLogicSolver::new(unsat_core_extraction),
                }
            }

            fn add_constraint(mut self, op: RelOp, left: AExpr, right: AExpr, id: usize) -> Self {
                self.solver.add_constraint(false, &op, &left, &right, id);
                self
            }

            fn should_be_sat(mut self) -> Self {
                assert!(
                    self.solver.check(),
                    "Expected DL constraints to be SAT, but they are UNSAT"
                );
                self
            }

            fn should_be_unsat(mut self) -> Self {
                assert!(
                    !self.solver.check(),
                    "Expected DL constraints to be UNSAT, but they are SAT"
                );
                self
            }

            fn should_have_unsat_core(self, expected: Option<Vec<usize>>) -> Self {
                let unsat_core = self.solver.extract_unsat_core();
                assert_eq!(
                    unsat_core, expected,
                    "Expected unsat core {:?}, but got {:?}",
                    expected, unsat_core
                );
                self
            }

            fn push(mut self) -> Self {
                self.solver.push();
                self
            }

            fn pop(mut self) -> Self {
                self.solver.pop();
                self
            }
        }
    }

    mod solver_supports {
        use crate::formula::{Expr, Interval};
        use rstest::rstest;

        use super::*;

        #[test]
        fn supports_empty_formula() {
            assert_supported(true, vec![]);
        }

        #[rstest]
        #[case(Formula::prop(Expr::bool("x".into())))]
        #[case(Formula::prop(Expr::true_expr()))]
        #[case(Formula::prop(Expr::false_expr()))]
        #[case(Formula::and(vec![diff_constraint()]))]
        #[case(Formula::or(vec![diff_constraint()]))]
        #[case(Formula::not(diff_constraint()))]
        #[case(Formula::imply(diff_constraint(), diff_constraint()))]
        #[case(Formula::g(interval(), diff_constraint()))]
        #[case(Formula::f(interval(), diff_constraint()))]
        #[case(Formula::u(interval(), diff_constraint(), diff_constraint()))]
        #[case(Formula::r(interval(), diff_constraint(), diff_constraint()))]
        fn simple_supported(#[case] formula: Formula) {
            assert_supported(true, vec![formula]);
        }

        #[rstest]
        #[case(three_variables())]
        #[case(Formula::and(vec![three_variables()]))]
        #[case(Formula::or(vec![three_variables()]))]
        #[case(Formula::not(three_variables()))]
        #[case(Formula::imply(three_variables(), diff_constraint()))]
        #[case(Formula::imply(diff_constraint(), three_variables()))]
        #[case(Formula::g(interval(), three_variables()))]
        #[case(Formula::f(interval(), three_variables()))]
        #[case(Formula::u(interval(), three_variables(), diff_constraint()))]
        #[case(Formula::u(interval(), diff_constraint(), three_variables()))]
        #[case(Formula::r(interval(), three_variables(), diff_constraint()))]
        #[case(Formula::r(interval(), diff_constraint(), three_variables()))]
        fn simple_unsupported(#[case] formula: Formula) {
            assert_supported(false, vec![formula]);
        }

        fn assert_supported(supported: bool, formulas: Vec<Formula>) {
            assert_eq!(
                supported,
                DifferenceLogicSolver::supports(&Node::from_operands(
                    formulas.into_iter().map(|f| f.into()).collect(),
                ))
            );
        }

        fn diff_constraint() -> Formula {
            Formula::prop(Expr::real(RelOp::Le, sub_expr("x", "y"), num_expr(1)))
        }

        fn three_variables() -> Formula {
            Formula::prop(Expr::real(RelOp::Le, sub_expr("x", "y"), var_expr("z")))
        }

        fn interval() -> Interval {
            Interval {
                lower: 0,
                upper: 42,
            }
        }
    }

    mod clause_set {
        use super::*;

        #[test]
        fn empty_clause_set_is_empty() {
            ClauseSetTest::new("x <= y").should_be_empty();
        }

        #[test]
        fn non_empty_clause_set_is_not_empty() {
            ClauseSetTest::new("x <= y")
                .add_constraint(RelOp::Le, var_expr("x"), var_expr("y"), 1)
                .should_not_be_empty();
        }

        #[test]
        fn var_eq_var() {
            ClauseSetTest::new("(x = y)  ==>  (x - y <= 0 && y - x <= 0)")
                .add_constraint(RelOp::Eq, var_expr("x"), var_expr("y"), 1)
                .should_contain_unary_constraint("x", "y", 0, false, 1)
                .should_contain_unary_constraint("y", "x", 0, false, 1)
                .should_have_clause_count(2);
        }

        #[test]
        fn var_eq_num() {
            ClauseSetTest::new("(x = 5)  ==>  (x - 0 <= 5 && 0 - x <= -5)")
                .add_constraint(RelOp::Eq, var_expr("x"), num_expr(5), 1)
                .should_contain_unary_constraint("x", "__dl_zero", 5, false, 1)
                .should_contain_unary_constraint("__dl_zero", "x", -5, false, 1)
                .should_have_clause_count(2);
        }

        #[test]
        fn var_ne_num() {
            ClauseSetTest::new(
                "(x != 3)  ==>  (x < 3 || x > 3)  ==>  (x - 0 <= 3 - ε || 0 - x <= -3 - ε)",
            )
            .add_constraint(RelOp::Ne, var_expr("x"), num_expr(3), 1)
            .should_contain_binary_constraint(
                ("x", "__dl_zero", 3, true, 1),
                ("__dl_zero", "x", -3, true, 1),
            )
            .should_have_clause_count(1);
        }

        #[test]
        fn var_ne_var() {
            ClauseSetTest::new(
                "(x != y)  ==>  (x < y || x > y)  ==>  (x - y <= 0 - ε || y - x <= 0 - ε)",
            )
            .add_constraint(RelOp::Ne, var_expr("x"), var_expr("y"), 1)
            .should_contain_binary_constraint(("x", "y", 0, true, 1), ("y", "x", 0, true, 1))
            .should_have_clause_count(1);
        }

        #[test]
        fn abs_le_num() {
            ClauseSetTest::new(
                "(|x| <= 2)  ==>  (x <= 2 && x >= -2)  ==>  (x - 0 <= 2 && 0 - x <= 2)",
            )
            .add_constraint(RelOp::Le, abs_expr("x"), num_expr(2), 1)
            .should_contain_unary_constraint("x", "__dl_zero", 2, false, 1)
            .should_contain_unary_constraint("__dl_zero", "x", 2, false, 1)
            .should_have_clause_count(2);
        }

        #[test]
        fn abs_lt_num() {
            ClauseSetTest::new(
                "(|x| < 4)  ==>  (x < 4 && x > -4)  ==>  (x - 0 <= 4 - ε && 0 - x <= 4 - ε)",
            )
            .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(4), 1)
            .should_contain_unary_constraint("x", "__dl_zero", 4, true, 1)
            .should_contain_unary_constraint("__dl_zero", "x", 4, true, 1)
            .should_have_clause_count(2);
        }

        #[test]
        fn abs_gt_num() {
            ClauseSetTest::new(
                "(|x| > 3)  ==>  (x > 3 || x < -3)  ==>  (0 - x <= -3 - ε || x - 0 <= -3 - ε)",
            )
            .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(3), 1)
            .should_contain_binary_constraint(
                ("__dl_zero", "x", -3, true, 1),
                ("x", "__dl_zero", -3, true, 1),
            )
            .should_have_clause_count(1);
        }

        #[test]
        fn abs_ge_num() {
            ClauseSetTest::new(
                "(|x| >= 5)  ==>  (x >= 5 || x <= -5)  ==>  (0 - x <= -5 || x - 0 <= -5)",
            )
            .add_constraint(RelOp::Ge, abs_expr("x"), num_expr(5), 1)
            .should_contain_binary_constraint(
                ("__dl_zero", "x", -5, false, 1),
                ("x", "__dl_zero", -5, false, 1),
            )
            .should_have_clause_count(1);
        }

        #[test]
        fn var_le_num() {
            ClauseSetTest::new("(x <= 10)  ==>  (x - 0 <= 10)")
                .add_constraint(RelOp::Le, var_expr("x"), num_expr(10), 1)
                .should_contain_unary_constraint("x", "__dl_zero", 10, false, 1)
                .should_have_clause_count(1);
        }

        #[test]
        #[should_panic]
        fn abs_ne_num() {
            ClauseSetTest::new("|x| != 9  ==> panic").add_constraint(
                RelOp::Ne,
                abs_expr("x"),
                num_expr(9),
                1,
            );
        }

        #[test]
        #[should_panic]
        fn abs_lt_var() {
            ClauseSetTest::new("|x| < y  ==> panic").add_constraint(
                RelOp::Lt,
                abs_expr("x"),
                var_expr("y"),
                1,
            );
        }

        #[test]
        #[should_panic]
        fn abs_le_var() {
            ClauseSetTest::new("|x| <= y  ==> panic").add_constraint(
                RelOp::Le,
                abs_expr("x"),
                var_expr("y"),
                1,
            );
        }

        #[test]
        #[should_panic]
        fn abs_gt_var() {
            ClauseSetTest::new("|x| > y  ==> panic").add_constraint(
                RelOp::Gt,
                abs_expr("x"),
                var_expr("y"),
                1,
            );
        }

        #[test]
        #[should_panic]
        fn abs_ge_var() {
            ClauseSetTest::new("|x| >= y  ==> panic").add_constraint(
                RelOp::Ge,
                abs_expr("x"),
                var_expr("y"),
                1,
            );
        }

        #[test]
        fn sub_le_num() {
            ClauseSetTest::new("(x - y <= 7)  ==>  (x - y <= 7)")
                .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(7), 1)
                .should_contain_unary_constraint("x", "y", 7, false, 1)
                .should_have_clause_count(1);
        }

        #[test]
        fn abs_sub_le_num() {
            ClauseSetTest::new("(|x - y| <= 4)  ==>  (x - y <= 4 && y - x <= 4)")
                .add_constraint(RelOp::Le, abs_sub_expr("x", "y"), num_expr(4), 1)
                .should_contain_unary_constraint("x", "y", 4, false, 1)
                .should_contain_unary_constraint("y", "x", 4, false, 1)
                .should_have_clause_count(2);
        }

        #[test]
        fn abs_sub_gt_num() {
            ClauseSetTest::new("(|x - y| > 12)  ==>  (x - y > 12 || x - y < -12)  ==>  (y - x <= -12 - ε || x - y <= -12 - ε)")
                .add_constraint(RelOp::Gt, abs_sub_expr("x", "y"), num_expr(12), 1)
                .should_contain_binary_constraint(
                    ("y", "x", -12, true, 1),
                    ("x", "y", -12, true, 1,)
                )
                .should_have_clause_count(1);
        }

        #[test]
        fn sub_ne_num() {
            ClauseSetTest::new("(x - y != 18)  ==>  (x - y < 18 || x - y > 18)  ==>  (x - y <= 18 - ε || y - x <= -18 - ε)")
                .add_constraint(RelOp::Ne, sub_expr("x", "y"), num_expr(18), 1)
                .should_contain_binary_constraint(
                    ("x", "y", 18, true, 1),
                    ("y", "x", -18, true, 1),
                )
                .should_have_clause_count(1);
        }

        #[test]
        fn complex_constraint_mix() {
            ClauseSetTest::new("Complex mix of constraints")
                .add_constraint(RelOp::Eq, var_expr("x"), num_expr(2), 1)
                .add_constraint(RelOp::Ne, var_expr("y"), num_expr(4), 2)
                .add_constraint(RelOp::Lt, abs_expr("z"), num_expr(8), 3)
                .should_contain_unary_constraint("x", "__dl_zero", 2, false, 1)
                .should_contain_unary_constraint("__dl_zero", "x", -2, false, 1)
                .should_contain_binary_constraint(
                    ("y", "__dl_zero", 4, true, 2),
                    ("__dl_zero", "y", -4, true, 2),
                )
                .should_contain_unary_constraint("z", "__dl_zero", 8, true, 3)
                .should_have_clause_count(5);
        }

        #[test]
        fn same_constraint() {
            ClauseSetTest::new("Same constraint is stored only once")
                .add_constraint(RelOp::Le, sub_expr("a", "b"), num_expr(10), 1)
                .add_constraint(RelOp::Le, sub_expr("a", "b"), num_expr(10), 1)
                .should_contain_unary_constraint("a", "b", 10, false, 1)
                .should_have_clause_count(1);
        }

        struct ClauseSetTest {
            description: String,
            clause_set: ClauseSet,
        }

        impl ClauseSetTest {
            fn new(description: &str) -> Self {
                Self {
                    description: description.to_string(),
                    clause_set: ClauseSet::new(),
                }
            }

            fn add_constraint(mut self, op: RelOp, left: AExpr, right: AExpr, id: usize) -> Self {
                let constraint = NormalizedConstraint::new(false, &op, &left, &right, Some(id));
                self.clause_set.add_constraint(&constraint).unwrap();
                self
            }

            fn should_be_empty(self) -> Self {
                assert!(
                    self.clause_set.is_empty(),
                    "Expected clause set '{}' to be empty, but it was {:?}",
                    self.description,
                    self.clause_set.len()
                );
                self
            }

            fn should_not_be_empty(self) -> Self {
                assert!(
                    !self.clause_set.is_empty(),
                    "Expected clause set '{}' to be non-empty, but it was empty",
                    self.description
                );
                self
            }

            fn should_contain_unary_constraint(
                self,
                x: &str,
                y: &str,
                c: i64,
                strict: bool,
                id: usize,
            ) -> Self {
                let expected = DifferenceConstraint {
                    x: x.into(),
                    y: y.into(),
                    c: Ratio::from_integer(c),
                    strict: strict,
                    id: Some(id),
                };

                let found = self
                    .clause_set
                    .unary
                    .iter()
                    .any(|clause| clause.0 == expected);

                assert!(
                    found,
                    "Expected unary constraint ({}, {} - {} <= {}{}) in '{}', but it was not found.\nActual clauses: {:?}",
                    id,
                    x,
                    y,
                    c,
                    if strict { " - ε" } else { "" },
                    self.description,
                    self.clause_set.unary
                );
                self
            }

            fn should_contain_binary_constraint(
                self,
                expected1: (&str, &str, i64, bool, usize),
                expected2: (&str, &str, i64, bool, usize),
            ) -> Self {
                let constraint1 = DifferenceConstraint {
                    x: expected1.0.into(),
                    y: expected1.1.into(),
                    c: Ratio::from_integer(expected1.2),
                    strict: expected1.3,
                    id: Some(expected1.4),
                };

                let constraint2 = DifferenceConstraint {
                    x: expected2.0.into(),
                    y: expected2.1.into(),
                    c: Ratio::from_integer(expected2.2),
                    strict: expected2.3,
                    id: Some(expected2.4),
                };

                let found = self
                    .clause_set
                    .binary
                    .iter()
                    .any(|clause| clause.0 == constraint1 && clause.1 == constraint2);

                assert!(
                    found,
                    "Expected binary clause with constraints ({}, {} - {} <= {}{}) and ({}, {} - {} <= {}{}) in '{}', but it was not found.\nActual clauses: {:?}",
                    expected1.4,
                    expected1.0,
                    expected1.1,
                    expected1.2,
                    if expected1.3 { " - ε" } else { "" },
                    expected2.4,
                    expected2.0,
                    expected2.1,
                    expected2.2,
                    if expected2.3 { " - ε" } else { "" },
                    self.description,
                    self.clause_set.binary
                );
                self
            }

            fn should_have_clause_count(self, expected_count: usize) -> Self {
                let actual = self.clause_set.len();
                assert_eq!(
                    actual,
                    expected_count,
                    "Expected exactly {} clauses in '{}', got {}\nActual clauses: {:?} and {:?}",
                    expected_count,
                    self.description,
                    actual,
                    self.clause_set.unary,
                    self.clause_set.binary
                );
                self
            }
        }
    }

    mod constraint {
        use super::*;

        #[test]
        fn sub_le_num() {
            ConstraintTest::new("(x - y <= 5)  ==>  (x - y <= 5)")
                .input(sub_expr("x", "y"), RelOp::Le, num_expr(5), 1)
                .should_become("x", "y", 5, false);
        }

        #[test]
        fn sub_lt_num() {
            ConstraintTest::new("(x - y < 4)  ==>  (x - y <= 4 - ε)")
                .input(sub_expr("x", "y"), RelOp::Lt, num_expr(4), 2)
                .should_become("x", "y", 4, true);
        }

        #[test]
        fn sub_ge_num() {
            ConstraintTest::new("(x - y >= -3)  ==>  (y - x <= 3)")
                .input(sub_expr("x", "y"), RelOp::Ge, num_expr(-3), 3)
                .should_become("y", "x", 3, false);
        }

        #[test]
        fn sub_gt_num() {
            ConstraintTest::new("(x - y > -2)  ==>  (y - x <= 2 - ε)")
                .input(sub_expr("x", "y"), RelOp::Gt, num_expr(-2), 4)
                .should_become("y", "x", 2, true);
        }

        #[test]
        fn num_le_sub() {
            ConstraintTest::new("(5 <= x - y)  ==>  (y - x <= -5)")
                .input(num_expr(5), RelOp::Le, sub_expr("x", "y"), 5)
                .should_become("y", "x", -5, false);
        }

        #[test]
        fn num_lt_sub() {
            ConstraintTest::new("(4 < x - y)  ==>  (y - x <= -4 - ε)")
                .input(num_expr(4), RelOp::Lt, sub_expr("x", "y"), 6)
                .should_become("y", "x", -4, true);
        }

        #[test]
        fn num_ge_sub() {
            ConstraintTest::new("(-3 >= x - y)  ==>  (x - y <= -3)")
                .input(num_expr(-3), RelOp::Ge, sub_expr("x", "y"), 7)
                .should_become("x", "y", -3, false);
        }

        #[test]
        fn num_gt_sub() {
            ConstraintTest::new("(-2 > x - y)  ==>  (x - y <= -2 - ε)")
                .input(num_expr(-2), RelOp::Gt, sub_expr("x", "y"), 8)
                .should_become("x", "y", -2, true);
        }

        #[test]
        fn var_le_num() {
            ConstraintTest::new("(x <= 5)  ==>  (x - 0 <= 5)")
                .input(var_expr("x"), RelOp::Le, num_expr(5), 9)
                .should_become("x", "__dl_zero", 5, false);
        }

        #[test]
        fn var_lt_num() {
            ConstraintTest::new("(x < 5)  ==>  (x - 0 <= 5 - ε)")
                .input(var_expr("x"), RelOp::Lt, num_expr(5), 10)
                .should_become("x", "__dl_zero", 5, true);
        }

        #[test]
        fn var_ge_num() {
            ConstraintTest::new("(x >= -5)  ==>  (0 - x <= 5)")
                .input(var_expr("x"), RelOp::Ge, num_expr(-5), 11)
                .should_become("__dl_zero", "x", 5, false);
        }

        #[test]
        fn var_gt_num() {
            ConstraintTest::new("(x > -5)  ==>  (0 - x <= 5 - ε)")
                .input(var_expr("x"), RelOp::Gt, num_expr(-5), 12)
                .should_become("__dl_zero", "x", 5, true);
        }

        #[test]
        fn num_le_var() {
            ConstraintTest::new("(5 <= x)  ==>  (0 - x <= -5)")
                .input(num_expr(5), RelOp::Le, var_expr("x"), 13)
                .should_become("__dl_zero", "x", -5, false);
        }

        #[test]
        fn num_lt_var() {
            ConstraintTest::new("(5 < x)  ==>  (0 - x <= -5 - ε)")
                .input(num_expr(5), RelOp::Lt, var_expr("x"), 14)
                .should_become("__dl_zero", "x", -5, true);
        }

        #[test]
        fn num_ge_var() {
            ConstraintTest::new("(5 >= x)  ==>  (x - 0 <= 5)")
                .input(num_expr(5), RelOp::Ge, var_expr("x"), 15)
                .should_become("x", "__dl_zero", 5, false);
        }

        #[test]
        fn num_gt_var() {
            ConstraintTest::new("(-4 > x)  ==>  (x - 0 <= -4 - ε)")
                .input(num_expr(-4), RelOp::Gt, var_expr("x"), 16)
                .should_become("x", "__dl_zero", -4, true);
        }

        #[test]
        fn var_le_var() {
            ConstraintTest::new("(x <= y)  ==>  (x - y <= 0)")
                .input(var_expr("x"), RelOp::Le, var_expr("y"), 17)
                .should_become("x", "y", 0, false);
        }

        #[test]
        fn var_lt_var() {
            ConstraintTest::new("(x < y)  ==>  (x - y <= 0 - ε)")
                .input(var_expr("x"), RelOp::Lt, var_expr("y"), 18)
                .should_become("x", "y", 0, true);
        }

        #[test]
        fn var_ge_var() {
            ConstraintTest::new("(x >= y)  ==>  (y - x <= 0)")
                .input(var_expr("x"), RelOp::Ge, var_expr("y"), 19)
                .should_become("y", "x", 0, false);
        }

        #[test]
        fn var_gt_var() {
            ConstraintTest::new("(x > y)  ==>  (y - x <= 0 - ε)")
                .input(var_expr("x"), RelOp::Gt, var_expr("y"), 20)
                .should_become("y", "x", 0, true);
        }

        #[test]
        fn invalid_sub_expression() {
            assert_eq!(
                NormalizedConstraint::new(
                    false,
                    &RelOp::Le,
                    &AExpr::BinOp {
                        op: ArithOp::Sub,
                        left: var_expr("x").into(),
                        right: num_expr(1).into(),
                    },
                    &num_expr(5),
                    None
                )
                .to_diff(),
                Err(DifferenceConstraintError::InvalidSubExpression)
            );
        }

        #[test]
        fn invalid_expression() {
            assert_eq!(
                NormalizedConstraint::new(
                    false,
                    &RelOp::Le,
                    &AExpr::BinOp {
                        op: ArithOp::Add, // Note this is addition, not subtraction
                        left: var_expr("x").into(),
                        right: num_expr(1).into(),
                    },
                    &num_expr(5),
                    None
                )
                .to_diff(),
                Err(DifferenceConstraintError::InvalidExpression)
            );
        }

        #[test]
        fn invalid_relation() {
            assert_eq!(
                NormalizedConstraint::new(false, &RelOp::Ne, &var_expr("x"), &var_expr("y"), None)
                    .to_diff(),
                Err(DifferenceConstraintError::InvalidRelation)
            );
        }

        struct ConstraintTest {
            description: String,
            input: Option<DifferenceConstraint>,
            id: Option<usize>,
        }

        impl ConstraintTest {
            fn new(description: &str) -> Self {
                Self {
                    description: description.to_string(),
                    input: None,
                    id: None,
                }
            }

            fn input(&self, left: AExpr, op: RelOp, right: AExpr, id: usize) -> Self {
                Self {
                    description: self.description.clone(),
                    input: Some(
                        NormalizedConstraint::new(false, &op, &left, &right, Some(id))
                            .to_diff()
                            .unwrap(),
                    ),
                    id: Some(id),
                }
            }

            fn should_become(&self, x: &str, y: &str, c: i64, strict: bool) {
                let constraint = self.input.clone().unwrap();

                assert_eq!(
                    constraint.x,
                    x.into(),
                    "Wrong x in test case \"{}\"",
                    self.description
                );
                assert_eq!(
                    constraint.y,
                    y.into(),
                    "Wrong y in test case \"{}\"",
                    self.description
                );
                assert_eq!(
                    constraint.c,
                    Ratio::from_integer(c),
                    "Wrong c in test case \"{}\"",
                    self.description
                );
                assert_eq!(
                    constraint.strict, strict,
                    "Wrong strict in test case \"{}\"",
                    self.description
                );
                assert_eq!(
                    constraint.id, self.id,
                    "Wrong id in test case \"{}\"",
                    self.description
                );
            }
        }
    }

    mod normalized_constraint {
        use super::*;
        use rstest::rstest;

        #[test]
        fn test_already_normalized() {
            let constraint =
                NormalizedConstraint::new(false, &RelOp::Le, &var_expr("x"), &num_expr(5), Some(1));

            assert_eq!(constraint.op, RelOp::Le);
            assert_eq!(constraint.left, var_expr("x"));
            assert_eq!(constraint.right, num_expr(5));
            assert_eq!(constraint.id, Some(1));
        }

        #[rstest]
        #[case(RelOp::Le, RelOp::Gt)]
        #[case(RelOp::Lt, RelOp::Ge)]
        #[case(RelOp::Ge, RelOp::Lt)]
        #[case(RelOp::Gt, RelOp::Le)]
        #[case(RelOp::Eq, RelOp::Ne)]
        #[case(RelOp::Ne, RelOp::Eq)]
        fn test_negation(#[case] input_op: RelOp, #[case] expected_op: RelOp) {
            let constraint =
                NormalizedConstraint::new(true, &input_op, &var_expr("x"), &num_expr(5), Some(1));

            assert_eq!(constraint.op, expected_op);
            assert_eq!(constraint.left, var_expr("x"));
            assert_eq!(constraint.right, num_expr(5));
            assert_eq!(constraint.id, Some(1));
        }

        #[rstest]
        #[case(RelOp::Le, RelOp::Ge)]
        #[case(RelOp::Lt, RelOp::Gt)]
        #[case(RelOp::Ge, RelOp::Le)]
        #[case(RelOp::Gt, RelOp::Lt)]
        #[case(RelOp::Eq, RelOp::Eq)]
        #[case(RelOp::Ne, RelOp::Ne)]
        fn test_normalization(#[case] input_op: RelOp, #[case] expected_op: RelOp) {
            let constraint =
                NormalizedConstraint::new(false, &input_op, &num_expr(5), &var_expr("x"), Some(1));

            assert_eq!(constraint.op, expected_op);
            assert_eq!(constraint.left, var_expr("x"));
            assert_eq!(constraint.right, num_expr(5));
            assert_eq!(constraint.id, Some(1));
        }
    }

    fn sub_expr(x: &str, y: &str) -> AExpr {
        AExpr::BinOp {
            op: ArithOp::Sub,
            left: Box::new(AExpr::Var(x.into())),
            right: Box::new(AExpr::Var(y.into())),
        }
    }

    fn abs_expr(x: &str) -> AExpr {
        AExpr::Abs(Box::new(AExpr::Var(x.into())))
    }

    fn abs_sub_expr(x: &str, y: &str) -> AExpr {
        AExpr::Abs(Box::new(AExpr::BinOp {
            op: ArithOp::Sub,
            left: Box::new(AExpr::Var(x.into())),
            right: Box::new(AExpr::Var(y.into())),
        }))
    }

    fn num_expr(n: i64) -> AExpr {
        AExpr::Num(Ratio::from_integer(n))
    }

    fn var_expr(x: &str) -> AExpr {
        AExpr::Var(x.into())
    }
}
