use super::*;

mod constraint_graph {
    use super::*;

    #[test]
    fn test_strict_zero_no_cycle() {
        CycleTest::new()
            .add("x1", "x0", -1)
            .add_strict("x2", "x1", 0)
            .add("x0", "x1", 1)
            .add("x2", "dl0", Ratio::new(-8, 25))
            .add("dl0", "x2", Ratio::new(8, 25))
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_two_overlapping_negative_cycles() {
        CycleTest::new()
            // u -> v
            .add("u", "v", 0)
            // v -> a -> b -> u
            .add("v", "a", -2)
            .add("a", "b", -2)
            .add("b", "u", -2)
            // v -> x -> y -> u
            .add("v", "x", -1)
            .add("x", "y", -1)
            .add("y", "u", -1)
            .should_have_one_of_negative_cycles(vec![
                vec!["u", "v", "a", "b"],
                vec!["u", "v", "x", "y"],
            ]);
    }

    #[test]
    fn test_two_overlapping_cycles_one_positive() {
        CycleTest::new()
            // u -> v
            .add("u", "v", 0)
            // v -> a -> b -> u
            .add("v", "a", 1)
            .add("a", "b", 1)
            .add("b", "u", 1)
            // v -> x -> y -> u
            .add("v", "x", -1)
            .add("x", "y", -1)
            .add("y", "u", -1)
            .should_have_negative_cycle(vec!["u", "v", "x", "y"]);
    }

    #[test]
    fn test_disconnected_one_negative() {
        CycleTest::new()
            // x -> y -> z -> x
            .add("x", "y", 0)
            .add("y", "z", 0)
            .add_strict("z", "x", 0)
            // a -> b -> c -> a
            .add("a", "b", -1)
            .add("b", "c", 1)
            .add("c", "a", Ratio::new(1, 10))
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_disconnected_both_negative() {
        CycleTest::new()
            // x -> y -> z -> x
            .add("x", "y", 0)
            .add("y", "z", 0)
            .add_strict("z", "x", 0)
            // a -> b -> c -> a
            .add("a", "b", -1)
            .add("b", "c", -1)
            .add("c", "a", -1)
            .should_have_one_of_negative_cycles(vec![vec!["a", "b", "c"], vec!["x", "y", "z"]]);
    }

    #[test]
    fn test_disconnected_none_negative() {
        CycleTest::new()
            // x -> y -> z -> x
            .add("x", "y", 0)
            .add("y", "z", 0)
            .add("z", "x", 1)
            // a -> b -> c -> a
            .add("a", "b", 0)
            .add("b", "c", 0)
            .add("c", "a", 2)
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_all_zero_all_non_strict_is_not_negative() {
        CycleTest::new()
            .add("x", "y", 0)
            .add("y", "z", 0)
            .add("z", "x", 0)
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_all_zero_all_strict_is_negative() {
        CycleTest::new()
            .add_strict("x", "y", 0)
            .add_strict("y", "z", 0)
            .add_strict("z", "x", 0)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_sum_zero_non_strict_is_not_negative() {
        CycleTest::new()
            .add("x", "y", 5)
            .add("y", "z", -2)
            .add("z", "x", -3)
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_all_negative_is_negative() {
        CycleTest::new()
            .add("x", "y", -1)
            .add("y", "z", -2)
            .add("z", "x", -3)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_all_negative_strict_is_negative() {
        CycleTest::new()
            .add_strict("x", "y", -1)
            .add_strict("y", "z", -2)
            .add_strict("z", "x", -3)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_all_zero_some_strict_is_negative() {
        CycleTest::new()
            .add("x", "y", 0)
            .add("y", "z", 0)
            .add_strict("z", "x", 0)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_sum_zero_some_strict_is_negative() {
        CycleTest::new()
            .add("x", "y", 5)
            .add("y", "z", -2)
            .add_strict("z", "x", -3)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_negative_non_cycle() {
        CycleTest::new()
            .add("x1", "x2", -5)
            .add("x2", "x3", -3)
            .add("x3", "x4", -2)
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_cse3220_sat() {
        // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
        CycleTest::new()
            .add("x1", "x3", -5)
            .add("x1", "x4", -3)
            .add("x2", "x1", 3)
            .add("x3", "x2", 2)
            .add("x3", "x4", -1)
            .add("x4", "x2", 5)
            .should_not_have_negative_cycle();
    }

    #[test]
    fn test_cse3220_unsat() {
        // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
        CycleTest::new()
            .add("x1", "x3", -6)
            .add("x1", "x4", -3)
            .add("x2", "x1", 3)
            .add("x3", "x2", 2)
            .add("x3", "x4", -1)
            .add("x4", "x2", 5)
            .should_have_negative_cycle(vec!["x3", "x2", "x1"]);
    }

    #[test]
    fn test_duplicate_edge_one_positive() {
        CycleTest::new()
            .add("x", "y", 0)
            .add("x", "y", 2)
            .add("y", "z", -1)
            .add("z", "x", -1)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_duplicate_edge_both_negative() {
        CycleTest::new()
            .add("x", "y", -1)
            .add("x", "y", -2)
            .add("y", "z", 0)
            .add("z", "x", 0)
            .should_have_negative_cycle(vec!["x", "y", "z"]);
    }

    #[test]
    fn test_positive_strict() {
        CycleTest::new()
            .add_strict("x", "y", 1)
            .add_strict("y", "z", 0)
            .add_strict("z", "x", 0)
            .should_not_have_negative_cycle();
    }

    struct CycleTest {
        graph: ConstraintGraph,
    }

    impl CycleTest {
        fn new() -> Self {
            CycleTest {
                graph: ConstraintGraph::with_capacity(0),
            }
        }

        fn add(mut self, from: &str, to: &str, weight: impl Into<Ratio<i64>>) -> Self {
            self.graph
                .add_edge(from, to, EdgeWeight(weight.into(), 0), None);
            self
        }

        fn add_strict(mut self, from: &str, to: &str, weight: impl Into<Ratio<i64>>) -> Self {
            self.graph
                .add_edge(from, to, EdgeWeight(weight.into(), 1), None);
            self
        }

        fn should_have_negative_cycle(self, expected: Vec<&str>) {
            self.should_have_one_of_negative_cycles(vec![expected]);
        }

        fn should_have_one_of_negative_cycles(self, expecteds: Vec<Vec<&str>>) {
            let neg_cycle = self.graph.find_negative_cycle(true);
            let actual = match neg_cycle {
                NegativeCycleResult::CycleWithCore(cycle) => cycle,
                NegativeCycleResult::CycleWithoutCore => {
                    panic!("Expected negative cycle with core, but got without core")
                }
                NegativeCycleResult::NoCycle => {
                    panic!("Expected negative cycle, but none found")
                }
            };

            for expected in &expecteds {
                if self.cycle_equals_rotating(&actual, &expected) {
                    return;
                }
            }

            panic!(
                "Negative cycle {:?} does not match any of the expected cycles {:?} (even considering rotations)",
                actual, expecteds
            );
        }

        fn cycle_equals_rotating(&self, actual: &[usize], expected: &[&str]) -> bool {
            if actual.len() != expected.len() {
                return false;
            }

            let mut actual_vec: Vec<&str> = actual
                .iter()
                .map(|id| {
                    self.graph
                        .vertex_ids
                        .iter()
                        .find(|(_, v_id)| *v_id == id)
                        .map(|(name, _)| name.as_str())
                        .expect("Vertex id not found in graph")
                })
                .collect();

            for _ in 0..actual_vec.len() {
                if actual_vec == expected {
                    return true;
                }
                actual_vec.rotate_left(1);
            }

            false
        }

        fn should_not_have_negative_cycle(self) {
            let neg_cycle = self.graph.find_negative_cycle(false);
            assert!(
                matches!(neg_cycle, NegativeCycleResult::NoCycle),
                "Expected no negative cycle, but found {:?}",
                neg_cycle
            );
        }
    }
}

mod solver {
    use super::*;

    #[test]
    fn test_empty_solver() {
        // Build a solver which is not empty.
        let mut solver = DifferenceLogicSolver::new(true);
        solver.add_constraint(false, &RelOp::Le, &sub_expr("x", "y"), &num_expr(-1), 1);
        solver.add_constraint(false, &RelOp::Le, &sub_expr("y", "x"), &num_expr(-1), 2);
        solver.check();
        solver.push();

        // Check that the empty solver is indeed not empty.
        assert!(!solver.clause_set.is_empty());
        assert!(!solver.stack.is_empty());
        assert!(!solver.unsat_core.is_none());
        assert!(!solver.result_cache.is_empty());

        // Create an empty solver from it.
        let empty_solver = solver.empty_solver();

        // Check that the empty solver is indeed empty.
        assert!(empty_solver.unsat_core_extraction);
        assert!(empty_solver.clause_set.is_empty());
        assert!(empty_solver.stack.is_empty());
        assert!(empty_solver.unsat_core.is_none());
        // Cache is copied.
        assert!(!empty_solver.result_cache.is_empty());
    }

    #[test]
    fn test_empty_is_sat() {
        SolverTest::new(true)
            .should_be_sat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_simple_sat() {
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(0), 1)
            .should_be_sat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_simple_sat_with_binary_clause() {
        SolverTest::new(true)
            .add_constraint(RelOp::Ge, abs_sub_expr("x", "y"), num_expr(0), 1)
            .should_be_sat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_simple_unsat() {
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
            .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 2]));
    }

    #[test]
    fn test_simple_unsat_with_binary_clause() {
        SolverTest::new(true)
            .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(1), 1)
            .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(1), 2)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 2]));
    }

    #[test]
    #[should_panic]
    fn test_simple_unsat_with_binary_clause_no_core() {
        SolverTest::new(false)
            .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(1), 1)
            .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(1), 2)
            .should_be_unsat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_two_cycles_returns_correct_core() {
        // The graph for this problem contains two cycles which both contain edge u -> v.
        // One positive cycle: u -> v -> a -> u
        // One negative cycle: u -> v -> x -> u
        // The unsat core should contain the negative cycle.
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("u", "v"), num_expr(0), 1)
            .add_constraint(RelOp::Le, sub_expr("v", "a"), num_expr(1), 2)
            .add_constraint(RelOp::Le, sub_expr("a", "u"), num_expr(1), 3)
            .add_constraint(RelOp::Le, sub_expr("v", "x"), num_expr(-1), 4)
            .add_constraint(RelOp::Le, sub_expr("x", "u"), num_expr(-1), 5)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 4, 5]));
    }

    #[test]
    fn test_duplicate_edge_one_positive() {
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(0), 1)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(3), 2)
            .add_constraint(RelOp::Le, sub_expr("y", "z"), num_expr(-1), 3)
            .add_constraint(RelOp::Le, sub_expr("z", "x"), num_expr(-1), 4)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 3, 4]));
    }

    #[test]
    fn test_push_pop() {
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
            .should_be_sat()
            .should_have_unsat_core(None)
            .push()
            .should_be_sat()
            .should_have_unsat_core(None)
            .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 2]))
            .pop()
            .should_be_sat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_cse3220_unsat() {
        // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x1", "x3"), num_expr(-6), 1)
            .add_constraint(RelOp::Le, sub_expr("x1", "x4"), num_expr(-3), 2)
            .add_constraint(RelOp::Le, sub_expr("x2", "x1"), num_expr(3), 3)
            .add_constraint(RelOp::Le, sub_expr("x3", "x2"), num_expr(2), 4)
            .add_constraint(RelOp::Le, sub_expr("x3", "x4"), num_expr(-1), 5)
            .add_constraint(RelOp::Le, sub_expr("x4", "x2"), num_expr(5), 6)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 3, 4]));
    }

    #[test]
    fn test_cse3220_sat() {
        // From https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-smt/diff_solver.html
        SolverTest::new(true)
            .add_constraint(RelOp::Le, sub_expr("x1", "x3"), num_expr(-5), 1)
            .add_constraint(RelOp::Le, sub_expr("x1", "x4"), num_expr(-3), 2)
            .add_constraint(RelOp::Le, sub_expr("x2", "x1"), num_expr(3), 3)
            .add_constraint(RelOp::Le, sub_expr("x3", "x2"), num_expr(2), 4)
            .add_constraint(RelOp::Le, sub_expr("x3", "x4"), num_expr(-1), 5)
            .add_constraint(RelOp::Le, sub_expr("x4", "x2"), num_expr(5), 6)
            .should_be_sat()
            .should_have_unsat_core(None);
    }

    #[test]
    fn test_cached_unsat_core() {
        SolverTest::new(true)
            .push()
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
            .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![1, 2]))
            .pop()
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 3)
            .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 4)
            .should_be_unsat()
            .should_have_unsat_core(Some(vec![3, 4]));
    }

    #[test]
    #[should_panic]
    fn test_pop_on_empty_panics() {
        let mut solver = DifferenceLogicSolver::new(false);
        solver.pop();
    }

    #[test]
    #[should_panic]
    fn test_extract_unsat_core_panics_when_not_enabled() {
        SolverTest::new(false)
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(-1), 1)
            .add_constraint(RelOp::Le, sub_expr("y", "x"), num_expr(-1), 2)
            .should_be_unsat()
            .should_have_unsat_core(None);
    }

    struct SolverTest {
        solver: DifferenceLogicSolver,
    }

    impl SolverTest {
        fn new(unsat_core_extraction: bool) -> Self {
            SolverTest {
                solver: DifferenceLogicSolver::new(unsat_core_extraction),
            }
        }

        fn add_constraint(mut self, op: RelOp, left: AExpr, right: AExpr, id: usize) -> Self {
            self.solver.add_constraint(false, &op, &left, &right, id);
            self
        }

        fn should_be_sat(mut self) -> Self {
            assert!(
                self.solver.check(),
                "Expected DL constraints to be SAT, but they are UNSAT"
            );
            self
        }

        fn should_be_unsat(mut self) -> Self {
            assert!(
                !self.solver.check(),
                "Expected DL constraints to be UNSAT, but they are SAT"
            );
            self
        }

        fn should_have_unsat_core(self, expected: Option<Vec<usize>>) -> Self {
            let unsat_core = self.solver.extract_unsat_core();
            assert_eq!(
                unsat_core, expected,
                "Expected unsat core {:?}, but got {:?}",
                expected, unsat_core
            );
            self
        }

        fn push(mut self) -> Self {
            self.solver.push();
            self
        }

        fn pop(mut self) -> Self {
            self.solver.pop();
            self
        }
    }
}

