use crate::{
    formula::{
        Formula,
        transform::{
            FlatTransformer, FormulaSimplifier, NegationNormalFormTransformer,
            RecursiveFormulaTransformer, STLTransformer, ShiftBoundsTransformer,
        },
    },
    util::join_with,
};
use std::{
    fmt::{self, Display},
    sync::atomic::{AtomicUsize, Ordering},
};

pub mod decompose;
pub mod intervals;
pub mod jump;
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
