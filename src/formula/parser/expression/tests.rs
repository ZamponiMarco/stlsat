use std::sync::Arc;

use crate::formula::{AExpr, Expr, ExprKind, Formula, RelOp, parser::parse_formula};

#[test]
fn test_parse_simple_proposition() {
    let input = "a";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::bool(Arc::from("a")));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_true() {
    let input = "TRUE";
    let result = parse_formula(input);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&Formula::Prop(Expr::true_expr()))));
}

#[test]
fn test_parse_false() {
    let input = "FALSE";
    let result = parse_formula(input);
    assert!(result.is_ok_and(|f| f.1.eq_structural(&Formula::Prop(Expr::false_expr()))));
}

#[test]
fn test_parse_decimal() {
    let input = "x > 0.75";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Gt,
        left: AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
        right: AExpr::Num(num_rational::Ratio::new(3, 4)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_fraction() {
    let input = "x > 3/4";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Gt,
        left: AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
        right: AExpr::Num(num_rational::Ratio::new(3, 4)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_negative_decimal() {
    let input = "x > -1.05";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Gt,
        left: AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
        right: AExpr::Num(num_rational::Ratio::new(-21, 20)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_decimal_leading_zeros() {
    let input = "x > 0.005";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Gt,
        left: AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
        right: AExpr::Num(num_rational::Ratio::new(1, 200)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_affine_expression() {
    let input = "2*x - 3/4*y <= 5";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Le,
        left: AExpr::BinOp {
            op: crate::formula::ArithOp::Sub,
            left: Box::new(AExpr::Var(
                Arc::from("x"),
                num_rational::Ratio::from_integer(2),
            )),
            right: Box::new(AExpr::Var(Arc::from("y"), num_rational::Ratio::new(3, 4))),
        },
        right: AExpr::Num(num_rational::Ratio::from_integer(5)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}

#[test]
fn test_parse_negative_decimal_coefficient() {
    let input = "-0.5*x == 1";
    let result = parse_formula(input);
    let expected = Formula::Prop(Expr::from_expr(ExprKind::Rel {
        op: RelOp::Eq,
        left: AExpr::Var(Arc::from("x"), num_rational::Ratio::new(-1, 2)),
        right: AExpr::Num(num_rational::Ratio::from_integer(1)),
    }));
    assert!(result.is_ok_and(|f| f.1.eq_structural(&expected)));
}
