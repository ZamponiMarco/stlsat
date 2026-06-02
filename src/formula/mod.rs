use std::fmt::{self, Display};
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use num_rational::Ratio;

use crate::formula::transform::{
    DupeFormula, NegationNormalFormTransformer, RecursiveFormulaTransformer,
};
use crate::util::join_with;

pub mod parser;
pub mod statistics;
pub mod transform;

pub type VariableName = Arc<str>;

pub static FORMULA_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArithOp {
    Add,
    Sub,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AExpr {
    Var(VariableName),
    Num(Ratio<i64>),
    Abs(Box<AExpr>),
    BinOp {
        op: ArithOp,
        left: Box<AExpr>,
        right: Box<AExpr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExprKind {
    Atom(VariableName),
    Rel {
        op: RelOp,
        left: AExpr,
        right: AExpr,
    },
    True,
    False,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Expr {
    pub id: usize,
    pub kind: ExprKind,
}

impl Expr {
    pub fn from_expr(kind: ExprKind) -> Self {
        Expr {
            id: FORMULA_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            kind,
        }
    }

    #[must_use]
    pub fn bool(var: VariableName) -> Self {
        Expr::from_expr(ExprKind::Atom(var))
    }

    #[must_use]
    pub fn real(op: RelOp, left: AExpr, right: AExpr) -> Self {
        Expr::from_expr(ExprKind::Rel { op, left, right })
    }

    #[must_use]
    pub fn true_expr() -> Self {
        Expr::from_expr(ExprKind::True)
    }

    #[must_use]
    pub fn false_expr() -> Self {
        Expr::from_expr(ExprKind::False)
    }

    #[must_use]
    pub fn eq_kind(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Interval {
    pub lower: i32,
    pub upper: i32,
}

impl Interval {
    #[must_use]
    pub fn contains(&self, other: &Interval) -> bool {
        self.lower <= other.lower && self.upper >= other.upper
    }

    #[must_use]
    pub fn intersects(&self, other: &Interval) -> bool {
        self.upper >= other.lower && other.upper >= self.lower
    }

    #[must_use]
    pub fn active(&self, current_time: i32) -> bool {
        current_time >= self.lower && current_time <= self.upper
    }

    #[must_use]
    pub fn contiguous(&self, other: &Interval) -> bool {
        self.upper + 1 == other.lower || other.upper + 1 == self.lower
    }

    #[must_use]
    pub fn union(&self, other: &Interval) -> Interval {
        Interval {
            lower: self.lower.min(other.lower),
            upper: self.upper.max(other.upper),
        }
    }

    #[must_use]
    pub fn intersection(&self, other: &Interval) -> Interval {
        Interval {
            lower: self.lower.max(other.lower),
            upper: self.upper.min(other.upper),
        }
    }

    #[must_use]
    pub fn shift_left(&self, time: i32) -> Option<Interval> {
        if time > self.upper {
            return None;
        }

        Some(Interval {
            lower: (self.lower - time).max(0),
            upper: self.upper - time,
        })
    }

    #[must_use]
    pub fn shift_right(&self, time: i32) -> Interval {
        Interval {
            lower: self.lower + time,
            upper: self.upper + time,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Formula {
    // Propositions
    Prop(Expr),

    // Boolean/structural
    And(Vec<Formula>),
    Or(Vec<Formula>),
    Imply {
        left: Box<Formula>,
        right: Box<Formula>,
        not_left: Box<Formula>,
    },
    Not(Box<Formula>),

    // Temporal
    G {
        interval: Interval,
        phi: Box<Formula>,
    },
    F {
        interval: Interval,
        phi: Box<Formula>,
    },
    U {
        interval: Interval,
        left: Box<Formula>,
        right: Box<Formula>,
    },
    R {
        interval: Interval,
        left: Box<Formula>,
        right: Box<Formula>,
    },
}

impl Formula {
    #[must_use]
    pub fn prop(expr: Expr) -> Self {
        Formula::Prop(expr)
    }

    #[must_use]
    pub fn and(operands: Vec<Formula>) -> Self {
        Formula::And(operands)
    }

    #[must_use]
    pub fn or(operands: Vec<Formula>) -> Self {
        Formula::Or(operands)
    }

    #[must_use]
    pub fn imply(left: Formula, right: Formula) -> Self {
        Formula::Imply {
            left: Box::new(left.clone()),
            right: Box::new(right),
            not_left: Box::new(
                NegationNormalFormTransformer.visit(&Formula::not(DupeFormula.visit(&left))),
            ),
        }
    }

    // Clippy wants us to implement the std::ops::Not trait, but I don't think that's appropriate here.
    // To avoid the warning, we could rename the method and corresponding enum variant to something like "negate",
    // but I think "not" is clearer and should not be changed just to satisfy the linting rule.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn not(inner: Formula) -> Self {
        Formula::Not(Box::new(inner))
    }

    #[must_use]
    pub fn g(interval: Interval, phi: Formula) -> Self {
        Formula::G {
            interval,
            phi: Box::new(phi),
        }
    }

    #[must_use]
    pub fn f(interval: Interval, phi: Formula) -> Self {
        Formula::F {
            interval,
            phi: Box::new(phi),
        }
    }

    #[must_use]
    pub fn u(interval: Interval, left: Formula, right: Formula) -> Self {
        Formula::U {
            interval,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[must_use]
    pub fn r(interval: Interval, left: Formula, right: Formula) -> Self {
        Formula::R {
            interval,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[must_use]
    pub fn with_operand(mut self, operand: Formula) -> Self {
        match &mut self {
            Formula::Not(inner) => **inner = operand,
            Formula::G { phi, .. } | Formula::F { phi, .. } => **phi = operand,
            _ => panic!("Cannot set operand on formula without a single inner operand"),
        }
        self
    }

    #[must_use]
    pub fn with_operand_couple(mut self, left: Formula, right: Formula) -> Self {
        match &mut self {
            Formula::U {
                left: l, right: r, ..
            }
            | Formula::R {
                left: l, right: r, ..
            } => {
                **l = left;
                **r = right;
            }
            _ => panic!("Cannot set operands on formula without two inner operands"),
        }
        self
    }

    #[must_use]
    pub fn with_interval(mut self, interval: Interval) -> Self {
        match &mut self {
            Formula::G { interval: int, .. }
            | Formula::F { interval: int, .. }
            | Formula::U { interval: int, .. }
            | Formula::R { interval: int, .. } => *int = interval,
            _ => panic!("Cannot set interval on non-temporal formula"),
        }
        self
    }

    #[must_use]
    pub fn with_operands(mut self, operands: Vec<Formula>) -> Self {
        match &mut self {
            Formula::And(ops) | Formula::Or(ops) => *ops = operands,
            _ => panic!("Cannot set operands on formulas different from And/Or"),
        }
        self
    }

    #[must_use]
    pub fn with_implication(mut self, left: Formula, right: Formula, not_left: Formula) -> Self {
        match &mut self {
            Formula::Imply {
                left: l,
                right: r,
                not_left: nl,
            } => {
                **l = left;
                **r = right;
                **nl = not_left;
            }
            _ => panic!("Cannot set implications on formulas different from Imply"),
        }
        self
    }

    #[must_use]
    pub fn get_interval(&self) -> Option<Interval> {
        match &self {
            Formula::G { interval, .. }
            | Formula::F { interval, .. }
            | Formula::U { interval, .. }
            | Formula::R { interval, .. } => Some(interval.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub fn lower_bound(&self) -> Option<i32> {
        self.get_interval().map(|i| i.lower)
    }

    #[must_use]
    pub fn upper_bound(&self) -> Option<i32> {
        self.get_interval().map(|i| i.upper)
    }

    #[must_use]
    pub fn has_temporal(&self) -> bool {
        match &self {
            Formula::G { .. } | Formula::F { .. } | Formula::U { .. } | Formula::R { .. } => true,
            Formula::And(v) | Formula::Or(v) => v.iter().any(Formula::has_temporal),
            Formula::Not(inner) => inner.has_temporal(),
            Formula::Imply { left, right, .. } => left.has_temporal() || right.has_temporal(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_complex_temporal_operator(&self) -> bool {
        match &self {
            Formula::G { phi, .. }
            | Formula::U { left: phi, .. }
            | Formula::R { right: phi, .. } => phi.has_temporal(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_negation_normal_form(&self) -> bool {
        match &self {
            Formula::Not(inner) => matches!(**inner, Formula::Prop(_)),
            Formula::And(ops) | Formula::Or(ops) => {
                ops.iter().all(Formula::is_negation_normal_form)
            }
            Formula::Imply {
                left,
                right,
                not_left,
            } => {
                left.is_negation_normal_form()
                    && right.is_negation_normal_form()
                    && not_left.is_negation_normal_form()
            }
            Formula::G { phi, .. } | Formula::F { phi, .. } => phi.is_negation_normal_form(),
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.is_negation_normal_form() && right.is_negation_normal_form()
            }
            _ => true,
        }
    }

    #[must_use]
    pub fn is_flat(&self) -> bool {
        match &self {
            Formula::And(ops) => !ops.iter().any(|f| matches!(f, Formula::And(_))),
            Formula::Or(ops) => !ops.iter().any(|f| matches!(f, Formula::Or(_))),
            Formula::Imply {
                left,
                right,
                not_left,
            } => left.is_flat() && right.is_flat() && not_left.is_flat(),
            Formula::G { phi, .. } | Formula::F { phi, .. } => phi.is_flat(),
            Formula::U { left, right, .. } | Formula::R { left, right, .. } => {
                left.is_flat() && right.is_flat()
            }
            _ => true,
        }
    }

    #[must_use]
    pub fn eq_structural(&self, other: &Self) -> bool {
        match (self, other) {
            (Formula::Prop(a), Formula::Prop(b)) => a.eq_kind(b),
            (Formula::And(a), Formula::And(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq_structural(y))
            }
            (Formula::Or(a), Formula::Or(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.eq_structural(y))
            }
            (Formula::Not(a), Formula::Not(b)) => a.eq_structural(b),
            (
                Formula::Imply {
                    left: al,
                    right: ar,
                    not_left: anl,
                },
                Formula::Imply {
                    left: bl,
                    right: br,
                    not_left: bnl,
                },
            ) => al.eq_structural(bl) && ar.eq_structural(br) && anl.eq_structural(bnl),
            (
                Formula::G {
                    interval: ai,
                    phi: ap,
                    ..
                },
                Formula::G {
                    interval: bi,
                    phi: bp,
                    ..
                },
            ) => ai == bi && ap.eq_structural(bp),
            (
                Formula::F {
                    interval: ai,
                    phi: ap,
                    ..
                },
                Formula::F {
                    interval: bi,
                    phi: bp,
                    ..
                },
            ) => ai == bi && ap.eq_structural(bp),
            (
                Formula::U {
                    interval: ai,
                    left: al,
                    right: ar,
                    ..
                },
                Formula::U {
                    interval: bi,
                    left: bl,
                    right: br,
                    ..
                },
            ) => ai == bi && al.eq_structural(bl) && ar.eq_structural(br),
            (
                Formula::R {
                    interval: ai,
                    left: al,
                    right: ar,
                    ..
                },
                Formula::R {
                    interval: bi,
                    left: bl,
                    right: br,
                    ..
                },
            ) => ai == bi && al.eq_structural(bl) && ar.eq_structural(br),
            _ => false,
        }
    }
}

impl Display for AExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AExpr::Var(s) => write!(f, "{s}"),
            AExpr::Num(n) => write!(f, "{n}"),
            AExpr::Abs(inner) => write!(f, "|{inner}|"),
            AExpr::BinOp { op, left, right } => {
                let sym = match op {
                    ArithOp::Add => "+",
                    ArithOp::Sub => "-",
                };
                write!(f, "({left} {sym} {right})")
            }
        }
    }
}

impl Display for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprKind::Atom(s) => write!(f, "{s}"),
            ExprKind::Rel { op, left, right } => {
                let sym = match op {
                    RelOp::Lt => "<",
                    RelOp::Le => "<=",
                    RelOp::Gt => ">",
                    RelOp::Ge => ">=",
                    RelOp::Eq => "==",
                    RelOp::Ne => "!=",
                };
                write!(f, "{left} {sym} {right}")
            }
            ExprKind::True => write!(f, "true"),
            ExprKind::False => write!(f, "false"),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.lower, self.upper)
    }
}

impl Display for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Formula::And(v) => write!(f, "{}", join_with(v, " && ")),
            Formula::Or(v) => write!(f, "{}", join_with(v, " || ")),
            Formula::Not(inner) => write!(f, "(!{inner})"),
            Formula::Imply { left, right, .. } => write!(f, "({left} -> {right})"),
            Formula::G { interval, phi, .. } => write!(f, "G{interval} {phi}"),
            Formula::F { interval, phi, .. } => write!(f, "F{interval} {phi}"),
            Formula::U {
                interval,
                left,
                right,
                ..
            } => write!(f, "({left} U{interval} {right})"),
            Formula::R {
                interval,
                left,
                right,
                ..
            } => write!(f, "({left} R{interval} {right})"),
            Formula::Prop(expr) => write!(f, "{expr}"),
        }
    }
}
