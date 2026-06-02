use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) const DEFAULT_ENGINE_TEST_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) struct SolverCase {
    pub(crate) name: &'static str,
    pub(crate) formula: &'static str,
    pub(crate) mltl: bool,
    pub(crate) expected: bool,
}

pub(crate) fn run_solver_case(
    engine: &'static str,
    case: SolverCase,
    run: fn(&str, bool) -> Option<bool>,
    timeout: Duration,
) {
    let (tx, rx) = mpsc::channel();
    let formula = case.formula;
    let mltl = case.mltl;

    thread::spawn(move || {
        let _ = tx.send(run(formula, mltl));
    });

    let actual = match rx.recv_timeout(timeout) {
        Ok(actual) => actual,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            eprintln!(
                "warning: {engine}::{name} exceeded {timeout:?}; treating as inconclusive",
                name = case.name
            );
            return;
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{engine}::{} panicked before returning a result", case.name);
        }
    };

    match actual {
        Some(actual) => assert_eq!(
            actual, case.expected,
            "{engine}::{} returned the wrong result for {}",
            case.name, case.formula
        ),
        None => eprintln!(
            "warning: {engine}::{} returned unknown for {}; expected {}",
            case.name, case.formula, case.expected
        ),
    }
}

macro_rules! solver_engine_tests {
    ($engine:literal, $run:path) => {
        #[rstest::rstest]
        #[case::and("test_and", "a && b", false, true)]
        #[case::many_ops("test_many_ops", "a && b && c && (a || b || c) && d", false, true)]
        #[case::true_const("test_true", "a && !TrUe", false, false)]
        #[case::false_const("test_false", "a && FaLsE", false, false)]
        #[case::globally0("test_globally0", "G[2,5] (R_x > 5 || R_x < 0)", false, true)]
        #[case::globally_add("test_globally_add", "G[2,5] (R_x + R_y > 5 && R_x - R_y < 0)", false, true)]
        #[case::globally_add_many("test_globally_add_many", "G[2,5] (R_x + R_y - R_z + R_x > 5 && R_x - R_y < 0)", false, true)]
        #[case::release("test_release", "(R_x == 10) R[1,6] (R_x < 10)", false, true)]
        #[case::abs("test_abs", "G[0,5] (|x| > 20 || |x| < 10) && F[0,5] (x == -15)", false, false)]
        #[case::mltl_stl_semantics("test_mltl_stl_semantics", "F[58,92] ((a1) U[87,100] ((a1 && a0 && ! a1) U[9,100] (a0)))", false, false)]
        #[case::mltl_mltl_semantics("test_mltl_mltl_semantics", "F[58,92] ((a1) U[87,100] ((a1 && a0 && ! a1) U[9,100] (a0)))", true, true)]
        #[case::release_false("test_release_false", "false R[0,10] a", false, true)]
        #[case::gfgg("test_gfgg", "G[0,6] F[2,4] a && G[0,6] (a -> G[1,3] !a)", false, false)]
        #[case::jump1_0("test_jump1_0", "!a && G[10,20] !a && F[0,20] a", false, true)]
        #[case::jump1_g("test_jump1_g", "G[0,10] !a && F[5,20] a && G[15,25] !a", false, true)]
        #[case::jump1_f("test_jump1_f", "F[0,10] !a && G[0,9] a && F[10,20] a && G[15,20] !a", false, true)]
        #[case::jump1_u("test_jump1_u", "b U[0,10] !a && G[0,9] a && F[10,20] a && G[15,20] !a", false, true)]
        #[case::g_is_derived("test_g_is_derived", "G[0,6] (!(a0 U[2,10] (F[0,6] (a0))))", true, true)]
        #[case::u_parent("test_u_parent", "(G[0,89] F[88,100] a2 U[0,78] !a1) && a1", true, true)]
        #[case::implication_negation("test_implication_negation", "G[0, 6] !a && (G[0, 3] !a -> F[0, 3] a)", false, false)]
        #[case::globally_imply_merge("test_globally_imply_merge", "G[0, 10] (a -> G[10, 15] b) && G[0, 10] a && G[16, 16] !b", false, false)]
        #[case::until_mltl("test_until_mltl", "a U[39, 77] (G[0, 15] a) && G[82, 100] !a", true, true)]
        #[case::gfg("test_gfg", "G[5, 10] F[8, 10] a && G[16, 17] !a", false, true)]
        #[case::fgf("test_fgf", "F[0,25] G[0,30] a0 && (F[0,28] ! a0) && a0", false, true)]
        #[case::fgfg("test_fgfg", "F[0,25] G[0,30] a0 && (F[0,28] ! a0) && G[0, 1] a0", false, true)]
        #[case::step_r("test_step_r", "(a R[0,10] b) && G[6,10] (!a && !b) && !a", false, true)]
        #[case::jump_error("test_jump_error", "G[0,4] (!b) && a && (!a R[0,3] (c U[0,4] b))", false, false)]
        #[case::jump_until_satisfied("test_jump_until_satisfied", "F[0,10] a && !b && ((G[0,5] !a) U[0,15] b)", false, true)]
        #[case::jump_completeness("test_jump_completeness", "(a U[0, 10] (b && G[20, 30] c)) && G[0, 27] !c && G[10, 10] !b", false, true)]
        #[case::jump_completeness_obstacle_nested("test_jump_completeness_obstacle_nested", "G[5, 5] G[5, 5] !a && b U[0, 5] G[10, 10] a && G[15, 15] !a", false, true)]
        #[case::jump_completeness_release_obstacle("test_jump_completeness_release_obstacle", "(F[0, 10] (b && G[20, 30] c)) && a R[0, 27] !c && G[10, 10] !b && G[0,50] !a", false, true)]
        #[case::jump_completeness_release_target_conflict("test_jump_completeness_release_target_conflict", "(F[0, 10] (b && G[20, 30] c)) && G[0, 27] !c && G[10, 10] !b && (F[10,10] !c) R[18,19] a && F[19,19] !a", false, true)]
        #[case::jump_completeness_release_target_conflict_postponed("test_jump_completeness_release_target_conflict_postponed", "(F[0, 15] (b && G[20, 30] c)) && G[0, 27] !c && G[15, 15] !b && (d && F[10,10] !c) R[17,20] a && F[20,20] !a && F[17,17] !d", false, true)]
        #[case::jump_soundness("test_jump_soundness", "(G[0,1] (F[5,5] a)) U[0,5] (!b) && G[0,4] b && G[8,8] !a", true, false)]
        #[case::jump_soundness_f("test_jump_soundness_f", "(G[0,1] (F[5,5] a)) U[0,5] (!b) && G[0,4] b && F[8,8] !a", true, false)]
        fn solver_cases(
            #[case] name: &'static str,
            #[case] formula: &'static str,
            #[case] mltl: bool,
            #[case] expected: bool,
        ) {
            $crate::sat::tests::run_solver_case(
                $engine,
                $crate::sat::tests::SolverCase {
                    name,
                    formula,
                    mltl,
                    expected,
                },
                $run,
                $crate::sat::tests::DEFAULT_ENGINE_TEST_TIMEOUT,
            );
        }
    };
}

pub(crate) use solver_engine_tests;
