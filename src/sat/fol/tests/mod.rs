use crate::sat::{config::GeneralOptions, fol::FolSolver, tests::solver_engine_tests};

fn make_test(formula_str: &str, mltl: bool) -> Option<bool> {
    let general = GeneralOptions {
        mltl,
        ..Default::default()
    };
    let mut fol_solver = FolSolver::new(general);
    fol_solver.make_fol_from_str(formula_str)
}

solver_engine_tests!("fol", make_test);