mod solver_supports {
    use crate::formula::{Expr, Interval};
    use rstest::rstest;

    use super::*;

    #[test]
    fn supports_empty_formula() {
        assert_supported(true, vec![]);
    }

    #[rstest]
    #[case(Formula::prop(Expr::bool("x".into())))]
    #[case(Formula::prop(Expr::true_expr()))]
    #[case(Formula::prop(Expr::false_expr()))]
    #[case(Formula::and(vec![diff_constraint()]))]
    #[case(Formula::or(vec![diff_constraint()]))]
    #[case(Formula::not(diff_constraint()))]
    #[case(Formula::imply(diff_constraint(), diff_constraint()))]
    #[case(Formula::g(interval(), diff_constraint()))]
    #[case(Formula::f(interval(), diff_constraint()))]
    #[case(Formula::u(interval(), diff_constraint(), diff_constraint()))]
    #[case(Formula::r(interval(), diff_constraint(), diff_constraint()))]
    fn simple_supported(#[case] formula: Formula) {
        assert_supported(true, vec![formula]);
    }

    #[rstest]
    #[case(three_variables())]
    #[case(Formula::and(vec![three_variables()]))]
    #[case(Formula::or(vec![three_variables()]))]
    #[case(Formula::not(three_variables()))]
    #[case(Formula::imply(three_variables(), diff_constraint()))]
    #[case(Formula::imply(diff_constraint(), three_variables()))]
    #[case(Formula::g(interval(), three_variables()))]
    #[case(Formula::f(interval(), three_variables()))]
    #[case(Formula::u(interval(), three_variables(), diff_constraint()))]
    #[case(Formula::u(interval(), diff_constraint(), three_variables()))]
    #[case(Formula::r(interval(), three_variables(), diff_constraint()))]
    #[case(Formula::r(interval(), diff_constraint(), three_variables()))]
    fn simple_unsupported(#[case] formula: Formula) {
        assert_supported(false, vec![formula]);
    }

