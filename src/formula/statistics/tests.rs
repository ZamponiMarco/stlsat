use num_rational::Ratio;
use std::sync::Arc;

use crate::formula::{AExpr, Expr, Interval, RelOp, statistics::Formula};

fn prop(name: &str) -> Formula {
    Formula::prop(Expr::bool(Arc::from(name)))
}

mod temporal_operator_depth_tests {
    use super::*;

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.temporal_operator_depth(), 2);
    }
}

mod depth_tests {
    use super::*;

    #[test]
    fn test_prop() {
        let f = prop("a");
        assert_eq!(f.depth(), 0);
    }

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.depth(), 3);
    }
}

mod horizon_tests {
    use super::*;

    #[test]
    fn test_prop() {
        let f = prop("a");
        assert_eq!(f.horizon(), 0);
    }

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.horizon(), 8);
    }
}

mod nodes_tests {
    use super::*;

    #[test]
    fn test_prop() {
        let f = prop("a");
        assert_eq!(f.nodes(), 1);
    }

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.nodes(), 9);
    }

    #[test]
    fn test_filter_g() {
        let f = Formula::g(Interval { lower: 0, upper: 5 }, prop("a"));
        assert_eq!(f.count_nodes(|f| matches!(f, Formula::G { .. })), 1);
    }
}

mod branching_factor_tests {
    use super::*;

    #[test]
    fn test_flat_and_or() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), prop("b"), prop("c")]),
            Formula::or(vec![prop("d"), prop("e")]),
        ]);
        assert_eq!(f.branching_factor_avg(), 7.0 / 3.0);
    }

    #[test]
    fn test_flat_or_and() {
        let f = Formula::or(vec![
            Formula::And(vec![prop("a"), prop("b"), prop("c")]),
            Formula::And(vec![prop("d"), prop("e")]),
        ]);
        assert_eq!(f.branching_factor_avg(), 7.0 / 3.0);
    }

    #[test]
    fn test_single_prop() {
        let f = prop("a");
        assert_eq!(f.branching_factor_avg(), 0.0);
    }
}

mod boolean_variables_tests {
    use super::*;

    #[test]
    fn test_nested_no_rep() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.boolean_variables(), 4);
    }

    #[test]
    fn test_nested_with_rep() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("a")),
            ),
        ]);
        assert_eq!(f.boolean_variables(), 3);
    }
}

mod real_variables_tests {
    use super::*;

    #[test]
    fn test_nested_no_rep() {
        let f = Formula::And(vec![
            Formula::Prop(Expr::real(
                RelOp::Ge,
                AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
                AExpr::Num(Ratio::from_integer(1)),
            )),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                Formula::Prop(Expr::real(
                    RelOp::Lt,
                    AExpr::Var(Arc::from("y"), num_rational::Ratio::from_integer(1)),
                    AExpr::Num(Ratio::from_integer(2)),
                )),
                Formula::g(
                    Interval { lower: 1, upper: 3 },
                    Formula::Prop(Expr::real(
                        RelOp::Eq,
                        AExpr::Var(Arc::from("z"), num_rational::Ratio::from_integer(1)),
                        AExpr::Num(Ratio::from_integer(3)),
                    )),
                ),
            ),
        ]);
        assert_eq!(f.real_variables(), 3);
    }

    #[test]
    fn test_nested_rep() {
        let f = Formula::And(vec![
            Formula::Prop(Expr::real(
                RelOp::Ge,
                AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
                AExpr::Num(Ratio::from_integer(1)),
            )),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                Formula::Prop(Expr::real(
                    RelOp::Lt,
                    AExpr::Var(Arc::from("y"), num_rational::Ratio::from_integer(1)),
                    AExpr::Num(Ratio::from_integer(2)),
                )),
                Formula::g(
                    Interval { lower: 1, upper: 3 },
                    Formula::Prop(Expr::real(
                        RelOp::Eq,
                        AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
                        AExpr::Num(Ratio::from_integer(3)),
                    )),
                ),
            ),
        ]);
        assert_eq!(f.real_variables(), 2);
    }
}

