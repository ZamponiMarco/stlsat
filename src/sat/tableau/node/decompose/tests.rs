use std::sync::Arc;

use crate::{
    formula::{Expr, Formula, Interval},
    sat::{
        config::{GeneralOptions, TableauOptions},
        tableau::{
            Tableau,
            node::{Node, NodeFormula},
        },
    },
};

fn prop(name: &str) -> Formula {
    Formula::prop(Expr::bool(Arc::from(name)))
}

fn tableau_data_gen(options: Option<TableauOptions>) -> Tableau {
    let general = GeneralOptions::default();
    let tableau = if let Some(tops) = options {
        tops
    } else {
        TableauOptions {
            graph_output: None,
            ..Default::default()
        }
    };
    Tableau::new(general, tableau)
}

fn decompose_jump_opt() -> TableauOptions {
    TableauOptions {
        jump_rule_enabled: true,
        graph_output: None,
        simple_first: false,
        ..Default::default()
    }
}

fn make_test_decompose(
    input: Vec<NodeFormula>,
    expected: Vec<Node>,
    options: Option<TableauOptions>,
) {
    let node = Node::from_operands(input);
    let tableau_data = tableau_data_gen(options);
    let decomposed = tableau_data.decompose(&node).unwrap();
    let decomposed_operands = decomposed
        .iter()
        .map(|n| n.operands.clone())
        .collect::<Vec<Vec<NodeFormula>>>();
    let expected_operands = expected
        .iter()
        .map(|n| n.operands.clone())
        .collect::<Vec<Vec<NodeFormula>>>();
    assert_eq!(decomposed_operands, expected_operands);
}

#[test]
fn test_and() {
    let a = prop("a");
    let b = prop("b");
    let expected: Node = Node::from_operands(vec![a.clone().into(), b.clone().into()]);
    make_test_decompose(vec![Formula::and(vec![a, b]).into()], vec![expected], None);
}

#[test]
fn test_or() {
    let a = prop("a");
    let b = prop("b");
    let expected1: Node = Node::from_operands(vec![a.clone().into()]);
    let expected2: Node = Node::from_operands(vec![b.clone().into()]);
    make_test_decompose(
        vec![Formula::or(vec![a, b]).into()],
        vec![expected1, expected2],
        None,
    );
}

#[test]
fn test_imply() {
    let a = prop("a");
    let b = prop("b");
    let imply = Formula::imply(a.clone(), b.clone());
    let Formula::Imply { not_left, .. } = imply.clone() else {
        panic!()
    };
    let expected1: Node = Node::from_operands(vec![(*not_left.clone()).into()]);
    let expected_optimization: Node = Node::from_operands(vec![a.clone().into(), b.clone().into()]);
    let expected_non_optimization: Node = Node::from_operands(vec![b.clone().into()]);
    make_test_decompose(
        vec![imply.clone().into()],
        vec![expected1.clone(), expected_optimization],
        None,
    );
    make_test_decompose(
        vec![imply.into()],
        vec![expected1, expected_non_optimization],
        Some(TableauOptions {
            formula_optimizations: false,
            ..Default::default()
        }),
    );
}

#[test]
fn test_globally() {
    let a = prop("a");
    let input: NodeFormula = Formula::g(Interval { lower: 0, upper: 5 }, a.clone()).into();
    let expected1: Node = Node::from_operands(vec![
        NodeFormula::from(Formula::g(Interval { lower: 0, upper: 5 }, a.clone())).with_marked(true),
        NodeFormula::from(a.clone()).with_parent_id(Some(input.id)),
    ]);
    make_test_decompose(vec![input], vec![expected1], None);
}

#[test]
fn test_globally_end() {
    let a = prop("a");
    let input: NodeFormula = Formula::g(Interval { lower: 0, upper: 0 }, a.clone()).into();
    let expected: Node = Node::from_operands(vec![
        NodeFormula::from(a.clone()).with_parent_id(Some(input.id)),
    ]);
    make_test_decompose(vec![input], vec![expected], None);
}

#[test]
fn test_finally() {
    let a = prop("a");
    let expected1: Node = Node::from_operands(vec![a.clone().into()]);
    let expected2: Node = Node::from_operands(vec![
        NodeFormula::from(Formula::f(Interval { lower: 0, upper: 5 }, a.clone())).with_marked(true),
    ]);
    make_test_decompose(
        vec![Formula::f(Interval { lower: 0, upper: 5 }, a.clone()).into()],
        vec![expected1, expected2],
        None,
    );
}