    fn assert_supported(supported: bool, formulas: Vec<Formula>) {
        assert_eq!(
            supported,
            DifferenceLogicSolver::supports(&Node::from_operands(
                formulas.into_iter().map(|f| f.into()).collect(),
            ))
        );
    }

    fn diff_constraint() -> Formula {
        Formula::prop(Expr::real(RelOp::Le, sub_expr("x", "y"), num_expr(1)))
    }

    fn three_variables() -> Formula {
        Formula::prop(Expr::real(RelOp::Le, sub_expr("x", "y"), var_expr("z")))
    }

    fn interval() -> Interval {
        Interval {
            lower: 0,
            upper: 42,
        }
    }
}

mod clause_set {
    use super::*;

    #[test]
    fn empty_clause_set_is_empty() {
        ClauseSetTest::new("x <= y").should_be_empty();
    }

    #[test]
    fn non_empty_clause_set_is_not_empty() {
        ClauseSetTest::new("x <= y")
            .add_constraint(RelOp::Le, var_expr("x"), var_expr("y"), 1)
            .should_not_be_empty();
    }

    #[test]
    fn var_eq_var() {
        ClauseSetTest::new("(x = y)  ==>  (x - y <= 0 && y - x <= 0)")
            .add_constraint(RelOp::Eq, var_expr("x"), var_expr("y"), 1)
            .should_contain_unary_constraint("x", "y", 0, false, 1)
            .should_contain_unary_constraint("y", "x", 0, false, 1)
            .should_have_clause_count(2);
    }

