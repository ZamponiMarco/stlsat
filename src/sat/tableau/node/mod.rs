use crate::{
    formula::{
        Formula, Interval,
        transform::{
            FlatTransformer, FormulaSimplifier, NegationNormalFormTransformer,
            RecursiveFormulaTransformer, STLTransformer, ShiftBoundsTransformer,
        },
    },
    util::join_with,
};
use std::{
    collections::HashSet,
    fmt::{self, Display},
    sync::atomic::{AtomicUsize, Ordering},
};

pub mod decompose;
pub mod rewrite;

pub static NODE_FORMULA_ID: AtomicUsize = AtomicUsize::new(0);
pub static NODE_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Eq)]
pub struct NodeFormula {
    pub kind: Formula,
    pub marked: bool,
    pub parent_id: Option<usize>,
    pub id: usize,
}

impl From<Formula> for NodeFormula {
    fn from(kind: Formula) -> Self {
        Self {
            kind,
            marked: false,
            parent_id: None,
            id: NODE_FORMULA_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl NodeFormula {
    pub fn with_kind(mut self, kind: Formula) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_marked(mut self, marked: bool) -> Self {
        self.marked = marked;
        self
    }

    pub fn with_parent_id(mut self, parent_id: Option<usize>) -> Self {
        self.parent_id = parent_id;
        self
    }

    #[must_use]
    pub fn is_active_at(&self, current_time: i32) -> bool {
        match &self.kind.get_interval() {
            Some(interval) => interval.active(current_time),
            _ => true,
        }
    }

    #[must_use]
    pub fn is_parent_active_in(&self, node: &Node) -> bool {
        match self.parent_id {
            None => false,
            Some(id) => node.operands.iter().any(|f| f.id == id),
        }
    }
}

impl PartialEq for NodeFormula {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.parent_id == other.parent_id && self.marked == other.marked
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Node {
    pub operands: Vec<NodeFormula>,
    pub current_time: i32,
    pub implies: Option<Vec<usize>>,
    pub id: usize,
}

impl Node {
    pub fn from_operands(operands: Vec<NodeFormula>) -> Self {
        Self {
            operands,
            current_time: 0,
            implies: None,
            id: NODE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn is_poised(&self) -> bool {
        self.operands.iter().all(|f| {
            matches!(f.kind, Formula::Prop(_) | Formula::Not(_))
                || f.marked
                || !f.is_active_at(self.current_time)
        })
    }

    #[must_use]
    pub fn to_formula(&self) -> Formula {
        if self.operands.len() == 1 {
            self.operands[0].clone().kind
        } else {
            Formula::And(self.operands.iter().map(|f| f.kind.clone()).collect())
        }
    }

    pub fn mltl_rewrite(&mut self) {
        self.operands.iter_mut().for_each(|f| {
            f.kind = STLTransformer.visit(&f.kind);
        });
    }

    pub fn negative_normal_form_rewrite(&mut self) {
        self.operands.iter_mut().for_each(|f| {
            f.kind = NegationNormalFormTransformer.visit(&f.kind);
        });
    }

    pub fn flatten(&mut self) {
        let mut flattened: Vec<NodeFormula> = Vec::new();
        for f in &self.operands {
            let flat = FlatTransformer.visit(&f.kind);
            if let Formula::And(ops) = &flat {
                flattened.extend(ops.iter().cloned().map(NodeFormula::from));
            } else {
                flattened.push(NodeFormula::from(flat));
            }
        }
        self.operands = flattened;
    }

    pub fn shift_bounds(&mut self) {
        self.operands.iter_mut().for_each(|f| {
            f.kind = ShiftBoundsTransformer.visit(&f.kind);
        });
    }

    pub fn simplify(&mut self) {
        self.operands.iter_mut().for_each(|f| {
            f.kind = FormulaSimplifier.visit(&f.kind);
        });
    }

    fn compute_target_set(&self) -> HashSet<Interval> {
        let mut targets = HashSet::new();

        for operand in &self.operands {
            if !operand.marked {
                continue;
            }

            match &operand.kind {
                Formula::U { right, .. } => {
                    targets.extend(right.proposition_start_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::R { left, .. } => {
                    targets.extend(left.proposition_end_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::F { phi, .. } => {
                    targets.extend(phi.proposition_start_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                _ => {}
            }
        }
        targets
    }

    fn compute_obstacle_set(&self) -> HashSet<Interval> {
        let mut obstacles = HashSet::new();

        for operand in &self.operands {
            if operand.is_parent_active_in(self) {
                continue;
            }

            match &operand.kind {
                Formula::R {
                    right: phi_1,
                    interval,
                    ..
                }
                | Formula::U {
                    interval,
                    left: phi_1,
                    ..
                } => {
                    obstacles.extend(phi_1.proposition_end_interval(interval.clone()));
                }
                Formula::G { interval, phi } => {
                    obstacles.extend(phi.proposition_end_interval(Interval {
                        lower: interval.upper,
                        upper: interval.upper,
                    }));
                }
                Formula::Prop(_) | Formula::Not(_) => {
                    obstacles.insert(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    });
                }
                _ => {}
            }
        }

        obstacles
    }

    fn compute_n(&self) -> HashSet<Interval> {
        let mut n_set = HashSet::new();

        for operand in &self.operands {
            if !operand.marked {
                continue;
            }

            if operand.is_parent_active_in(self) {
                continue;
            }

            match &operand.kind {
                Formula::U { left, .. } => {
                    n_set.extend(left.proposition_end_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::R { right, .. } => {
                    n_set.extend(right.proposition_end_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::G { phi, .. } => {
                    n_set.extend(phi.proposition_end_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                _ => {}
            }
        }
        n_set
    }

    fn compute_o(&self) -> HashSet<Interval> {
        let mut o_set = HashSet::new();

        for operand in &self.operands {
            if operand.is_parent_active_in(self) {
                continue;
            }

            o_set.extend(operand.kind.proposition_end_interval(Interval { lower: 0, upper: 0 }));
        }

        o_set
    }

    pub fn calculate_k_star(&self, max_jump: i32) -> i32 {
        if max_jump <= 1 {
            return 1;
        }

        let target_starts = self.compute_target_set();
        let invariant_ends = self.compute_obstacle_set();
        //println!("M: {:?}, S: {:?}", target_starts, invariant_ends);

        let active_invariant_ends = self.compute_n();
        let invariant_starts = self.compute_o();
        //println!("N: {:?}, O: {:?}", active_invariant_ends, invariant_starts);

        let condition_step_complete = target_starts.iter().any(|t| {
            invariant_ends
                .iter()
                .any(|o| o.lower <= t.upper && t.upper <= o.upper)
        });

        let condition_step_sound = active_invariant_ends
            .iter()
            .any(|n| invariant_starts.iter().any(|o| n.intersects(o)));

        if condition_step_complete || condition_step_sound {
            return 1;
        }

        let jump_complete = target_starts
            .iter()
            .flat_map(|t| invariant_ends.iter().map(move |o| o.lower - t.upper + 1))
            .filter(|&k| k >= 1)
            .min()
            .unwrap_or(max_jump);

        let jump_sound = active_invariant_ends
            .iter()
            .flat_map(|n| invariant_starts.iter().map(move |o| o.lower - n.upper))
            .filter(|&k| k >= 1)
            .min()
            .unwrap_or(max_jump);

        //println!("Node {}: max_jump = {:?}, jump_complete = {:?}, jump_sound = {:?}, selected jump = {}", self.id, max_jump, jump_complete, jump_sound, jump_complete.min(jump_sound).min(max_jump));
        jump_complete.min(jump_sound).min(max_jump)
    }
}

impl Formula {
    fn proposition_start_interval(&self, interval: Interval) -> HashSet<Interval> {
        fn inner_start(formula: &Formula, delta: Interval, set: &mut HashSet<Interval>) {
            match formula {
                Formula::Prop(_) => {
                    set.insert(delta);
                }
                Formula::Not(inner) => {
                    inner_start(inner, delta, set);
                }
                Formula::Or(operands) | Formula::And(operands) => {
                    for op in operands {
                        inner_start(op, delta.clone(), set);
                    }
                }
                Formula::Imply {
                    right, not_left, ..
                } => {
                    inner_start(not_left, delta.clone(), set);
                    inner_start(right, delta, set);
                }
                Formula::U {
                    left,
                    right,
                    interval,
                } => {
                    inner_start(
                        left,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.lower,
                        },
                        set,
                    );
                    inner_start(
                        right,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
                Formula::R {
                    left,
                    right,
                    interval,
                } => {
                    inner_start(
                        left,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                    inner_start(
                        right,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.lower,
                        },
                        set,
                    );
                }
                Formula::G { interval, phi } => {
                    inner_start(
                        phi,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.lower,
                        },
                        set,
                    );
                }
                Formula::F { interval, phi } => {
                    inner_start(
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
        inner_start(self, interval, &mut set);
        set
    }

    fn proposition_end_interval(&self, interval: Interval) -> HashSet<Interval> {
        fn inner_end(formula: &Formula, delta: Interval, set: &mut HashSet<Interval>) {
            match formula {
                Formula::Prop(_) => {
                    set.insert(delta);
                }
                Formula::Not(inner) => {
                    inner_end(inner, delta, set);
                }
                Formula::Or(operands) | Formula::And(operands) => {
                    for op in operands {
                        inner_end(op, delta.clone(), set);
                    }
                }
                Formula::Imply {
                    right, not_left, ..
                } => {
                    inner_end(not_left, delta.clone(), set);
                    inner_end(right, delta, set);
                }
                Formula::U {
                    left,
                    right,
                    interval,
                } => {
                    inner_end(
                        left,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                    inner_end(
                        right,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
                Formula::R {
                    left,
                    right,
                    interval,
                } => {
                    inner_end(
                        left,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                    inner_end(
                        right,
                        Interval {
                            lower: delta.lower + interval.lower,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
                Formula::G { interval, phi } => {
                    inner_end(
                        phi,
                        Interval {
                            lower: delta.lower + interval.upper,
                            upper: delta.upper + interval.upper,
                        },
                        set,
                    );
                }
                Formula::F { interval, phi } => {
                    inner_end(
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
        inner_end(self, interval, &mut set);
        set
    }
}

impl Clone for Node {
    fn clone(&self) -> Self {
        Self {
            operands: self.operands.clone(),
            current_time: self.current_time,
            implies: None,
            id: NODE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {}",
            join_with(&self.operands, ", "),
            self.current_time
        )
    }
}

impl Display for NodeFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.marked {
            write!(f, "O{}", self.kind)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}
