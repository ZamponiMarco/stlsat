use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use dot_graph::{Graph, Kind};

use crate::formula::parser::parse_formula;
use crate::sat::config::{GeneralOptions, TableauOptions};
use crate::sat::tableau::core::UnsatCore;
use crate::sat::tableau::node::{Node, NodeFormula};
use crate::sat::tableau::solver::Solver;
use crate::sat::tableau::store::Store;
use crate::sat::tableau::trace::{Trace, TraceBuilder};

#[cfg(test)]
mod tests;

pub mod core;
pub mod graph;
pub mod node;
pub mod solver;
pub mod store;
pub mod trace;

pub struct Tableau {
    pub options: GeneralOptions,
    pub tableau_options: TableauOptions,
    pub graph: Option<Graph>,
    pub store: Option<Store>,
    pub unsat_core: Option<UnsatCore>,
    trace_builder: Option<TraceBuilder>,
    pub trace: Option<Trace>,
}

#[derive(Clone, Copy)]
enum JobState {
    Sat,
    Unsat,
    Undefined,
}

enum JobOutcome {
    Decomposed(Frame),
    Final(JobState),
}

struct Frame {
    node: Node,
    children: VecDeque<Node>,
    depth: usize,
    solver: Rc<RefCell<Solver>>,
    result: Option<JobState>,
}

impl Tableau {
    #[must_use]
    pub fn new(options: GeneralOptions, tableau_options: TableauOptions) -> Self {
        let graph = if tableau_options.graph_output.is_some() {
            Some(Graph::new("Tableau", Kind::Graph))
        } else {
            None
        };
        let store = if tableau_options.memoization {
            Some(Store::new())
        } else {
            None
        };
        let unsat_core = if tableau_options.unsat_core_extraction {
            Some(UnsatCore::new())
        } else {
            None
        };
        let trace = if tableau_options.trace_extraction {
            Some(TraceBuilder::new())
        } else {
            None
        };
        Tableau {
            options,
            tableau_options,
            graph,
            store,
            unsat_core,
            trace_builder: trace,
            trace: None,
        }
    }

    pub fn make_tableau_from_str(&mut self, formula: &str) -> Option<bool> {
        // Parsing Stage
        let formula_ast = match parse_formula(formula) {
            Ok((remaining, formula_ast)) => {
                if !remaining.trim().is_empty() {
                    panic!(
                        "Unparsed input remaining after parsing formula: {}",
                        remaining
                    );
                }
                formula_ast
            }
            Err(e) => {
                panic!("Failed to parse formula, parse error: {}", e);
            }
        };

        let root = Node::from_operands(vec![NodeFormula::from(formula_ast)]);

        self.make_tableau_from_root(root)
    }

    pub fn make_tableau_from_root(&mut self, mut root: Node) -> Option<bool> {
        self.normalize_root(&mut root);
        self.initialize_root(&root);
        self.solve_root(root)
    }

    fn normalize_root(&self, root: &mut Node) {
        root.negative_normal_form_rewrite();

        if !self.options.mltl {
            root.mltl_rewrite();
        }

        if self.tableau_options.formula_simplifications {
            root.simplify();
        }

        root.flatten();

        if self.tableau_options.formula_optimizations {
            root.shift_bounds();
        }
    }

    fn initialize_root(&mut self, root: &Node) {
        if let Some(core) = &mut self.unsat_core {
            core.initialize_root_node(root);
        }
        self.add_graph_node(root);
    }

    fn solve_root(&mut self, root: Node) -> Option<bool> {
        let mut solver = Solver::factory(
            self.tableau_options.unsat_core_extraction,
            self.options.mltl,
            self.tableau_options.solver,
            &root,
        );
        solver.push();

        if !solver.check(&root) {
            return Some(false);
        }

        let Some(children) = self.decompose(&root) else {
            return Some(true);
        };
        self.add_graph_children(&root, &children);

        self.tableau_loop(root, children, solver)
    }

