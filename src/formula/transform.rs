use std::collections::{BTreeMap, BTreeSet};

use crate::formula::{Expr, ExprKind, Formula, Interval};

#[cfg(test)]
mod tests;

pub trait RecursiveFormulaTransformer {
    fn visit(&self, formula: &Formula) -> Formula {
        match &formula {
            Formula::And(ops) => self.visit_and(formula, ops),
            Formula::Or(ops) => self.visit_or(formula, ops),
            Formula::Not(inner) => self.visit_not(formula, inner),
            Formula::G { interval, phi } => self.visit_globally(formula, interval, phi),
            Formula::F { interval, phi } => self.visit_finally(formula, interval, phi),
            Formula::U {
                interval,
                left,
                right,
            } => self.visit_until(formula, interval, left, right),
            Formula::R {
                interval,
                left,
                right,
            } => self.visit_release(formula, interval, left, right),
            Formula::Imply {
                left,
                right,
                not_left,
            } => self.visit_imply(formula, left, right, not_left),
            Formula::Prop(expr) => self.visit_leaf(formula, expr),
        }
    }

    fn visit_and(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        formula
            .clone()
            .with_operands(ops.iter().map(|op| self.visit(op)).collect())
    }

    fn visit_or(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        formula
            .clone()
            .with_operands(ops.iter().map(|op| self.visit(op)).collect())
    }

    fn visit_not(&self, formula: &Formula, inner: &Formula) -> Formula {
        formula.clone().with_operand(self.visit(inner))
    }

    fn visit_next(&self, formula: &Formula, inner: &Formula) -> Formula {
        formula.clone().with_operand(self.visit(inner))
    }

    fn visit_globally(&self, formula: &Formula, _interval: &Interval, phi: &Formula) -> Formula {
        formula.clone().with_operand(self.visit(phi))
    }

    fn visit_finally(&self, formula: &Formula, _interval: &Interval, phi: &Formula) -> Formula {
        formula.clone().with_operand(self.visit(phi))
    }

    fn visit_until(
        &self,
        formula: &Formula,
        _interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        formula
            .clone()
            .with_operand_couple(self.visit(left), self.visit(right))
    }

    fn visit_release(
        &self,
        formula: &Formula,
        _interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        formula
            .clone()
            .with_operand_couple(self.visit(left), self.visit(right))
    }

    fn visit_imply(
        &self,
        formula: &Formula,
        left: &Formula,
        right: &Formula,
        not_left: &Formula,
    ) -> Formula {
        formula
            .clone()
            .with_implication(self.visit(left), self.visit(right), self.visit(not_left))
    }

    fn visit_leaf(&self, formula: &Formula, _expr: &Expr) -> Formula {
        formula.clone()
    }
}

pub struct NegationNormalFormTransformer;
impl RecursiveFormulaTransformer for NegationNormalFormTransformer {
    fn visit_not(&self, formula: &Formula, inner: &Formula) -> Formula {
        match &inner {
            Formula::Not(i) => self.visit(i),
            Formula::And(ops) => Formula::or(
                ops.iter()
                    .map(|f| self.visit(&Formula::not(f.clone())))
                    .collect(),
            ),
            Formula::Or(ops) => Formula::and(
                ops.iter()
                    .map(|f| self.visit(&Formula::not(f.clone())))
                    .collect(),
            ),
            Formula::Imply { left, right, .. } => Formula::and(vec![
                *left.clone(),
                self.visit(&Formula::not(*right.clone())),
            ]),
            Formula::G { phi, interval } => {
                Formula::f(interval.clone(), self.visit(&Formula::not(*phi.clone())))
            }
            Formula::F { phi, interval } => {
                Formula::g(interval.clone(), self.visit(&Formula::not(*phi.clone())))
            }
            Formula::U {
                interval,
                left,
                right,
            } => Formula::r(
                interval.clone(),
                self.visit(&Formula::not(*left.clone())),
                self.visit(&Formula::not(*right.clone())),
            ),
            Formula::R {
                interval,
                left,
                right,
            } => Formula::u(
                interval.clone(),
                self.visit(&Formula::not(*left.clone())),
                self.visit(&Formula::not(*right.clone())),
            ),
            Formula::Prop(_) => formula.clone(),
        }
    }
}