    #[test]
    fn var_eq_num() {
        ClauseSetTest::new("(x = 5)  ==>  (x - 0 <= 5 && 0 - x <= -5)")
            .add_constraint(RelOp::Eq, var_expr("x"), num_expr(5), 1)
            .should_contain_unary_constraint("x", "__dl_zero", 5, false, 1)
            .should_contain_unary_constraint("__dl_zero", "x", -5, false, 1)
            .should_have_clause_count(2);
    }

    #[test]
    fn var_ne_num() {
        ClauseSetTest::new(
            "(x != 3)  ==>  (x < 3 || x > 3)  ==>  (x - 0 <= 3 - ε || 0 - x <= -3 - ε)",
        )
        .add_constraint(RelOp::Ne, var_expr("x"), num_expr(3), 1)
        .should_contain_binary_constraint(
            ("x", "__dl_zero", 3, true, 1),
            ("__dl_zero", "x", -3, true, 1),
        )
        .should_have_clause_count(1);
    }

    #[test]
    fn var_ne_var() {
        ClauseSetTest::new(
            "(x != y)  ==>  (x < y || x > y)  ==>  (x - y <= 0 - ε || y - x <= 0 - ε)",
        )
        .add_constraint(RelOp::Ne, var_expr("x"), var_expr("y"), 1)
        .should_contain_binary_constraint(("x", "y", 0, true, 1), ("y", "x", 0, true, 1))
        .should_have_clause_count(1);
    }

    #[test]
    fn abs_le_num() {
        ClauseSetTest::new("(|x| <= 2)  ==>  (x <= 2 && x >= -2)  ==>  (x - 0 <= 2 && 0 - x <= 2)")
            .add_constraint(RelOp::Le, abs_expr("x"), num_expr(2), 1)
            .should_contain_unary_constraint("x", "__dl_zero", 2, false, 1)
            .should_contain_unary_constraint("__dl_zero", "x", 2, false, 1)
            .should_have_clause_count(2);
    }

    #[test]
    fn abs_lt_num() {
        ClauseSetTest::new(
            "(|x| < 4)  ==>  (x < 4 && x > -4)  ==>  (x - 0 <= 4 - ε && 0 - x <= 4 - ε)",
        )
        .add_constraint(RelOp::Lt, abs_expr("x"), num_expr(4), 1)
        .should_contain_unary_constraint("x", "__dl_zero", 4, true, 1)
        .should_contain_unary_constraint("__dl_zero", "x", 4, true, 1)
        .should_have_clause_count(2);
    }

