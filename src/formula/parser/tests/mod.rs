use crate::formula::{Interval, parser::parse_formula};

#[test]
fn test_fractional_times_preserve_remaining_input() {
    let (remaining, formula) = parse_formula("F[0.5,1] a trailing").unwrap();
    assert_eq!(remaining, " trailing");
    assert_eq!(
        formula.get_interval(),
        Some(Interval { lower: 1, upper: 2 })
    );
}
