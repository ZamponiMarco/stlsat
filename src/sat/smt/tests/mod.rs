use super::*;

fn make_test(formula_str: &str, mltl: bool) -> Option<bool> {
    let general = GeneralOptions {
        mltl,
        ..Default::default()
    };
    let mut smt_solver = SmtSolver::new(general);
    smt_solver.make_smt_from_str(formula_str)
}

#[test]
fn test_and() {
    assert_eq!(make_test("a && b", false), Some(true));
}

#[test]
fn test_many_ops() {
    assert_eq!(
        make_test("a && b && c && (a || b || c) && d", false),
        Some(true)
    );
}

#[test]
fn test_true() {
    assert_eq!(make_test("a && !TrUe", false), Some(false));
}

#[test]
fn test_false() {
    assert_eq!(make_test("a && FaLsE", false), Some(false));
}

#[test]
fn test_globally0() {
    assert_eq!(make_test("G[2,5] (R_x > 5 || R_x < 0)", false), Some(true));
}

#[test]
fn test_globally_add() {
    assert_eq!(
        make_test("G[2,5] (R_x + R_y > 5 && R_x - R_y < 0)", false),
        Some(true)
    );
}

#[test]
fn test_globally_add_many() {
    assert_eq!(
        make_test("G[2,5] (R_x + R_y - R_z + R_x > 5 && R_x - R_y < 0)", false),
        Some(true)
    );
}

#[test]
fn test_release() {
    assert_eq!(
        make_test("(R_x == 10) R[1,6] (R_x < 10)", false),
        Some(true)
    );
}

#[test]
fn test_abs() {
    assert_eq!(
        make_test("G[0,5] (|x| > 20 || |x| < 10) && F[0,5] (x == -15)", false),
        Some(false)
    );
}

#[test]
fn test_mltl() {
    let formula = "F[58,92] ((a1) U[87,100] ((a1 && a0 && ! a1) U[9,100] (a0)))";
    assert_eq!(make_test(formula, false), Some(false));
    assert_eq!(make_test(formula, true), Some(true));
}

#[test]
fn test_release_false() {
    assert_eq!(make_test("false R[0,10] a", false), Some(true));
}

#[test]
fn test_gfgg() {
    assert_eq!(
        make_test("G[0,6] F[2,4] a && G[0,6] (a -> G[1,3] !a)", false),
        Some(false)
    );
}

#[test]
fn test_jump1_0() {
    assert_eq!(
        make_test("!a && G[10,20] !a && F[0,20] a", false),
        Some(true)
    );
}

#[test]
fn test_jump1_g() {
    assert_eq!(
        make_test("G[0,10] !a && F[5,20] a && G[15,25] !a", false),
        Some(true)
    );
}

#[test]
fn test_jump1_f() {
    assert_eq!(
        make_test("F[0,10] !a && G[0,9] a && F[10,20] a && G[15,20] !a", false),
        Some(true)
    );
}

#[test]
fn test_jump1_u() {
    assert_eq!(
        make_test(
            "b U[0,10] !a && G[0,9] a && F[10,20] a && G[15,20] !a",
            false
        ),
        Some(true)
    );
}

#[test]
fn test_g_is_derived() {
    assert_eq!(
        make_test("G[0,6] (!(a0 U[2,10] (F[0,6] (a0))))", true),
        Some(true)
    );
}

#[test]
fn test_u_parent() {
    assert_eq!(
        make_test("(G[0,89] F[88,100] a2 U[0,78] !a1) && a1", true),
        Some(true)
    );
}

#[test]
fn test_implication_negation() {
    assert_eq!(
        make_test("G[0, 6] !a && (G[0, 3] !a -> F[0, 3] a)", false),
        Some(false)
    );
}

#[test]
fn test_globally_imply_merge() {
    assert_eq!(
        make_test(
            "G[0, 10] (a -> G[10, 15] b) && G[0, 10] a && G[16, 16] !b",
            false
        ),
        Some(false)
    );
}

#[test]
fn test_until_mltl() {
    assert_eq!(
        make_test("a U[39, 77] (G[0, 15] a) && G[82, 100] !a", true),
        Some(true)
    );
}

#[test]
fn test_gfg() {
    assert_eq!(
        make_test("G[5, 10] F[8, 10] a && G[16, 17] !a", false),
        Some(true)
    );
}

#[test]
fn test_fgf() {
    assert_eq!(
        make_test("F[0,25] G[0,30] a0 && (F[0,28] ! a0) && a0", false),
        Some(true)
    );
}

#[test]
fn test_fgfg() {
    assert_eq!(
        make_test("F[0,25] G[0,30] a0 && (F[0,28] ! a0) && G[0, 1] a0", false),
        Some(true)
    );
}

#[test]
fn test_step_r() {
    assert_eq!(
        make_test("(a R[0,10] b) && G[6,10] (!a && !b) && !a", false),
        Some(true)
    );
}

#[test]
fn test_jump_error() {
    assert_eq!(
        make_test("G[0,4] (!b) && a && (!a R[0,3] (c U[0,4] b))", false),
        Some(false)
    );
}

#[test]
fn test_jump_until_satisfied() {
    assert_eq!(
        make_test("F[0,10] a && !b && ((G[0,5] !a) U[0,15] b)", false),
        Some(true)
    );
}

#[test]
fn test_jump_completeness() {
    assert_eq!(
        make_test(
            "(a U[0, 10] (b && G[20, 30] c)) && G[0, 27] !c && G[10, 10] !b",
            false
        ),
        Some(true)
    )
}

#[test]
fn test_jump_completeness_obstacle_nested() {
    assert_eq!(
        make_test(
            "G[5, 5] G[5, 5] !a && b U[0, 5] G[10, 10] a && G[15, 15] !a",
            false
        ),
        Some(true)
    )
}

#[test]
fn test_jump_completeness_release_obstacle() {
    assert_eq!(
        make_test(
            "(F[0, 10] (b && G[20, 30] c)) && a R[0, 27] !c && G[10, 10] !b && G[0,50] !a",
            false
        ),
        Some(true)
    )
}

#[test]
fn test_jump_completeness_release_target_conflict() {
    assert_eq!(
        make_test(
            "(F[0, 10] (b && G[20, 30] c)) && G[0, 27] !c && G[10, 10] !b && (F[10,10] !c) R[18,19] a && F[19,19] !a",
            false
        ),
        Some(true)
    )
}

#[test]
fn test_jump_completeness_release_target_conflict_postponed() {
    assert_eq!(
        make_test(
            "(F[0, 15] (b && G[20, 30] c)) && G[0, 27] !c && G[15, 15] !b && (d && F[10,10] !c) R[17,20] a && F[20,20] !a && F[17,17] !d",
            false
        ),
        Some(true)
    )
}

#[test]
fn test_jump_soundness() {
    assert_eq!(
        make_test(
            "(G[0,1] (F[5,5] a)) U[0,5] (!b) && G[0,4] b && G[8,8] !a",
            true
        ),
        Some(false)
    )
}

#[test]
fn test_jump_soundness_f() {
    assert_eq!(
        make_test(
            "(G[0,1] (F[5,5] a)) U[0,5] (!b) && G[0,4] b && F[8,8] !a",
            true
        ),
        Some(false)
    )
}
