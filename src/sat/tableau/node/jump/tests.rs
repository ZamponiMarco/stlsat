
use std::sync::Arc;

use crate::{
    formula::{Expr, Formula, Interval},
    sat::tableau::node::{Node, NodeFormula},
};

fn prop(name: &str) -> Formula {
    Formula::prop(Expr::bool(Arc::from(name)))
}

#[test]
fn max_jump_one_returns_one() {
    let node = Node::from_operands(vec![]);
    assert_eq!(node.calculate_k_star(1), 1);
    assert_eq!(node.calculate_k_star(0), 1);
}

#[test]
fn empty_node_returns_max_jump() {
    let node = Node::from_operands(vec![]);
    assert_eq!(node.calculate_k_star(10), 10);
}

#[test]
fn intersection_completeness_forces_one() {
    let mut node = Node::from_operands(vec![
        NodeFormula::from(Formula::f(Interval { lower: 0, upper: 5 }, prop("a"))).with_marked(true),
        Formula::g(Interval { lower: 0, upper: 0 }, prop("b")).into(),
    ]);
    node.current_time = 0;
    assert_eq!(node.calculate_k_star(10), 1);
}

#[test]
fn intersection_soundness_forces_one() {
    let mut node = Node::from_operands(vec![
        NodeFormula::from(Formula::g(Interval { lower: 0, upper: 5 }, prop("a"))).with_marked(true),
        Formula::g(Interval { lower: 0, upper: 3 }, prop("b")).into(),
    ]);
    node.current_time = 0;
    assert_eq!(node.calculate_k_star(10), 1);
}

#[test]
fn gap_completeness_computed() {
    let mut node = Node::from_operands(vec![
        NodeFormula::from(Formula::f(Interval { lower: 3, upper: 8 }, prop("b"))).with_marked(true),
        Formula::g(Interval { lower: 0, upper: 5 }, prop("a")).into(),
    ]);
    node.current_time = 0;
    assert_eq!(node.calculate_k_star(10), 5);
}

#[test]
fn gap_soundness_computed() {
    let mut node = Node::from_operands(vec![
        NodeFormula::from(Formula::g(Interval { lower: 0, upper: 3 }, prop("a"))).with_marked(true),
        Formula::f(Interval { lower: 5, upper: 8 }, prop("b")).into(),
    ]);
    node.current_time = 0;
    assert_eq!(node.calculate_k_star(10), 5);
}