#[test]
fn test_finally_end() {
    let a = prop("a");
    let expected1: Node = Node::from_operands(vec![a.clone().into()]);
    make_test_decompose(
        vec![Formula::f(Interval { lower: 0, upper: 0 }, a.clone()).into()],
        vec![expected1],
        None,
    );
}

#[test]
fn test_gf() {
    let a = prop("a");
    let input: NodeFormula = Formula::g(
        Interval { lower: 0, upper: 5 },
        Formula::f(Interval { lower: 0, upper: 5 }, a.clone()),
    )
    .into();

    let expected1: Node = Node::from_operands(vec![
        NodeFormula::from(Formula::g(
            Interval { lower: 0, upper: 5 },
            Formula::f(Interval { lower: 0, upper: 5 }, a.clone()),
        ))
        .with_marked(true),
        NodeFormula::from(Formula::f(Interval { lower: 0, upper: 5 }, a.clone()))
            .with_parent_id(Some(input.id)),
    ]);
    let options = TableauOptions {
        formula_optimizations: false,
        ..Default::default()
    };
    make_test_decompose(vec![input], vec![expected1], Some(options));
}

#[test]
fn test_until() {
    let a = prop("a");
    let b = prop("b");
    let input: NodeFormula =
        Formula::u(Interval { lower: 0, upper: 5 }, a.clone(), b.clone()).into();
    let expected1: Node = Node::from_operands(vec![b.clone().into()]);
    let expected2: Node = Node::from_operands(vec![
        NodeFormula::from(Formula::u(
            Interval { lower: 0, upper: 5 },
            a.clone(),
            b.clone(),
        ))
        .with_marked(true),
        NodeFormula::from(a.clone()).with_parent_id(Some(input.id)),
    ]);
    make_test_decompose(vec![input], vec![expected1, expected2], None);
}

#[test]
fn test_until_end() {
    let a = prop("a");
    let b = prop("b");

    let expected1: Node = Node::from_operands(vec![b.clone().into()]);
    make_test_decompose(
        vec![Formula::u(Interval { lower: 0, upper: 0 }, a.clone(), b.clone()).into()],
        vec![expected1],
        None,
    );
}

#[test]
fn test_release() {
    let a = prop("a");
    let b = prop("b");
    let input: NodeFormula =
        Formula::r(Interval { lower: 0, upper: 5 }, a.clone(), b.clone()).into();
    let expected1: Node = Node::from_operands(vec![a.clone().into(), b.clone().into()]);
    let expected2: Node = Node::from_operands(vec![
        NodeFormula::from(Formula::r(
            Interval { lower: 0, upper: 5 },
            a.clone(),
            b.clone(),
        ))
        .with_marked(true),
        NodeFormula::from(b.clone()).with_parent_id(Some(input.id)),
    ]);
    make_test_decompose(vec![input], vec![expected1, expected2], None);
}

#[test]
fn test_jump_only_prop() {
    let a = prop("a");
    let b = prop("b");
    let to_decompose = Node::from_operands(vec![a.clone().into(), b.clone().into()]);
    let res = tableau_data_gen(Some(decompose_jump_opt())).decompose_jump(&to_decompose);
    assert_eq!(res, None);
}

#[test]
fn test_jump_temporal_end() {
    let a = prop("a");
    let mut to_decompose = Node::from_operands(vec![
        NodeFormula::from(Formula::g(Interval { lower: 0, upper: 5 }, a.clone())).with_marked(true),
        Formula::f(Interval { lower: 3, upper: 5 }, a.clone()).into(),
    ]);
    to_decompose.current_time = 5;
    let res = tableau_data_gen(Some(decompose_jump_opt())).decompose_jump(&to_decompose);
    assert_eq!(res, None);
}

#[test]
fn test_jump_end() {
    let a = prop("a");
    let mut to_decompose = Node::from_operands(vec![
        NodeFormula::from(Formula::f(
            Interval {
                lower: 20,
                upper: 50,
            },
            a.clone(),
        ))
        .with_marked(true),
    ]);
    to_decompose.current_time = 20;
    let res = tableau_data_gen(Some(decompose_jump_opt())).decompose_jump(&to_decompose);
    assert!(res.is_some());
    let vec = res.unwrap();
    assert_eq!(vec.len(), 1);
    let node = &vec[0];

    let expected = Node::from_operands(vec![
        Formula::f(
            Interval {
                lower: 20,
                upper: 50,
            },
            a.clone(),
        )
        .into(),
    ]);

    assert_eq!(node.current_time, 50);
    assert_eq!(node.operands, expected.operands);
}
