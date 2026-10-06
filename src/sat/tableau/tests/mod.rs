use crate::sat::{
    config::{GeneralOptions, TableauOptions},
    tableau::Tableau,
    tests::solver_engine_tests,
};

fn make_test(formula_str: &str, mltl: bool) -> Option<bool> {
    let general = GeneralOptions {
        mltl,
        ..Default::default()
    };
    let tableau = TableauOptions {
        formula_optimizations: false,
        formula_simplifications: false,
        memoization: false,
        simple_first: false,
        ..Default::default()
    };
    let mut tableau_solver = Tableau::new(general, tableau);
    tableau_solver.make_tableau_from_str(formula_str)
}

solver_engine_tests!("tableau", make_test);

#[rstest::rstest]
#[case("a", "a")]
#[case("a || b", "a")]
fn test_terminal_sat_trace(#[case] formula: &str, #[case] expected: &str) {
    let options = TableauOptions {
        trace_extraction: true,
        ..Default::default()
    };
    let mut tableau = Tableau::new(GeneralOptions::default(), options);
    assert_eq!(tableau.make_tableau_from_str(formula), Some(true));
    let trace = tableau.trace.expect("SAT should produce a trace");
    assert_eq!(trace.length(), 0);
    let states = trace.full_trace();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].len(), 1);
    assert_eq!(states[0][0].to_string(), expected);
}

#[test]
fn test_depth_reached() {
    let general = GeneralOptions::default();
    let options = TableauOptions {
        max_depth: 10,
        ..Default::default()
    };
    let mut tableau = Tableau::new(general, options);
    assert_eq!(
        tableau.make_tableau_from_str("(G[0,1000] F[0, 100] a) || (a && !a)"),
        None
    );
}
