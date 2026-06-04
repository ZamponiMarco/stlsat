use std::collections::{BTreeSet, HashSet};

use crate::{
    formula::{Formula, Interval},
    sat::tableau::node::NodeFormula,
};

use super::{Node, intervals::PropositionValidityInterval};

#[cfg(test)]
mod tests;

impl Node {
    fn compute_target_set(&self) -> HashSet<PropositionValidityInterval> {
        let mut targets = HashSet::new();

        for operand in &self.operands {
            if !operand.marked {
                continue;
            }

            match &operand.kind {
                Formula::U { right, .. } => {
                    targets.extend(right.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::R { left, .. } => {
                    targets.extend(left.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::F { phi, .. } => {
                    targets.extend(phi.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                _ => {}
            }
        }
        targets
    }

    fn compute_obstacle_set(&self) -> HashSet<PropositionValidityInterval> {
        let mut obstacles = HashSet::new();

        for operand in &self.operands {
            if operand.is_parent_active_in(self) {
                continue;
            }

            match &operand.kind {
                Formula::R {
                    right: phi,
                    interval,
                    ..
                }
                | Formula::U {
                    interval,
                    left: phi,
                    ..
                }
                | Formula::G { interval, phi } => {
                    obstacles.extend(phi.proposition_full_interval(interval.clone()));
                }
                Formula::Not(phi) => {
                    if let Formula::Prop(e) = &**phi {
                        obstacles.insert(PropositionValidityInterval {
                            expr: e.clone(),
                            interval: Interval {
                                lower: self.current_time,
                                upper: self.current_time,
                            },
                        });
                    } else {
                        panic!("Unexpected formula inside Not: {:?}", phi);
                    };
                }
                Formula::Prop(e) => {
                    obstacles.insert(PropositionValidityInterval {
                        expr: e.clone(),
                        interval: Interval {
                            lower: self.current_time,
                            upper: self.current_time,
                        },
                    });
                }
                _ => {}
            }
        }

        obstacles
    }

    fn compute_n(&self) -> HashSet<PropositionValidityInterval> {
        let mut n_set = HashSet::new();

        for operand in &self.operands {
            if !operand.marked {
                continue;
            }

            match &operand.kind {
                Formula::U { left, .. } => {
                    n_set.extend(left.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::R { right, .. } => {
                    n_set.extend(right.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                Formula::G { phi, .. } => {
                    n_set.extend(phi.proposition_full_interval(Interval {
                        lower: self.current_time,
                        upper: self.current_time,
                    }));
                }
                _ => {}
            }
        }
        n_set
    }

    fn compute_o(&self) -> HashSet<PropositionValidityInterval> {
        let mut o_set = HashSet::new();

        for operand in &self.operands {
            if operand.is_parent_active_in(self) {
                continue;
            }

            o_set.extend(
                operand
                    .kind
                    .proposition_full_interval(Interval { lower: 0, upper: 0 }),
            );
        }

        o_set
    }

    fn compute_jump_bound(
        targets: &HashSet<PropositionValidityInterval>,
        obstacles: &HashSet<PropositionValidityInterval>,
    ) -> i32 {
        let cross = || {
            targets.iter().flat_map(|a| {
                obstacles
                    .iter()
                    .filter(move |b| a.expr.id != b.expr.id)
                    .map(move |b| (a, b))
            })
        };
        if cross().any(|(a, b)| a.interval.intersects(&b.interval)) {
            return 1;
        }
        cross()
            .map(|(a, b)| b.interval.lower - a.interval.upper)
            .filter(|&k| k >= 1)
            .min()
            .unwrap_or(i32::MAX)
    }

    pub fn compute_k(&self) -> i32 {
        fn sorted_time_instants(node: &Node) -> BTreeSet<i32> {
            fn top_level_interval(formula: &NodeFormula, node: &Node) -> Option<Vec<i32>> {
                match &formula.kind {
                    Formula::G { interval, .. } | Formula::R { interval, .. }
                        if !formula.is_parent_active_in(node) =>
                    {
                        Some(vec![interval.lower, interval.upper])
                    }
                    Formula::F { interval, .. } | Formula::U { interval, .. } => {
                        Some(vec![interval.lower, interval.upper])
                    }
                    _ => None,
                }
            }

            node.operands
                .iter()
                .filter_map(|f| top_level_interval(f, node))
                .flatten()
                .collect()
        }

        if let Some(target_time) = sorted_time_instants(self)
            .into_iter()
            .find(|&t| t > self.current_time)
        {
            target_time - self.current_time
        } else {
            i32::MAX
        }
    }

    pub fn calculate_k_star(&self) -> i32 {
        let max_jump = self.compute_k();

        if max_jump <= 1 {
            return 1;
        }

        let completeness_targets = self.compute_target_set();
        let completeness_obstacles = self.compute_obstacle_set();

        let jump_complete =
            Self::compute_jump_bound(&completeness_targets, &completeness_obstacles);

        if jump_complete <= 1 {
            return 1;
        }

        let soundness_targets = self.compute_n();
        let soundness_obstacles = self.compute_o();

        let jump_sound = Self::compute_jump_bound(&soundness_targets, &soundness_obstacles);

        max_jump.min(jump_complete).min(jump_sound)
    }
}