    #[test]
    fn abs_gt_num() {
        ClauseSetTest::new(
            "(|x| > 3)  ==>  (x > 3 || x < -3)  ==>  (0 - x <= -3 - ε || x - 0 <= -3 - ε)",
        )
        .add_constraint(RelOp::Gt, abs_expr("x"), num_expr(3), 1)
        .should_contain_binary_constraint(
            ("__dl_zero", "x", -3, true, 1),
            ("x", "__dl_zero", -3, true, 1),
        )
        .should_have_clause_count(1);
    }

    #[test]
    fn abs_ge_num() {
        ClauseSetTest::new(
            "(|x| >= 5)  ==>  (x >= 5 || x <= -5)  ==>  (0 - x <= -5 || x - 0 <= -5)",
        )
        .add_constraint(RelOp::Ge, abs_expr("x"), num_expr(5), 1)
        .should_contain_binary_constraint(
            ("__dl_zero", "x", -5, false, 1),
            ("x", "__dl_zero", -5, false, 1),
        )
        .should_have_clause_count(1);
    }

    #[test]
    fn var_le_num() {
        ClauseSetTest::new("(x <= 10)  ==>  (x - 0 <= 10)")
            .add_constraint(RelOp::Le, var_expr("x"), num_expr(10), 1)
            .should_contain_unary_constraint("x", "__dl_zero", 10, false, 1)
            .should_have_clause_count(1);
    }

    #[test]
    #[should_panic]
    fn abs_ne_num() {
        ClauseSetTest::new("|x| != 9  ==> panic").add_constraint(
            RelOp::Ne,
            abs_expr("x"),
            num_expr(9),
            1,
        );
    }

    #[test]
    #[should_panic]
    fn abs_lt_var() {
        ClauseSetTest::new("|x| < y  ==> panic").add_constraint(
            RelOp::Lt,
            abs_expr("x"),
            var_expr("y"),
            1,
        );
    }

    #[test]
    #[should_panic]
    fn abs_le_var() {
        ClauseSetTest::new("|x| <= y  ==> panic").add_constraint(
            RelOp::Le,
            abs_expr("x"),
            var_expr("y"),
            1,
        );
    }

    #[test]
    #[should_panic]
    fn abs_gt_var() {
        ClauseSetTest::new("|x| > y  ==> panic").add_constraint(
            RelOp::Gt,
            abs_expr("x"),
            var_expr("y"),
            1,
        );
    }

    #[test]
    #[should_panic]
    fn abs_ge_var() {
        ClauseSetTest::new("|x| >= y  ==> panic").add_constraint(
            RelOp::Ge,
            abs_expr("x"),
            var_expr("y"),
            1,
        );
    }

    #[test]
    fn sub_le_num() {
        ClauseSetTest::new("(x - y <= 7)  ==>  (x - y <= 7)")
            .add_constraint(RelOp::Le, sub_expr("x", "y"), num_expr(7), 1)
            .should_contain_unary_constraint("x", "y", 7, false, 1)
            .should_have_clause_count(1);
    }

    #[test]
    fn abs_sub_le_num() {
        ClauseSetTest::new("(|x - y| <= 4)  ==>  (x - y <= 4 && y - x <= 4)")
            .add_constraint(RelOp::Le, abs_sub_expr("x", "y"), num_expr(4), 1)
            .should_contain_unary_constraint("x", "y", 4, false, 1)
            .should_contain_unary_constraint("y", "x", 4, false, 1)
            .should_have_clause_count(2);
    }

    #[test]
    fn abs_sub_gt_num() {
        ClauseSetTest::new("(|x - y| > 12)  ==>  (x - y > 12 || x - y < -12)  ==>  (y - x <= -12 - ε || x - y <= -12 - ε)")
                .add_constraint(RelOp::Gt, abs_sub_expr("x", "y"), num_expr(12), 1)
                .should_contain_binary_constraint(
                    ("y", "x", -12, true, 1),
                    ("x", "y", -12, true, 1,)
                )
                .should_have_clause_count(1);
    }

    #[test]
    fn sub_ne_num() {
        ClauseSetTest::new("(x - y != 18)  ==>  (x - y < 18 || x - y > 18)  ==>  (x - y <= 18 - ε || y - x <= -18 - ε)")
                .add_constraint(RelOp::Ne, sub_expr("x", "y"), num_expr(18), 1)
                .should_contain_binary_constraint(
                    ("x", "y", 18, true, 1),
                    ("y", "x", -18, true, 1),
                )
                .should_have_clause_count(1);
    }