    fn tableau_loop(&mut self, root: Node, children: Vec<Node>, solver: Solver) -> Option<bool> {
        fn merge_results(
            previous: Option<JobState>,
            current: JobState,
            implies: bool,
        ) -> Option<JobState> {
            match (previous, current) {
                (prev, JobState::Sat) => {
                    if implies {
                        prev
                    } else {
                        Some(JobState::Sat)
                    }
                }

                (Some(JobState::Sat), JobState::Undefined) => Some(JobState::Sat),
                (_, JobState::Undefined) => Some(JobState::Undefined),

                (Some(JobState::Sat), JobState::Unsat) => Some(JobState::Sat),
                (Some(JobState::Undefined), JobState::Unsat) => Some(JobState::Undefined),
                (_, JobState::Unsat) => Some(JobState::Unsat),
            }
        }

        let mut stack = VecDeque::new();
        stack.push_front(Frame {
            node: root,
            children: children.into(),
            depth: 0,
            solver: Rc::new(RefCell::new(solver)),
            result: None,
        });

        while let Some(mut job) = stack.pop_front() {
            // Case 1: no more children — finalize frame
            let Some(child) = job.children.pop_front() else {
                // Case 1.1: no parent — done
                let Some(parent) = stack.front_mut() else {
                    return match job.result {
                        Some(JobState::Sat) => {
                            if let Some(trace) = &mut self.trace_builder
                                && job.node.is_poised()
                            {
                                trace.add_node(&job.node);
                            }
                            if let Some(trace) = self.trace_builder.take() {
                                self.trace = Some(trace.freeze());
                            }
                            Some(true)
                        }
                        Some(JobState::Unsat) => Some(false),
                        Some(JobState::Undefined) => None,
                        None => panic!(),
                    };
                };

                // Case 1.2: has parent — propagate result
                parent.solver.borrow_mut().pop();
                let res = job.result.expect("Job result should be set");
                let implies = job.node.implies.is_some();
                parent.result = merge_results(parent.result, res, implies);

                match res {
                    JobState::Sat => {
                        if implies {
                            if let Some(trace) = &mut self.trace_builder {
                                trace.reset();
                            }
                        } else {
                            parent.children.clear();
                            if let Some(trace) = &mut self.trace_builder
                                && job.node.is_poised()
                            {
                                trace.add_node(&job.node);
                            }
                        }
                    }
                    JobState::Unsat => {
                        if implies {
                            parent.children.clear();
                        }
                        if parent.node.current_time < job.node.current_time
                            && let Some(store) = &mut self.store
                        {
                            store.add_rejected((&job.node).into());
                        }
                    }
                    _ => {}
                }
                continue;
            };

            // Case 2: still has children — process next child
            job.solver.borrow_mut().push();
            let implies = child.implies.is_some();
            let outcome =
                self.process_job(child, job.node.current_time, &mut job.solver, job.depth);

            match outcome {
                // Case 2.1: child has result — handle and re-push job
                JobOutcome::Final(res) => {
                    match res {
                        JobState::Sat if !implies => {
                            job.children.clear();
                            job.result = Some(JobState::Sat);
                        }
                        JobState::Undefined => job.result = Some(JobState::Undefined),
                        JobState::Unsat => {
                            job.result = Some(JobState::Unsat);
                            if job.node.implies.is_some() {
                                job.children.clear();
                            }
                        }
                        _ => {}
                    }
                    job.solver.borrow_mut().pop();
                    stack.push_front(job);
                }
                // Case 2.2: child needs decomposition — push both jobs in order
                JobOutcome::Decomposed(new_job) => {
                    stack.push_front(job);
                    stack.push_front(new_job);
                }
            }
        }
        panic!("Tableau loop exited unexpectedly");
    }

    fn process_job(
        &mut self,
        node: Node,
        parent_time: i32,
        solver: &mut Rc<RefCell<Solver>>,
        depth: usize,
    ) -> JobOutcome {
        if depth >= self.tableau_options.max_depth {
            return JobOutcome::Final(JobState::Undefined);
        }

        if !solver.borrow_mut().check(&node) {
            if let Some(core) = &mut self.unsat_core
                && let Some(new_core) = solver.borrow_mut().extract_unsat_core()
            {
                core.add_to_unsat_core(new_core);
            }
            return JobOutcome::Final(JobState::Unsat);
        }

        if let Some(store) = &mut self.store
            && parent_time < node.current_time
        {
            let rejected_node = (&node).into();
            if store.check_rejected(&rejected_node) {
                return JobOutcome::Final(JobState::Unsat);
            }
        }

        let Some(children) = self.decompose(&node) else {
            return JobOutcome::Final(JobState::Sat);
        };

        self.add_graph_children(&node, &children);

        let solver_ref = if parent_time < node.current_time {
            Rc::new(RefCell::new(solver.borrow().empty_solver()))
        } else {
            solver.clone()
        };

        let job = Frame {
            node,
            children: children.into(),
            depth: depth + 1,
            solver: solver_ref,
            result: None,
        };
        JobOutcome::Decomposed(job)
    }
}
