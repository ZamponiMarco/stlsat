use crate::sat::{config::GeneralOptions, smt::SmtSolver, tests::solver_engine_tests};

fn make_test(formula_str: &str, mltl: bool) -> Option<bool> {
    let general = GeneralOptions {
        mltl,
        ..Default::default()
    };
    let mut smt_solver = SmtSolver::new(general);
    smt_solver.make_smt_from_str(formula_str)
}

solver_engine_tests!("smt", make_test);