    #[test]
    fn complex_constraint_mix() {
        ClauseSetTest::new("Complex mix of constraints")
            .add_constraint(RelOp::Eq, var_expr("x"), num_expr(2), 1)
            .add_constraint(RelOp::Ne, var_expr("y"), num_expr(4), 2)
            .add_constraint(RelOp::Lt, abs_expr("z"), num_expr(8), 3)
            .should_contain_unary_constraint("x", "__dl_zero", 2, false, 1)
            .should_contain_unary_constraint("__dl_zero", "x", -2, false, 1)
            .should_contain_binary_constraint(
                ("y", "__dl_zero", 4, true, 2),
                ("__dl_zero", "y", -4, true, 2),
            )
            .should_contain_unary_constraint("z", "__dl_zero", 8, true, 3)
            .should_have_clause_count(5);
    }

    #[test]
    fn same_constraint() {
        ClauseSetTest::new("Same constraint is stored only once")
            .add_constraint(RelOp::Le, sub_expr("a", "b"), num_expr(10), 1)
            .add_constraint(RelOp::Le, sub_expr("a", "b"), num_expr(10), 1)
            .should_contain_unary_constraint("a", "b", 10, false, 1)
            .should_have_clause_count(1);
    }

    struct ClauseSetTest {
        description: String,
        clause_set: ClauseSet,
    }

    impl ClauseSetTest {
        fn new(description: &str) -> Self {
            Self {
                description: description.to_string(),
                clause_set: ClauseSet::new(),
            }
        }

        fn add_constraint(mut self, op: RelOp, left: AExpr, right: AExpr, id: usize) -> Self {
            let constraint = NormalizedConstraint::new(false, &op, &left, &right, Some(id));
            self.clause_set.add_constraint(&constraint).unwrap();
            self
        }

        fn should_be_empty(self) -> Self {
            assert!(
                self.clause_set.is_empty(),
                "Expected clause set '{}' to be empty, but it was {:?}",
                self.description,
                self.clause_set.len()
            );
            self
        }

        fn should_not_be_empty(self) -> Self {
            assert!(
                !self.clause_set.is_empty(),
                "Expected clause set '{}' to be non-empty, but it was empty",
                self.description
            );
            self
        }

        fn should_contain_unary_constraint(
            self,
            x: &str,
            y: &str,
            c: i64,
            strict: bool,
            id: usize,
        ) -> Self {
            let expected = DifferenceConstraint {
                x: x.into(),
                y: y.into(),
                c: Ratio::from_integer(c),
                strict: strict,
                id: Some(id),
            };

            let found = self
                .clause_set
                .unary
                .iter()
                .any(|clause| clause.0 == expected);

            assert!(
                found,
                "Expected unary constraint ({}, {} - {} <= {}{}) in '{}', but it was not found.\nActual clauses: {:?}",
                id,
                x,
                y,
                c,
                if strict { " - ε" } else { "" },
                self.description,
                self.clause_set.unary
            );
            self
        }

        fn should_contain_binary_constraint(
            self,
            expected1: (&str, &str, i64, bool, usize),
            expected2: (&str, &str, i64, bool, usize),
        ) -> Self {
            let constraint1 = DifferenceConstraint {
                x: expected1.0.into(),
                y: expected1.1.into(),
                c: Ratio::from_integer(expected1.2),
                strict: expected1.3,
                id: Some(expected1.4),
            };

            let constraint2 = DifferenceConstraint {
                x: expected2.0.into(),
                y: expected2.1.into(),
                c: Ratio::from_integer(expected2.2),
                strict: expected2.3,
                id: Some(expected2.4),
            };

            let found = self
                .clause_set
                .binary
                .iter()
                .any(|clause| clause.0 == constraint1 && clause.1 == constraint2);

            assert!(
                found,
                "Expected binary clause with constraints ({}, {} - {} <= {}{}) and ({}, {} - {} <= {}{}) in '{}', but it was not found.\nActual clauses: {:?}",
                expected1.4,
                expected1.0,
                expected1.1,
                expected1.2,
                if expected1.3 { " - ε" } else { "" },
                expected2.4,
                expected2.0,
                expected2.1,
                expected2.2,
                if expected2.3 { " - ε" } else { "" },
                self.description,
                self.clause_set.binary
            );
            self
        }

        fn should_have_clause_count(self, expected_count: usize) -> Self {
            let actual = self.clause_set.len();
            assert_eq!(
                actual,
                expected_count,
                "Expected exactly {} clauses in '{}', got {}\nActual clauses: {:?} and {:?}",
                expected_count,
                self.description,
                actual,
                self.clause_set.unary,
                self.clause_set.binary
            );
            self
        }
    }
}

mod constraint {
    use super::*;

    #[test]
    fn sub_le_num() {
        ConstraintTest::new("(x - y <= 5)  ==>  (x - y <= 5)")
            .input(sub_expr("x", "y"), RelOp::Le, num_expr(5), 1)
            .should_become("x", "y", 5, false);
    }

    #[test]
    fn sub_lt_num() {
        ConstraintTest::new("(x - y < 4)  ==>  (x - y <= 4 - ε)")
            .input(sub_expr("x", "y"), RelOp::Lt, num_expr(4), 2)
            .should_become("x", "y", 4, true);
    }

    #[test]
    fn sub_ge_num() {
        ConstraintTest::new("(x - y >= -3)  ==>  (y - x <= 3)")
            .input(sub_expr("x", "y"), RelOp::Ge, num_expr(-3), 3)
            .should_become("y", "x", 3, false);
    }

