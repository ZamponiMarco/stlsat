use std::vec;

use crate::formula::Formula;
use crate::sat::tableau::Tableau;
use crate::sat::tableau::node::{Node, NodeFormula};

#[cfg(test)]
mod tests;

impl Tableau {
    #[must_use]
    pub fn decompose(&self, node: &Node) -> Option<Vec<Node>> {
        if self.tableau_options.formula_optimizations
            && let Some(res) = node.rewrite_chain()
        {
            return Some(res);
        }

        if let Some(res) = self.decompose_and(node) {
            return Some(res);
        }

        if let Some(res) = self.decompose_g(node) {
            return Some(res);
        }

        for (i, operand) in node.operands.iter().enumerate() {
            match &operand.kind {
                Formula::Or(_) => {
                    return Some(self.decompose_or_at(node, i));
                }
                Formula::Imply { .. } => {
                    return Some(self.decompose_imply_at(node, i));
                }
                _ => {}
            }
        }

        for (i, operand) in node.operands.iter().enumerate() {
            match &operand.kind {
                Formula::F { .. } if !operand.marked && operand.is_active_at(node.current_time) => {
                    return Some(self.decompose_f_at(node, i));
                }
                Formula::U { .. } if !operand.marked && operand.is_active_at(node.current_time) => {
                    return Some(self.decompose_u_at(node, i));
                }
                Formula::R { .. } if !operand.marked && operand.is_active_at(node.current_time) => {
                    return Some(self.decompose_r_at(node, i));
                }
                _ => {}
            }
        }

        self.decompose_jump(node)
    }

    #[must_use]
    pub fn decompose_and(&self, node: &Node) -> Option<Vec<Node>> {
        let mut changed = false;
        let flattened_operands: Vec<NodeFormula> = node
            .operands
            .iter()
            .flat_map(|nf| match &nf.kind {
                Formula::And(inner) => {
                    changed = true;
                    inner
                        .iter()
                        .map(|f| NodeFormula::from(f.clone()).with_parent_id(nf.parent_id))
                        .collect()
                }
                _ => vec![nf.clone()],
            })
            .collect();

        changed.then(|| {
            vec![Node {
                operands: flattened_operands,
                ..node.clone()
            }]
        })
    }

    #[must_use]
    pub fn decompose_g(&self, node: &Node) -> Option<Vec<Node>> {
        let mut changed = false;
        let flattened_operands: Vec<NodeFormula> = node
            .operands
            .iter()
            .flat_map(|f| match &f.kind {
                Formula::G { interval, phi, .. }
                    if f.is_active_at(node.current_time) && !f.marked =>
                {
                    changed = true;
                    let tex = phi.temporal_expansion(node.current_time, Some(f.id));
                    if node.current_time < interval.upper {
                        vec![f.clone().with_marked(true), tex]
                    } else {
                        vec![tex]
                    }
                }
                _ => vec![f.clone()],
            })
            .collect();

        changed.then(|| {
            vec![Node {
                operands: flattened_operands,
                ..node.clone()
            }]
        })
    }

    #[must_use]
    pub fn decompose_or_at(&self, node: &Node, i: usize) -> Vec<Node> {
        let Formula::Or(or_operands) = &node.operands[i].kind else {
            panic!("decompose_or_at called on non-Or formula at index {i}");
        };

        or_operands
            .iter()
            .map(|or_operand| {
                let mut new_operands = node.operands.clone();
                new_operands[i] =
                    NodeFormula::from(or_operand.clone()).with_parent_id(new_operands[i].parent_id);
                Node {
                    operands: new_operands,
                    ..node.clone()
                }
            })
            .collect()
    }

    #[must_use]
    pub fn decompose_imply_at(&self, node: &Node, i: usize) -> Vec<Node> {
        let Formula::Imply {
            left,
            right,
            not_left,
        } = &node.operands[i].kind
        else {
            panic!("decompose_imply_at called on non-Imply formula at index {i}");
        };

        let mut new_node1 = node.clone();
        new_node1.operands[i] =
            NodeFormula::from((**not_left).clone()).with_parent_id(node.operands[i].parent_id);

        let mut new_node2 = node.clone();
        new_node2.operands[i] =
            NodeFormula::from((**right).clone()).with_parent_id(node.operands[i].parent_id);
        if self.tableau_options.formula_optimizations {
            new_node2.operands.insert(
                i,
                NodeFormula::from((**left).clone()).with_parent_id(node.operands[i].parent_id),
            );
        }

        vec![new_node1, new_node2]
    }

    #[must_use]
    pub fn decompose_f_at(&self, node: &Node, i: usize) -> Vec<Node> {
        let f_formula = &node.operands[i];

        let Formula::F { phi, interval, .. } = &f_formula.kind else {
            panic!("decompose_f_at called on non-F formula at index {i}");
        };

        debug_assert!(
            node.operands[i].is_active_at(node.current_time),
            "decompose_f_at called on F formula that is not active at current time {}",
            node.current_time
        );

        debug_assert!(
            !node.operands[i].marked,
            "decompose_f_at called on F formula that is already marked at current time {}",
            node.current_time
        );

        // Node where F is satisfied (p)
        let mut new_node1 = node.clone();
        new_node1.operands[i] = phi.temporal_expansion(node.current_time, None);

        // Node in which F is not satisfied (OF)
        if node.current_time < interval.upper {
            let mut new_node2 = node.clone();
            new_node2.operands[i] = f_formula.clone().with_marked(true);

            vec![new_node1, new_node2]
        } else {
            vec![new_node1]
        }
    }