pub struct STLTransformer;
impl RecursiveFormulaTransformer for STLTransformer {
    fn visit_until(
        &self,
        formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        let g_part = Formula::g(
            Interval {
                lower: 0,
                upper: interval.lower,
            },
            self.visit(left),
        );
        Formula::and(vec![
            g_part,
            formula.clone().with_operand_couple(
                self.visit(left),
                Formula::and(vec![self.visit(left), self.visit(right)]),
            ),
        ])
    }

    fn visit_release(
        &self,
        _formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        let new_left = self.visit(left);
        let new_right = self.visit(right);

        let f_part = Formula::f(
            Interval {
                lower: 0,
                upper: interval.lower,
            },
            new_left.clone(),
        );
        let g_part = Formula::g(interval.clone(), new_right.clone());
        let u_part = Formula::u(interval.clone(), new_right, new_left);
        Formula::or(vec![f_part, u_part, g_part])
    }
}

pub struct FlatTransformer;
impl RecursiveFormulaTransformer for FlatTransformer {
    fn visit_and(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        formula.clone().with_operands(
            ops.iter()
                .map(|op| self.visit(op))
                .flat_map(|flat_op| {
                    if let Formula::And(inner_ops) = &flat_op {
                        inner_ops.clone()
                    } else {
                        vec![flat_op]
                    }
                })
                .collect(),
        )
    }

    fn visit_or(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        formula.clone().with_operands(
            ops.iter()
                .map(|op| self.visit(op))
                .flat_map(|flat_op| {
                    if let Formula::Or(inner_ops) = &flat_op {
                        inner_ops.clone()
                    } else {
                        vec![flat_op]
                    }
                })
                .collect(),
        )
    }
}

pub struct ShiftBoundsTransformer;
impl RecursiveFormulaTransformer for ShiftBoundsTransformer {
    fn visit_globally(&self, formula: &Formula, interval: &Interval, phi: &Formula) -> Formula {
        let new_phi = self.visit(phi);
        if let Some(shift) = new_phi.get_shift() {
            formula
                .clone()
                .with_interval(interval.shift_right(shift))
                .with_operand(ShiftBackwardTransformer(shift).visit(&new_phi))
        } else {
            formula.clone().with_operand(new_phi)
        }
    }

    fn visit_finally(&self, formula: &Formula, interval: &Interval, phi: &Formula) -> Formula {
        let new_phi = self.visit(phi);
        if let Some(shift) = new_phi.get_shift() {
            formula
                .clone()
                .with_interval(interval.shift_right(shift))
                .with_operand(ShiftBackwardTransformer(shift).visit(&new_phi))
        } else {
            formula.clone().with_operand(new_phi)
        }
    }

    fn visit_until(
        &self,
        formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        let new_left = self.visit(left);
        let new_right = self.visit(right);
        if let Some(shift) = new_left.get_shift().min(new_right.get_shift()) {
            formula
                .clone()
                .with_interval(interval.shift_right(shift))
                .with_operand_couple(
                    ShiftBackwardTransformer(shift).visit(&new_left),
                    ShiftBackwardTransformer(shift).visit(&new_right),
                )
        } else {
            formula.clone().with_operand_couple(new_left, new_right)
        }
    }

    fn visit_release(
        &self,
        formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        let new_left = self.visit(left);
        let new_right = self.visit(right);
        if let Some(shift) = new_left.get_shift().min(new_right.get_shift()) {
            formula
                .clone()
                .with_interval(interval.shift_right(shift))
                .with_operand_couple(
                    ShiftBackwardTransformer(shift).visit(&new_left),
                    ShiftBackwardTransformer(shift).visit(&new_right),
                )
        } else {
            formula.clone().with_operand_couple(new_left, new_right)
        }
    }
}

pub struct ShiftBackwardTransformer(i32);
impl RecursiveFormulaTransformer for ShiftBackwardTransformer {
    fn visit_globally(&self, formula: &Formula, interval: &Interval, _phi: &Formula) -> Formula {
        formula
            .clone()
            .with_interval(interval.shift_left(self.0).unwrap())
    }