    #[test]
    fn sub_gt_num() {
        ConstraintTest::new("(x - y > -2)  ==>  (y - x <= 2 - ε)")
            .input(sub_expr("x", "y"), RelOp::Gt, num_expr(-2), 4)
            .should_become("y", "x", 2, true);
    }

    #[test]
    fn num_le_sub() {
        ConstraintTest::new("(5 <= x - y)  ==>  (y - x <= -5)")
            .input(num_expr(5), RelOp::Le, sub_expr("x", "y"), 5)
            .should_become("y", "x", -5, false);
    }

    #[test]
    fn num_lt_sub() {
        ConstraintTest::new("(4 < x - y)  ==>  (y - x <= -4 - ε)")
            .input(num_expr(4), RelOp::Lt, sub_expr("x", "y"), 6)
            .should_become("y", "x", -4, true);
    }

    #[test]
    fn num_ge_sub() {
        ConstraintTest::new("(-3 >= x - y)  ==>  (x - y <= -3)")
            .input(num_expr(-3), RelOp::Ge, sub_expr("x", "y"), 7)
            .should_become("x", "y", -3, false);
    }

    #[test]
    fn num_gt_sub() {
        ConstraintTest::new("(-2 > x - y)  ==>  (x - y <= -2 - ε)")
            .input(num_expr(-2), RelOp::Gt, sub_expr("x", "y"), 8)
            .should_become("x", "y", -2, true);
    }

    #[test]
    fn var_le_num() {
        ConstraintTest::new("(x <= 5)  ==>  (x - 0 <= 5)")
            .input(var_expr("x"), RelOp::Le, num_expr(5), 9)
            .should_become("x", "__dl_zero", 5, false);
    }

    #[test]
    fn var_lt_num() {
        ConstraintTest::new("(x < 5)  ==>  (x - 0 <= 5 - ε)")
            .input(var_expr("x"), RelOp::Lt, num_expr(5), 10)
            .should_become("x", "__dl_zero", 5, true);
    }

    #[test]
    fn var_ge_num() {
        ConstraintTest::new("(x >= -5)  ==>  (0 - x <= 5)")
            .input(var_expr("x"), RelOp::Ge, num_expr(-5), 11)
            .should_become("__dl_zero", "x", 5, false);
    }

    #[test]
    fn var_gt_num() {
        ConstraintTest::new("(x > -5)  ==>  (0 - x <= 5 - ε)")
            .input(var_expr("x"), RelOp::Gt, num_expr(-5), 12)
            .should_become("__dl_zero", "x", 5, true);
    }

    #[test]
    fn num_le_var() {
        ConstraintTest::new("(5 <= x)  ==>  (0 - x <= -5)")
            .input(num_expr(5), RelOp::Le, var_expr("x"), 13)
            .should_become("__dl_zero", "x", -5, false);
    }

    #[test]
    fn num_lt_var() {
        ConstraintTest::new("(5 < x)  ==>  (0 - x <= -5 - ε)")
            .input(num_expr(5), RelOp::Lt, var_expr("x"), 14)
            .should_become("__dl_zero", "x", -5, true);
    }

    #[test]
    fn num_ge_var() {
        ConstraintTest::new("(5 >= x)  ==>  (x - 0 <= 5)")
            .input(num_expr(5), RelOp::Ge, var_expr("x"), 15)
            .should_become("x", "__dl_zero", 5, false);
    }

    #[test]
    fn num_gt_var() {
        ConstraintTest::new("(-4 > x)  ==>  (x - 0 <= -4 - ε)")
            .input(num_expr(-4), RelOp::Gt, var_expr("x"), 16)
            .should_become("x", "__dl_zero", -4, true);
    }

    #[test]
    fn var_le_var() {
        ConstraintTest::new("(x <= y)  ==>  (x - y <= 0)")
            .input(var_expr("x"), RelOp::Le, var_expr("y"), 17)
            .should_become("x", "y", 0, false);
    }

    #[test]
    fn var_lt_var() {
        ConstraintTest::new("(x < y)  ==>  (x - y <= 0 - ε)")
            .input(var_expr("x"), RelOp::Lt, var_expr("y"), 18)
            .should_become("x", "y", 0, true);
    }

    #[test]
    fn var_ge_var() {
        ConstraintTest::new("(x >= y)  ==>  (y - x <= 0)")
            .input(var_expr("x"), RelOp::Ge, var_expr("y"), 19)
            .should_become("y", "x", 0, false);
    }

    #[test]
    fn var_gt_var() {
        ConstraintTest::new("(x > y)  ==>  (y - x <= 0 - ε)")
            .input(var_expr("x"), RelOp::Gt, var_expr("y"), 20)
            .should_become("y", "x", 0, true);
    }

    #[test]
    fn invalid_sub_expression() {
        assert_eq!(
            NormalizedConstraint::new(
                false,
                &RelOp::Le,
                &AExpr::BinOp {
                    op: ArithOp::Sub,
                    left: var_expr("x").into(),
                    right: num_expr(1).into(),
                },
                &num_expr(5),
                None
            )
            .to_diff(),
            Err(DifferenceConstraintError::InvalidSubExpression)
        );
    }

