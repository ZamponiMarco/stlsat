use std::sync::Arc;

use num_rational::Ratio;

use crate::{
    formula::{Expr, Formula, parser::parse_formula},
    sat::tableau::{
        node::Node,
        solver::{RealSolver, Solver},
    },
};

fn parse_node(input: &str) -> Node {
    let (_, formula) = parse_formula(input).unwrap();
    let mut node = Node::from_operands(vec![formula.into()]);
    node.flatten();
    node
}

fn make_solver_test(input: &str) -> bool {
    let mut solver = Solver::new(false, false);
    solver.check(&parse_node(input))
}

#[test]
fn test_false_expr() {
    assert!(!make_solver_test("false"));
}

#[test]
fn test_not_true_expr() {
    assert!(!make_solver_test("!true"));
}

#[test]
fn test_bool_true() {
    assert!(make_solver_test("a && b"));
}

#[test]
fn test_bool_false() {
    assert!(!make_solver_test("a && !a"));
}

#[test]
fn test_real_true() {
    assert!(make_solver_test("R_x > 0 && R_x < 5"));
}

#[test]
fn test_real_false() {
    assert!(!make_solver_test("R_x > 5 && R_x < 0"));
}

#[test]
fn test_push_pop_bool() {
    let mut solver = Solver::new(false, false);

    solver.push();

    let node = parse_node("a && b");
    assert!(solver.check(&node));

    solver.push();

    let node_false = parse_node("!a, a");
    assert!(!solver.check(&node_false));

    solver.pop();

    solver.push();
    let node_true = parse_node("c");
    assert!(solver.check(&node_true));

    solver.push();
    let node_false_2 = parse_node("!a");
    assert!(!solver.check(&node_false_2));
}

#[test]
fn test_push_pop_real() {
    let mut solver = Solver::new(false, false);

    solver.push();

    let node = parse_node("R_x > 0 && R_x < 5");
    assert!(solver.check(&node));

    solver.push();

    let node_false = parse_node("R_x < 0");
    assert!(!solver.check(&node_false));

    solver.pop();

    solver.push();
    let node_true = parse_node("R_y > 1");
    assert!(solver.check(&node_true));

    solver.push();
    let node_false_2 = parse_node("R_x < 0");
    assert!(!solver.check(&node_false_2));
}

#[test]
#[should_panic]
fn mltl_real_constraint() {
    let mut solver = Solver::new(false, true);
    let node = parse_node("R_x > 0");
    solver.check(&node);
}

#[test]
fn mltl_boolean() {
    let mut solver = Solver::new(false, true);
    let node = parse_node("a && b");
    assert!(solver.check(&node));
}

#[test]
fn empty_solver_not_mltl() {
    let solver = Solver::new(false, false);
    assert!(matches!(solver.real_solver, RealSolver::Z3(_)));
    let empty_solver = solver.empty_solver();
    assert!(matches!(empty_solver.real_solver, RealSolver::Z3(_)));
}

#[test]
fn empty_solver_mltl() {
    let solver = Solver::new(false, true);
    assert!(matches!(solver.real_solver, RealSolver::Empty));
    let empty_solver = solver.empty_solver();
    assert!(matches!(empty_solver.real_solver, RealSolver::Empty));
}

#[test]
fn test_unsat_core_not_enabled() {
    let mut solver = Solver::new(false, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::not(Formula::prop(Expr::bool(Arc::from("a"))));

    let node = Node::from_operands(vec![one.into(), two.into()]);

    assert!(!solver.check(&node));
    assert_eq!(solver.extract_unsat_core(), None);
}

#[test]
fn test_unsat_core_bool() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::not(Formula::prop(Expr::bool(Arc::from("a"))));

    let node = Node::from_operands(vec![one.clone().into(), two.clone().into()]);

    assert!(!solver.check(&node));

    let id = if let Formula::Prop(prop) = one {
        prop.id
    } else {
        unreachable!()
    };
    let id2 = if let Formula::Not(inner) = two
        && let Formula::Prop(expr) = *inner
    {
        expr.id
    } else {
        unreachable!()
    };

    assert_eq!(solver.extract_unsat_core(), Some(vec![id, id2]));
}

