//! Nonnegative rational interval bounds. Integer conversion happens after parsing.

#[cfg(test)]
mod tests;

use super::numbers::{parse_decimal, parse_ratio};
use nom::{
    IResult, Parser,
    branch::alt,
    character::complete::{char, space0},
    combinator::{map, verify},
    sequence::{delimited, preceded},
};
use num_rational::Ratio;

#[derive(Clone)]
pub(super) struct ParsedInterval {
    pub(super) lower: Ratio<i64>,
    pub(super) upper: Ratio<i64>,
}

pub(super) fn parse_interval(input: &str) -> IResult<&str, ParsedInterval> {
    map(
        delimited(
            char('['),
            (
                preceded(space0, parse_interval_number),
                preceded(delimited(space0, char(','), space0), parse_interval_number),
            ),
            preceded(space0, char(']')),
        ),
        |(lower, upper)| ParsedInterval { lower, upper },
    )
    .parse(input)
}

fn parse_interval_number(input: &str) -> IResult<&str, Ratio<i64>> {
    verify(alt((parse_ratio, parse_decimal)), |value: &Ratio<i64>| {
        *value >= Ratio::from_integer(0)
    })
    .parse(input)
}