    #[test]
    fn invalid_expression() {
        assert_eq!(
            NormalizedConstraint::new(
                false,
                &RelOp::Le,
                &AExpr::BinOp {
                    op: ArithOp::Add, // Note this is addition, not subtraction
                    left: var_expr("x").into(),
                    right: num_expr(1).into(),
                },
                &num_expr(5),
                None
            )
            .to_diff(),
            Err(DifferenceConstraintError::InvalidExpression)
        );
    }

    #[test]
    fn invalid_relation() {
        assert_eq!(
            NormalizedConstraint::new(false, &RelOp::Ne, &var_expr("x"), &var_expr("y"), None)
                .to_diff(),
            Err(DifferenceConstraintError::InvalidRelation)
        );
    }

    struct ConstraintTest {
        description: String,
        input: Option<DifferenceConstraint>,
        id: Option<usize>,
    }

    impl ConstraintTest {
        fn new(description: &str) -> Self {
            Self {
                description: description.to_string(),
                input: None,
                id: None,
            }
        }

        fn input(&self, left: AExpr, op: RelOp, right: AExpr, id: usize) -> Self {
            Self {
                description: self.description.clone(),
                input: Some(
                    NormalizedConstraint::new(false, &op, &left, &right, Some(id))
                        .to_diff()
                        .unwrap(),
                ),
                id: Some(id),
            }
        }

        fn should_become(&self, x: &str, y: &str, c: i64, strict: bool) {
            let constraint = self.input.clone().unwrap();

            assert_eq!(
                constraint.x,
                x.into(),
                "Wrong x in test case \"{}\"",
                self.description
            );
            assert_eq!(
                constraint.y,
                y.into(),
                "Wrong y in test case \"{}\"",
                self.description
            );
            assert_eq!(
                constraint.c,
                Ratio::from_integer(c),
                "Wrong c in test case \"{}\"",
                self.description
            );
            assert_eq!(
                constraint.strict, strict,
                "Wrong strict in test case \"{}\"",
                self.description
            );
            assert_eq!(
                constraint.id, self.id,
                "Wrong id in test case \"{}\"",
                self.description
            );
        }
    }
}

mod normalized_constraint {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_already_normalized() {
        let constraint =
            NormalizedConstraint::new(false, &RelOp::Le, &var_expr("x"), &num_expr(5), Some(1));

        assert_eq!(constraint.op, RelOp::Le);
        assert_eq!(constraint.left, var_expr("x"));
        assert_eq!(constraint.right, num_expr(5));
        assert_eq!(constraint.id, Some(1));
    }

    #[rstest]
    #[case(RelOp::Le, RelOp::Gt)]
    #[case(RelOp::Lt, RelOp::Ge)]
    #[case(RelOp::Ge, RelOp::Lt)]
    #[case(RelOp::Gt, RelOp::Le)]
    #[case(RelOp::Eq, RelOp::Ne)]
    #[case(RelOp::Ne, RelOp::Eq)]
    fn test_negation(#[case] input_op: RelOp, #[case] expected_op: RelOp) {
        let constraint =
            NormalizedConstraint::new(true, &input_op, &var_expr("x"), &num_expr(5), Some(1));

        assert_eq!(constraint.op, expected_op);
        assert_eq!(constraint.left, var_expr("x"));
        assert_eq!(constraint.right, num_expr(5));
        assert_eq!(constraint.id, Some(1));
    }

    #[rstest]
    #[case(RelOp::Le, RelOp::Ge)]
    #[case(RelOp::Lt, RelOp::Gt)]
    #[case(RelOp::Ge, RelOp::Le)]
    #[case(RelOp::Gt, RelOp::Lt)]
    #[case(RelOp::Eq, RelOp::Eq)]
    #[case(RelOp::Ne, RelOp::Ne)]
    fn test_normalization(#[case] input_op: RelOp, #[case] expected_op: RelOp) {
        let constraint =
            NormalizedConstraint::new(false, &input_op, &num_expr(5), &var_expr("x"), Some(1));

        assert_eq!(constraint.op, expected_op);
        assert_eq!(constraint.left, var_expr("x"));
        assert_eq!(constraint.right, num_expr(5));
        assert_eq!(constraint.id, Some(1));
    }
}

fn sub_expr(x: &str, y: &str) -> AExpr {
    AExpr::BinOp {
        op: ArithOp::Sub,
        left: Box::new(AExpr::Var(x.into())),
        right: Box::new(AExpr::Var(y.into())),
    }
}

fn abs_expr(x: &str) -> AExpr {
    AExpr::Abs(Box::new(AExpr::Var(x.into())))
}

fn abs_sub_expr(x: &str, y: &str) -> AExpr {
    AExpr::Abs(Box::new(AExpr::BinOp {
        op: ArithOp::Sub,
        left: Box::new(AExpr::Var(x.into())),
        right: Box::new(AExpr::Var(y.into())),
    }))
}

fn num_expr(n: i64) -> AExpr {
    AExpr::Num(Ratio::from_integer(n))
}

fn var_expr(x: &str) -> AExpr {
    AExpr::Var(x.into())
}