#[test]
fn test_unsat_core_bool_sat() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::prop(Expr::bool(Arc::from("b")));

    let node = Node::from_operands(vec![one.into(), two.into()]);

    assert!(solver.check(&node));
    assert_eq!(solver.extract_unsat_core(), None);
}

#[test]
fn test_unsat_core_bool_one_excluded() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::not(Formula::prop(Expr::bool(Arc::from("a"))));
    let three = Formula::prop(Expr::bool(Arc::from("b")));

    let node = Node::from_operands(vec![
        one.clone().into(),
        two.clone().into(),
        three.clone().into(),
    ]);

    assert!(!solver.check(&node));
    let core = solver.extract_unsat_core().unwrap();
    assert_eq!(core.len(), 2);

    let id = if let Formula::Prop(expr) = one {
        expr.id
    } else {
        unreachable!()
    };
    let id2 = if let Formula::Not(inner) = two
        && let Formula::Prop(expr) = *inner
    {
        expr.id
    } else {
        unreachable!()
    };

    assert!(core.contains(&id));
    assert!(core.contains(&id2));
}

#[test]
fn test_unsat_core_real() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(5)),
    ));
    let two = Formula::prop(Expr::real(
        crate::formula::RelOp::Le,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(0)),
    ));

    let node = Node::from_operands(vec![one.clone().into(), two.clone().into()]);

    let id = if let Formula::Prop(expr) = one {
        expr.id
    } else {
        unreachable!()
    };
    let id2 = if let Formula::Prop(expr) = two {
        expr.id
    } else {
        unreachable!()
    };

    assert!(!solver.check(&node));
    assert_eq!(solver.extract_unsat_core(), Some(vec![id, id2]));
}

#[test]
fn test_unsat_core_real_sat() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(0)),
    ));
    let two = Formula::prop(Expr::real(
        crate::formula::RelOp::Le,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(5)),
    ));

    let node = Node::from_operands(vec![one.into(), two.into()]);

    assert!(solver.check(&node));
    assert_eq!(solver.extract_unsat_core(), None);
}

#[test]
fn test_unsat_core_real_one_excluded() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(5)),
    ));
    let two = Formula::prop(Expr::real(
        crate::formula::RelOp::Le,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(0)),
    ));
    let three = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("y")),
        crate::formula::AExpr::Num(Ratio::from_integer(1)),
    ));

    let node = Node::from_operands(vec![one.clone().into(), two.clone().into(), three.into()]);

    let id = if let Formula::Prop(expr) = one {
        expr.id
    } else {
        unreachable!()
    };
    let id2 = if let Formula::Prop(expr) = two {
        expr.id
    } else {
        unreachable!()
    };

    assert!(!solver.check(&node));
    let core = solver.extract_unsat_core().unwrap();
    assert_eq!(core.len(), 2);
    assert!(core.contains(&id));
    assert!(core.contains(&id2));
}

#[test]
fn test_unsat_core_false() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::prop(Expr::false_expr());
    let three = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(5)),
    ));

    let node = Node::from_operands(vec![
        one.clone().into(),
        two.clone().into(),
        three.clone().into(),
    ]);

    assert!(!solver.check(&node));

    let id2 = if let Formula::Prop(expr) = two {
        expr.id
    } else {
        unreachable!()
    };

    let core = solver.extract_unsat_core().unwrap();
    assert!(core.contains(&id2));
}

#[test]
fn test_unsat_core_not_true() {
    let mut solver = Solver::new(true, false);

    let one = Formula::prop(Expr::bool(Arc::from("a")));
    let two = Formula::not(Formula::prop(Expr::true_expr()));
    let three = Formula::prop(Expr::real(
        crate::formula::RelOp::Ge,
        crate::formula::AExpr::Var(Arc::from("x")),
        crate::formula::AExpr::Num(Ratio::from_integer(5)),
    ));

    let node = Node::from_operands(vec![
        one.clone().into(),
        two.clone().into(),
        three.clone().into(),
    ]);

    assert!(!solver.check(&node));

    let id2 = if let Formula::Not(inner) = two
        && let Formula::Prop(expr) = *inner
    {
        expr.id
    } else {
        unreachable!()
    };

    let core = solver.extract_unsat_core().unwrap();
    assert!(core.contains(&id2));
}
