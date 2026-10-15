use std::sync::Arc;

use crate::formula::{Expr, Formula, Interval, parser::parse_formula};

#[test]
fn test_parse_negation() {
    let input = "!a";
    let result = parse_formula(input);
    let expected = Formula::Not(Box::new(Formula::Prop(Expr::bool(Arc::from("a")))));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_conjunction() {
    let input = "a && b";
    let result = parse_formula(input);
    let expected = Formula::And(vec![
        Formula::Prop(Expr::bool(Arc::from("a"))),
        Formula::Prop(Expr::bool(Arc::from("b"))),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_disjunction() {
    let input = "a || b";
    let result = parse_formula(input);
    let expected = Formula::Or(vec![
        Formula::Prop(Expr::bool(Arc::from("a"))),
        Formula::Prop(Expr::bool(Arc::from("b"))),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_implication() {
    let input = "a -> b";
    let result = parse_formula(input);
    let expected = Formula::Imply {
        left: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        right: Box::new(Formula::Prop(Expr::bool(Arc::from("b")))),
        not_left: Box::new(Formula::Not(Box::new(Formula::Prop(Expr::bool(
            Arc::from("a"),
        ))))),
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_globally() {
    let input = "G[0,10] a";
    let result = parse_formula(input);
    let expected = Formula::G {
        phi: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        interval: Interval {
            lower: 0,
            upper: 10,
        },
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_finally() {
    let input = "F[1,5] b";
    let result = parse_formula(input);
    let expected = Formula::F {
        phi: Box::new(Formula::Prop(Expr::bool(Arc::from("b")))),
        interval: Interval { lower: 1, upper: 5 },
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_until() {
    let input = "a U[2,8] b";
    let result = parse_formula(input);
    let expected = Formula::U {
        left: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        right: Box::new(Formula::Prop(Expr::bool(Arc::from("b")))),
        interval: Interval { lower: 2, upper: 8 },
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_release() {
    let input = "a R[3,7] b";
    let result = parse_formula(input);
    let expected = Formula::R {
        left: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        right: Box::new(Formula::Prop(Expr::bool(Arc::from("b")))),
        interval: Interval { lower: 3, upper: 7 },
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_with_parentheses() {
    let input = "(a && b)";
    let result = parse_formula(input);
    let expected = Formula::And(vec![
        Formula::Prop(Expr::bool(Arc::from("a"))),
        Formula::Prop(Expr::bool(Arc::from("b"))),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_parentheses_nested() {
    let input = "(TRUE -> (a -> b))";
    let result = parse_formula(input);
    let inner_imply = Formula::Imply {
        left: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        right: Box::new(Formula::Prop(Expr::bool(Arc::from("b")))),
        not_left: Box::new(Formula::Not(Box::new(Formula::Prop(Expr::bool(
            Arc::from("a"),
        ))))),
    };
    let expected = Formula::Imply {
        left: Box::new(Formula::Prop(Expr::true_expr())),
        right: Box::new(inner_imply),
        not_left: Box::new(Formula::Not(Box::new(Formula::Prop(Expr::true_expr())))),
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_parentheses_space() {
    let input = "( a && b )";
    let result = parse_formula(input);
    let expected = Formula::And(vec![
        Formula::Prop(Expr::bool(Arc::from("a"))),
        Formula::Prop(Expr::bool(Arc::from("b"))),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_single_mid_or() {
    let input = "a | b | c";
    let result = parse_formula(input);
    let expected = Formula::Or(vec![
        Formula::Or(vec![
            Formula::Prop(Expr::bool(Arc::from("a"))),
            Formula::Prop(Expr::bool(Arc::from("b"))),
        ]),
        Formula::Prop(Expr::bool(Arc::from("c"))),
    ]);
    println!("{:?}", result);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_single_and() {
    let input = "a & b & c";
    let result = parse_formula(input);
    let expected = Formula::And(vec![
        Formula::And(vec![
            Formula::Prop(Expr::bool(Arc::from("a"))),
            Formula::Prop(Expr::bool(Arc::from("b"))),
        ]),
        Formula::Prop(Expr::bool(Arc::from("c"))),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_iff() {
    let input = "a <-> b";
    let result = parse_formula(input);
    let left = Formula::Prop(Expr::bool(Arc::from("a")));
    let right = Formula::Prop(Expr::bool(Arc::from("b")));
    let expected = Formula::or(vec![
        Formula::and(vec![left.clone(), right.clone()]),
        Formula::and(vec![
            Formula::Not(Box::new(left)),
            Formula::Not(Box::new(right)),
        ]),
    ]);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_next() {
    let input = "X a";
    let result = parse_formula(input);
    let expected = Formula::F {
        phi: Box::new(Formula::Prop(Expr::bool(Arc::from("a")))),
        interval: Interval { lower: 1, upper: 1 },
    };
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}
