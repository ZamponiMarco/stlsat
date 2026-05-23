use std::collections::HashSet;
use std::sync::Arc;

use crate::formula::{Expr, Formula, Interval};
use crate::sat::tableau::node::intervals::PropositionValidityInterval;

fn expr(name: &str) -> Expr {
    Expr::bool(Arc::from(name))
}

fn interval(lower: i32, upper: i32) -> Interval {
    Interval { lower, upper }
}

fn validity(expr: &Expr, interval: Interval) -> PropositionValidityInterval {
    PropositionValidityInterval {
        expr: expr.clone(),
        interval,
    }
}

fn assert_intervals(
    formula: &Formula,
    delta: Interval,
    expected: Vec<PropositionValidityInterval>,
) {
    let expected = expected.into_iter().collect::<HashSet<_>>();
    assert_eq!(formula.proposition_full_interval(delta), expected);
}

#[test]
fn interval_proposition() {
    let a = expr("a");

    let formula = Formula::prop(a.clone());
    assert_intervals(&formula, interval(0, 0), vec![validity(&a, interval(0, 0))]);
}

#[test]
fn interval_not() {
    let a = expr("a");

    let formula = Formula::not(Formula::prop(a.clone()));
    assert_intervals(&formula, interval(0, 0), vec![validity(&a, interval(0, 0))]);
}

#[test]
fn interval_binary() {
    let a = expr("a");
    let b = expr("b");

    let formula_and = Formula::and(vec![
        Formula::prop(a.clone()),
        Formula::not(Formula::prop(b.clone())),
    ]);
    assert_intervals(
        &formula_and,
        interval(0, 0),
        vec![validity(&a, interval(0, 0)), validity(&b, interval(0, 0))],
    );

    let formula_or = Formula::or(vec![
        Formula::prop(a.clone()),
        Formula::not(Formula::prop(b.clone())),
    ]);
    assert_intervals(
        &formula_or,
        interval(0, 0),
        vec![validity(&a, interval(0, 0)), validity(&b, interval(0, 0))],
    );
}

#[test]
fn interval_imply() {
    let a = expr("a");
    let b = expr("b");

    let formula = Formula::imply(Formula::prop(a.clone()), Formula::prop(b.clone()));
    let not_a = if let Formula::Imply { not_left, .. } = &formula {
        if let Formula::Not(not_a) = not_left.as_ref() {
            if let Formula::Prop(e) = not_a.as_ref() {
                e.clone()
            } else {
                panic!("Expected a proposition inside the Not formula");
            }
        } else {
            panic!("Expected a Not formula for the left operand of the implication");
        }
    } else {
        panic!("Expected an implication formula");
    };

    assert_intervals(
        &formula,
        interval(0, 0),
        vec![
            validity(&not_a, interval(0, 0)),
            validity(&b, interval(0, 0)),
        ],
    );
}

#[test]
fn interval_temporal_unary() {
    let a = expr("a");

    let formula_globally = Formula::g(interval(2, 5), Formula::prop(a.clone()));
    assert_intervals(
        &formula_globally,
        interval(10, 20),
        vec![validity(&a, interval(12, 25))],
    );

    let formula_finally = Formula::f(interval(2, 5), Formula::prop(a.clone()));
    assert_intervals(
        &formula_finally,
        interval(10, 20),
        vec![validity(&a, interval(12, 25))],
    );
}

#[test]
fn interval_temporal_binary() {
    let a = expr("a");
    let b = expr("b");

    let formula_until = Formula::u(
        interval(2, 5),
        Formula::prop(a.clone()),
        Formula::prop(b.clone()),
    );
    assert_intervals(
        &formula_until,
        interval(10, 20),
        vec![
            validity(&a, interval(12, 24)),
            validity(&b, interval(12, 25)),
        ],
    );

    let formula_release = Formula::r(
        interval(2, 5),
        Formula::prop(a.clone()),
        Formula::prop(b.clone()),
    );
    assert_intervals(
        &formula_release,
        interval(10, 20),
        vec![
            validity(&a, interval(12, 24)),
            validity(&b, interval(12, 25)),
        ],
    );
}

#[test]
fn interval_temporal_binary_one_step() {
    let a = expr("a");
    let b = expr("b");

    let formula_until = Formula::u(
        interval(5, 5),
        Formula::prop(a.clone()),
        Formula::prop(b.clone()),
    );
    assert_intervals(
        &formula_until,
        interval(0, 2),
        vec![validity(&b, interval(5, 7))],
    );

    let formula_release = Formula::r(
        interval(5, 5),
        Formula::prop(a.clone()),
        Formula::prop(b.clone()),
    );
    assert_intervals(
        &formula_release,
        interval(0, 2),
        vec![validity(&b, interval(5, 7))],
    );
}

#[test]
fn interval_nested() {
    let a = expr("a");

    let formula = Formula::g(
        interval(1, 3),
        Formula::f(interval(2, 4), Formula::prop(a.clone())),
    );
    assert_intervals(
        &formula,
        interval(10, 12),
        vec![validity(&a, interval(13, 19))],
    );
}
