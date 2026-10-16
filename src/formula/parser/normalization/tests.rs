use crate::formula::parser::parse_formula;

#[rstest::rstest]
#[case("G[0.5,1.25] a", "G[2,5] a")]
#[case("F[1/3,2/3] a", "F[1,2] a")]
#[case("G[0.50,1.00] a", "G[1,2] a")]
#[case("G[0,2] (F[0.5,1] a)", "G[0,4] (F[1,2] a)")]
#[case("(F[0.5,1] a) && (G[0.25,2] b)", "(F[2,4] a) && (G[1,8] b)")]
#[case("a U[1/3,1] b R[0.25,2] c", "a U[4,12] b R[3,24] c")]
#[case("F[0.5,1] a -> G[0.25,2] b", "F[2,4] a -> G[1,8] b")]
#[case("F[0.5,1] a <-> G[0.25,2] b", "F[2,4] a <-> G[1,8] b")]
#[case("!(F[0.5,1] a) || X b", "!(F[1,2] a) || F[2,2] b")]
#[case("F[0.5,1] (x > 0.125)", "F[1,2] (x > 0.125)")]
#[case("G[0,2147483647] a", "G[0,2147483647] a")]
fn test_normalize_fractional_times(#[case] input: &str, #[case] expected: &str) {
    let (remaining, actual) = parse_formula(input).unwrap();
    assert_eq!(remaining, "");
    let (remaining, expected) = parse_formula(expected).unwrap();
    assert_eq!(remaining, "");
    assert!(actual.eq_structural(&expected), "{actual} != {expected}");
}

#[rstest::rstest]
#[case("G[0,2147483648] a")]
#[case("G[0,2147483647] F[0.5,1] a")]
#[case("G[1/4000000007,1] F[1/4000000009,1] a")]
fn test_reject_time_normalization_overflow(#[case] input: &str) {
    assert!(parse_formula(input).is_err());
}