    #[must_use]
    pub fn decompose_u_at(&self, node: &Node, i: usize) -> Vec<Node> {
        let u_formula = &node.operands[i];

        let Formula::U {
            left,
            right,
            interval,
            ..
        } = &u_formula.kind
        else {
            panic!("decompose_u_at called on non-U formula at index {i}");
        };

        debug_assert!(
            node.operands[i].is_active_at(node.current_time),
            "decompose_u_at called on U formula that is not active at current time {}",
            node.current_time
        );

        debug_assert!(
            !node.operands[i].marked,
            "decompose_u_at called on U formula that is already marked at current time {}",
            node.current_time
        );

        // Node where U is satisfied (q)
        let mut new_node1 = node.clone();
        new_node1.operands[i] = right.temporal_expansion(node.current_time, None);

        if node.current_time < interval.upper {
            // Node in which U is not satisfied (OU, p)
            let mut new_node2 = node.clone();
            new_node2.operands[i] = left.temporal_expansion(node.current_time, Some(u_formula.id));
            new_node2
                .operands
                .insert(i, u_formula.clone().with_marked(true));
            vec![new_node1, new_node2]
        } else {
            vec![new_node1]
        }
    }

    #[must_use]
    pub fn decompose_r_at(&self, node: &Node, i: usize) -> Vec<Node> {
        let r_formula = &node.operands[i];

        let Formula::R {
            interval,
            left,
            right,
            ..
        } = &r_formula.kind
        else {
            panic!("decompose_r_at called on non-R formula at index {i}");
        };

        debug_assert!(
            node.operands[i].is_active_at(node.current_time),
            "decompose_r_at called on R formula that is not active at current time {}",
            node.current_time
        );

        debug_assert!(
            !node.operands[i].marked,
            "decompose_r_at called on R formula that is already marked at current time {}",
            node.current_time
        );

        // Node in which R is not satisfied now (OR, q)
        let mut new_node2 = node.clone();
        new_node2.operands[i] = right.temporal_expansion(node.current_time, Some(r_formula.id));
        if node.current_time < interval.upper {
            new_node2
                .operands
                .insert(i, r_formula.clone().with_marked(true));
        }

        if node.current_time < interval.upper {
            // Node where R is satisfied now (p, q)
            let mut new_node1 = node.clone();
            new_node1.operands[i] = right.temporal_expansion(node.current_time, None);
            new_node1
                .operands
                .insert(i, left.temporal_expansion(node.current_time, None));
            vec![new_node1, new_node2]
        } else {
            vec![new_node2]
        }
    }

    #[must_use]
    pub fn decompose_jump(&self, node: &Node) -> Option<Vec<Node>> {
        // Select jump length
        let jump = if !self.tableau_options.jump_rule_enabled {
            1
        } else {
            node.calculate_k_star()
        };

        // Retain only temporal operators
        let new_operands: Vec<NodeFormula> = node
            .operands
            .iter()
            .filter_map(|op| {
                if let Some(interval) = op.kind.get_interval()
                    && node.current_time < interval.upper
                {
                    Some(op.clone().with_marked(false))
                } else {
                    None
                }
            })
            .collect();

        // Construct return value
        if new_operands.is_empty() {
            return None;
        }

        let mut new_node = node.clone();
        new_node.operands = new_operands;
        new_node.current_time += jump;

        if self.tableau_options.simple_first {
            let simple_operands: Vec<NodeFormula> = new_node
                .operands
                .iter()
                .filter(|f| !f.kind.is_complex_temporal_operator())
                .cloned()
                .collect();
            if !simple_operands.is_empty() && simple_operands.len() < new_node.operands.len() {
                let mut simple_node = new_node.clone();
                simple_node.operands = simple_operands;
                simple_node.implies = Some(vec![new_node.id]);
                return Some(vec![simple_node, new_node]);
            }
        }
        Some(vec![new_node])
    }
}

impl Formula {
    fn temporal_expansion(&self, current_time: i32, parent_id: Option<usize>) -> NodeFormula {
        fn inner(formula: &Formula, current_time: i32) -> Formula {
            match formula {
                Formula::Prop(_) | Formula::Not(_) => formula.clone(),
                Formula::F { interval, .. }
                | Formula::G { interval, .. }
                | Formula::U { interval, .. }
                | Formula::R { interval, .. } => formula
                    .clone()
                    .with_interval(interval.shift_right(current_time)),
                Formula::And(operands) | Formula::Or(operands) => formula
                    .clone()
                    .with_operands(operands.iter().map(|op| inner(op, current_time)).collect()),
                Formula::Imply {
                    left,
                    right,
                    not_left,
                } => formula.clone().with_implication(
                    inner(left, current_time),
                    inner(right, current_time),
                    inner(not_left, current_time),
                ),
            }
        }
        NodeFormula::from(inner(self, current_time)).with_parent_id(parent_id)
    }
}
