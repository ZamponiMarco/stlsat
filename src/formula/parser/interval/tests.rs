#[rstest::rstest]
#[case("[1/0,2]")]
#[case("[-0.5,2]")]
#[case("[0,-1]")]
#[case("[0.12345678901234567890,2]")]
fn test_reject_invalid_interval_bounds(#[case] input: &str) {
    assert!(super::parse_interval(input).is_err());
}