mod boolean_constraints_tests {
    use super::*;

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![prop("a"), Formula::Not(Box::new(prop("b")))]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("c"),
                Formula::g(Interval { lower: 1, upper: 3 }, prop("d")),
            ),
        ]);
        assert_eq!(f.boolean_constraints(), 4);
    }
}

mod real_constraints_tests {
    use super::*;

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::Prop(Expr::real(
                RelOp::Ge,
                AExpr::Var(Arc::from("x"), num_rational::Ratio::from_integer(1)),
                AExpr::Num(Ratio::from_integer(1)),
            )),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                Formula::Prop(Expr::real(
                    RelOp::Lt,
                    AExpr::Var(Arc::from("y"), num_rational::Ratio::from_integer(1)),
                    AExpr::Num(Ratio::from_integer(2)),
                )),
                Formula::g(
                    Interval { lower: 1, upper: 3 },
                    Formula::Prop(Expr::real(
                        RelOp::Eq,
                        AExpr::Var(Arc::from("z"), num_rational::Ratio::from_integer(1)),
                        AExpr::Num(Ratio::from_integer(3)),
                    )),
                ),
            ),
        ]);
        assert_eq!(f.real_constraints(), 3);
    }
}

mod disjunction_max_width_tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_not_flat() {
        let f = Formula::And(vec![Formula::or(vec![
            prop("a"),
            Formula::or(vec![prop("b1"), prop("b2")]),
            prop("c"),
        ])]);
        let _ = f.disjunction_max_width();
    }

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![
                prop("a"),
                Formula::Not(Box::new(prop("b"))),
                prop("c"),
            ]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("d"),
                Formula::g(
                    Interval { lower: 1, upper: 3 },
                    Formula::or(vec![prop("e"), prop("f")]),
                ),
            ),
        ]);
        assert_eq!(f.disjunction_max_width(), 3);
    }
}

mod disjunction_total_width_tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_not_flat() {
        let f = Formula::And(vec![Formula::or(vec![
            prop("a"),
            Formula::or(vec![prop("b1"), prop("b2")]),
            prop("c"),
        ])]);
        let _ = f.disjunction_total_width();
    }

    #[test]
    fn test_nested() {
        let f = Formula::And(vec![
            Formula::or(vec![
                prop("a"),
                Formula::Not(Box::new(prop("b"))),
                prop("c"),
            ]),
            Formula::u(
                Interval { lower: 0, upper: 5 },
                prop("d"),
                Formula::g(
                    Interval { lower: 1, upper: 3 },
                    Formula::or(vec![prop("e"), prop("f")]),
                ),
            ),
        ]);
        assert_eq!(f.disjunction_total_width(), 5);
    }
}

mod combinatorial_branching_count_tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_not_flat() {
        let f = Formula::And(vec![Formula::or(vec![
            prop("a"),
            Formula::or(vec![prop("b1"), prop("b2")]),
            prop("c"),
        ])]);
        let _ = f.combinatorial_branching_count();
    }

    #[test]
    fn test_and_or() {
        let f = Formula::And(vec![
            Formula::or(vec![
                prop("a"),
                Formula::Not(Box::new(prop("b"))),
                prop("c"),
            ]),
            Formula::or(vec![prop("d"), prop("e")]),
        ]);
        assert_eq!(f.combinatorial_branching_count(), 6);
    }

    #[test]
    fn test_or_and() {
        let f = Formula::or(vec![
            Formula::And(vec![
                prop("a"),
                Formula::Not(Box::new(prop("b"))),
                prop("c"),
            ]),
            Formula::And(vec![prop("d"), prop("e")]),
        ]);
        assert_eq!(f.combinatorial_branching_count(), 2);
    }

    #[test]
    fn test_or_u() {
        let f = Formula::or(vec![
            Formula::u(Interval { lower: 0, upper: 5 }, prop("a"), prop("b")),
            Formula::u(Interval { lower: 1, upper: 3 }, prop("c"), prop("d")),
        ]);
        assert_eq!(f.combinatorial_branching_count(), 2);
    }
}