    fn visit_finally(&self, formula: &Formula, interval: &Interval, _phi: &Formula) -> Formula {
        formula
            .clone()
            .with_interval(interval.shift_left(self.0).unwrap())
    }

    fn visit_until(
        &self,
        formula: &Formula,
        interval: &Interval,
        _left: &Formula,
        _right: &Formula,
    ) -> Formula {
        formula
            .clone()
            .with_interval(interval.shift_left(self.0).unwrap())
    }

    fn visit_release(
        &self,
        formula: &Formula,
        interval: &Interval,
        _left: &Formula,
        _right: &Formula,
    ) -> Formula {
        formula
            .clone()
            .with_interval(interval.shift_left(self.0).unwrap())
    }
}

pub struct ShiftForwardTransformer(i32);
impl RecursiveFormulaTransformer for ShiftForwardTransformer {
    fn visit_globally(&self, formula: &Formula, interval: &Interval, _phi: &Formula) -> Formula {
        formula.clone().with_interval(interval.shift_right(self.0))
    }

    fn visit_finally(&self, formula: &Formula, interval: &Interval, _phi: &Formula) -> Formula {
        formula.clone().with_interval(interval.shift_right(self.0))
    }

    fn visit_until(
        &self,
        formula: &Formula,
        interval: &Interval,
        _left: &Formula,
        _right: &Formula,
    ) -> Formula {
        formula.clone().with_interval(interval.shift_right(self.0))
    }

    fn visit_release(
        &self,
        formula: &Formula,
        interval: &Interval,
        _left: &Formula,
        _right: &Formula,
    ) -> Formula {
        formula.clone().with_interval(interval.shift_right(self.0))
    }

    fn visit_not(&self, formula: &Formula, _inner: &Formula) -> Formula {
        Formula::f(
            Interval {
                lower: self.0,
                upper: self.0,
            },
            formula.clone(),
        )
    }

    fn visit_leaf(&self, formula: &Formula, _expr: &Expr) -> Formula {
        Formula::f(
            Interval {
                lower: self.0,
                upper: self.0,
            },
            formula.clone(),
        )
    }
}

pub struct DupeFormula;
impl RecursiveFormulaTransformer for DupeFormula {
    fn visit_leaf(&self, _formula: &Formula, expr: &Expr) -> Formula {
        Formula::prop(Expr::from_expr(expr.kind.clone()))
    }
}

