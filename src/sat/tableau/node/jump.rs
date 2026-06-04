use std::collections::HashSet;

use crate::formula::{Formula, Interval};

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
        max_jump: i32,
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
            .unwrap_or(max_jump)
    }

    pub fn calculate_k_star(&self, max_jump: i32) -> i32 {
        if max_jump <= 1 {
            return 1;
        }

        let completeness_targets = self.compute_target_set();
        let completeness_obstacles = self.compute_obstacle_set();
        let soundness_targets = self.compute_n();
        let soundness_obstacles = self.compute_o();

        let jump_complete =
            Self::compute_jump_bound(&completeness_targets, &completeness_obstacles, max_jump);
        let jump_sound =
            Self::compute_jump_bound(&soundness_targets, &soundness_obstacles, max_jump);

        jump_complete.min(jump_sound)
    }
}