pub struct FormulaSimplifier;
impl RecursiveFormulaTransformer for FormulaSimplifier {
    fn visit_and(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        fn merge_globally_in_and(input: Vec<Formula>) -> Vec<Formula> {
            let mut to_remove = BTreeSet::new();
            let mut map: BTreeMap<usize, Interval> = BTreeMap::new();

            for (idx, op) in input.iter().enumerate() {
                if let Formula::G { interval, phi, .. } = op {
                    let mut found = None;
                    for (rep_idx, rep_formula) in input.iter().enumerate() {
                        if rep_idx >= idx {
                            break;
                        }
                        if let Formula::G { phi: rep_phi, .. } = rep_formula
                            && rep_phi.eq_structural(phi)
                        {
                            found = Some(rep_idx);
                            break;
                        }
                    }

                    match found {
                        Some(rep_idx) => {
                            if let Some(current) = map.get_mut(&rep_idx) {
                                if current.intersects(interval) || current.contiguous(interval) {
                                    *current = current.union(interval);
                                    to_remove.insert(idx);
                                }
                            } else if let Formula::G {
                                interval: rep_int, ..
                            } = &input[rep_idx]
                                && (rep_int.intersects(interval) || rep_int.contiguous(interval))
                            {
                                map.insert(rep_idx, rep_int.union(interval));
                                to_remove.insert(idx);
                            }
                        }
                        None => {
                            map.insert(idx, interval.clone());
                        }
                    }
                }
            }

            let mut new_operands = input.clone();
            for (idx, merged_interval) in &map {
                new_operands[*idx] = new_operands[*idx]
                    .clone()
                    .with_interval(merged_interval.clone());
            }

            new_operands
                .into_iter()
                .enumerate()
                .filter(|(i, _)| !to_remove.contains(i))
                .map(|(_, f)| f)
                .collect()
        }

        // 1. recursively simplify children
        let mut flat: Vec<Formula> = Vec::new();
        for op in ops {
            let v = self.visit(op);
            // flatten nested ANDs
            match v {
                Formula::And(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }

        // 2. remove duplicates (A && A = A)
        let mut unique: Vec<Formula> = Vec::new();
        for op in flat {
            if !unique.iter().any(|u| u.eq_structural(&op)) {
                unique.push(op);
            }
        }

        // 3. check annihilators and identities

        // false in any operand ⇒ false
        for u in &unique {
            if let Formula::Prop(e) = u
                && let ExprKind::False = e.kind
            {
                return Formula::prop(Expr::false_expr());
            }
        }

        // remove all "true" operands (A && true = A)
        unique.retain(|u| {
            if let Formula::Prop(e) = u
                && let ExprKind::True = e.kind
            {
                return false;
            }
            true
        });

        // 4. contradiction: A && !A = false
        for u in &unique {
            if let Formula::Not(inner) = u
                && unique.iter().any(|x| x.eq_structural(inner))
            {
                return Formula::prop(Expr::false_expr());
            }
        }

        // 5. simplify interaction with disjunctive structures
        let mut reduced: Vec<Formula> = Vec::new();
        for u in &unique {
            match u {
                // (A && (A || B)) → A
                Formula::Or(disjuncts)
                    if disjuncts
                        .iter()
                        .any(|d| unique.iter().any(|a| a.eq_structural(d))) =>
                {
                    continue; // redundant OR
                }
                // (A && F[0,u](A)) → A
                Formula::F { interval, phi, .. }
                    if interval.lower == 0 && unique.iter().any(|a| a.eq_structural(phi)) =>
                {
                    continue; // redundant F[0,u](A)
                }
                _ => {}
            }
            reduced.push(u.clone());
        }

        // 6. merge temporal operators
        reduced = merge_globally_in_and(reduced);

        // 7. collapse trivial results
        if reduced.is_empty() {
            return Formula::prop(Expr::true_expr()); // empty conjunction = true
        }
        if reduced.len() == 1 {
            return reduced[0].clone();
        }

        // 8. rebuild normalized formula
        formula.clone().with_operands(reduced)
    }

    fn visit_or(&self, formula: &Formula, ops: &[Formula]) -> Formula {
        fn merge_globally_in_or(input: Vec<Formula>) -> Vec<Formula> {
            let mut map: BTreeMap<usize, Interval> = BTreeMap::new();
            let mut to_remove = BTreeSet::new();

            for (idx, op) in input.iter().enumerate() {
                if let Formula::G { phi, interval, .. } = op {
                    for (j, prev) in input.iter().enumerate().take(idx) {
                        if let Formula::G {
                            phi: phi_j,
                            interval: int_j,
                            ..
                        } = prev
                            && phi.eq_structural(phi_j)
                        {
                            if interval.contains(int_j) {
                                to_remove.insert(idx);
                            } else if int_j.contains(interval) {
                                to_remove.insert(j);
                                map.insert(idx, interval.clone());
                            }
                            break;
                        }
                    }
                    map.entry(idx).or_insert(interval.clone());
                }
            }

            input
                .into_iter()
                .enumerate()
                .filter(|(i, _)| !to_remove.contains(i))
                .map(|(_, f)| f)
                .collect()
        }

        // 1. recursively simplify children
        let mut flat: Vec<Formula> = Vec::new();
        for op in ops {
            let v = self.visit(op);
            // flatten nested ORs
            match v {
                Formula::Or(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }

        // 2. remove duplicates (A || A = A)
        let mut unique: Vec<Formula> = Vec::new();
        for op in flat {
            if !unique.iter().any(|u| u.eq_structural(&op)) {
                unique.push(op);
            }
        }

        // 3. annihilators and identities
        // true in any operand ⇒ true
        for u in &unique {
            if let Formula::Prop(e) = u
                && let ExprKind::True = e.kind
            {
                return Formula::prop(Expr::true_expr());
            }
        }

        // remove all "false" operands (A || false = A)
        unique.retain(|u| {
            if let Formula::Prop(e) = u
                && let ExprKind::False = e.kind
            {
                return false;
            }
            true
        });

        // 4. tautology: A || !A = true
        for u in &unique {
            if let Formula::Not(inner) = u
                && unique.iter().any(|x| x.eq_structural(inner))
            {
                return Formula::prop(Expr::true_expr());
            }
        }

        // 5. absorption with conjunctive or temporal structures
        let mut reduced: Vec<Formula> = Vec::new();
        for u in &unique {
            match u {
                // (A || (A && B)) → A
                Formula::And(conjuncts)
                    if conjuncts
                        .iter()
                        .any(|c| unique.iter().any(|a| a.eq_structural(c))) =>
                {
                    continue; // redundant AND
                }
                // (A || G[0,u](A)) → A
                Formula::G { interval, phi, .. }
                    if interval.lower == 0 && unique.iter().any(|a| a.eq_structural(phi)) =>
                {
                    continue; // redundant G[0,u](A)
                }
                _ => {}
            }
            reduced.push(u.clone());
        }

        // 6. merge temporal operators
        reduced = merge_globally_in_or(reduced);

        // 7. collapse trivial results
        if reduced.is_empty() {
            return Formula::prop(Expr::false_expr());
        }
        if reduced.len() == 1 {
            return reduced[0].clone();
        }

        // 8. rebuild normalized formula
        formula.clone().with_operands(reduced)
    }

    fn visit_globally(&self, formula: &Formula, interval: &Interval, phi: &Formula) -> Formula {
        // 1. simplify inner formula recursively
        let new_phi = self.visit(phi);

        // 2. constant folding
        if let Formula::Prop(e) = &new_phi {
            match e.kind {
                ExprKind::True => return Formula::prop(Expr::true_expr()),
                ExprKind::False => return Formula::prop(Expr::false_expr()),
                _ => {}
            }
        }

        // 3. collapse nested G:  G[a,b](G[c,d](φ)) → G[a+c, b+d](φ)
        if let Formula::G {
            interval: inner_i,
            phi: inner_phi,
            ..
        } = &new_phi
        {
            let summed = Interval {
                lower: interval.lower + inner_i.lower,
                upper: interval.upper + inner_i.upper,
            };
            return self.visit(&Formula::g(summed, *inner_phi.clone()));
        }

        // 4. degenerate intervals
        if interval.lower == 0 && interval.upper == 0 {
            return new_phi; // G[0,0](φ) ≡ φ
        }

        if interval.lower == interval.upper {
            return ShiftForwardTransformer(interval.lower).visit(&new_phi);
        }

        // 5. distributivity
        if let Formula::And(ops) = &new_phi {
            let mut new_operands: Vec<Formula> = Vec::new();
            for op in ops {
                new_operands.push(Formula::g(interval.clone(), op.clone()));
            }
            return Formula::and(new_operands);
        }

        // 6. rebuild
        formula.clone().with_operand(new_phi)
    }

    fn visit_finally(&self, formula: &Formula, interval: &Interval, phi: &Formula) -> Formula {
        // 1. simplify inner formula recursively
        let new_phi = self.visit(phi);

        // 2. constant folding
        if let Formula::Prop(e) = &new_phi {
            match e.kind {
                ExprKind::True => return Formula::prop(Expr::true_expr()),
                ExprKind::False => return Formula::prop(Expr::false_expr()),
                _ => {}
            }
        }

        // 3. collapse nested F:  F[a,b](F[c,d](φ)) → F[a+c, b+d](φ)
        if let Formula::F {
            interval: inner_i,
            phi: inner_phi,
            ..
        } = &new_phi
        {
            let summed = Interval {
                lower: interval.lower + inner_i.lower,
                upper: interval.upper + inner_i.upper,
            };
            return self.visit(&Formula::f(summed, *inner_phi.clone()));
        }

        // 4. degenerate intervals
        if interval.lower == 0 && interval.upper == 0 {
            return new_phi; // F[0,0](φ) ≡ φ
        }

        if interval.lower == interval.upper {
            return ShiftForwardTransformer(interval.lower).visit(&new_phi);
        }

        // 5. rebuild
        formula.clone().with_operand(new_phi)
    }

    fn visit_until(
        &self,
        formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        // 1. recursively simplify both operands
        let new_left = self.visit(left);
        let new_right = self.visit(right);

        // 2. constant folding: left side simplifications
        if let Formula::Prop(e) = &new_left {
            match e.kind {
                // true U[a,b](f) = F[a,b](f)
                ExprKind::True => {
                    return self.visit(&Formula::f(interval.clone(), new_right));
                }

                // false U[a,b](f) = F[a,a](f)
                ExprKind::False => {
                    let reduced = Interval {
                        lower: interval.lower,
                        upper: interval.lower,
                    };
                    return self.visit(&Formula::f(reduced, new_right));
                }

                _ => {}
            }
        }

        // 3. constant folding: right side simplifications
        if let Formula::Prop(e) = &new_right {
            match e.kind {
                // f U[a,b](true) = true
                ExprKind::True => return Formula::prop(Expr::true_expr()),
                // f U[a,b](false) = false
                ExprKind::False => return Formula::prop(Expr::false_expr()),
                _ => {}
            }
        }

        // a U[a,b](!a) = F[a,b](!a)
        if let Formula::Not(inner) = &new_right
            && inner.eq_structural(&new_left)
        {
            return self.visit(&Formula::f(interval.clone(), new_right.clone()));
        }

        // !a U[a,b](a) = F[a,b](a)
        if let Formula::Not(inner) = &new_left
            && inner.eq_structural(&new_right)
        {
            return self.visit(&Formula::f(interval.clone(), new_right.clone()));
        }

        // 4. degenerate intervals
        if interval.lower == 0 && interval.upper == 0 {
            // U[0,0](φ, ψ) ≡ ψ
            return new_right;
        }

        if interval.lower == interval.upper {
            return ShiftForwardTransformer(interval.lower).visit(&new_right);
        }

        // 5. rebuild
        formula.clone().with_operand_couple(new_left, new_right)
    }

    fn visit_release(
        &self,
        formula: &Formula,
        interval: &Interval,
        left: &Formula,
        right: &Formula,
    ) -> Formula {
        let new_left = self.visit(left);
        let new_right = self.visit(right);

        // constant folding on right operand
        if let Formula::Prop(e) = &new_right {
            match e.kind {
                // f R[a,b](true) = true
                ExprKind::True => return Formula::prop(Expr::true_expr()),
                // f R[a,b](false) = false
                ExprKind::False => return Formula::prop(Expr::false_expr()),
                _ => {}
            }
        }

        // constant folding on left operand
        if let Formula::Prop(e) = &new_left {
            match e.kind {
                // true R[a,b](f) = F[a,a] f
                ExprKind::True => {
                    return self.visit(&Formula::f(
                        Interval {
                            lower: interval.lower,
                            upper: interval.lower,
                        },
                        new_right,
                    ));
                }
                // false R[a,b](f) = G[a,b] f
                ExprKind::False => {
                    return self.visit(&Formula::g(interval.clone(), new_right));
                }
                _ => {}
            }
        }

        // !a R[a,b](a) = G[a,b](a)
        if let Formula::Not(inner) = &new_left
            && inner.eq_structural(&new_right)
        {
            return self.visit(&Formula::g(interval.clone(), new_right.clone()));
        }

        // a R[a,b](!a) = G[a,b](!a)
        if let Formula::Not(inner) = &new_right
            && inner.eq_structural(&new_left)
        {
            return self.visit(&Formula::g(interval.clone(), new_right.clone()));
        }

        formula.clone().with_operand_couple(new_left, new_right)
    }
}

impl Formula {
    fn get_shift(&self) -> Option<i32> {
        match &self {
            Formula::And(operands) | Formula::Or(operands) => operands
                .iter()
                .map(super::Formula::get_shift)
                .min()
                .unwrap_or(None),
            Formula::Imply {
                left,
                right,
                not_left,
            } => left
                .get_shift()
                .min(right.get_shift())
                .min(not_left.get_shift()),
            _ => self.lower_bound(),
        }
    }
}
