//! Exact integer, fraction, and decimal literals shared by expressions and times.

use nom::{
    IResult, Parser,
    character::complete::{char, digit1},
    combinator::{map_res, opt, recognize},
    sequence::{pair, preceded},
};
use num_rational::Ratio;
use std::str::FromStr;

fn parse_number(input: &str) -> IResult<&str, i64> {
    map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| {
        i64::from_str(s)
    })
    .parse(input)
}

pub(super) fn parse_ratio(input: &str) -> IResult<&str, Ratio<i64>> {
    map_res(
        pair(parse_number, preceded(char('/'), parse_number)),
        |(num, den)| {
            // Normalize in i128 so even i64::MIN cannot overflow during reduction.
            if den == 0 {
                return Err("zero denominator");
            }
            let ratio = Ratio::new(i128::from(num), i128::from(den));
            Ok(Ratio::new(
                i64::try_from(*ratio.numer()).map_err(|_| "ratio overflow")?,
                i64::try_from(*ratio.denom()).map_err(|_| "ratio overflow")?,
            ))
        },
    )
    .parse(input)
}

pub(super) fn parse_decimal(input: &str) -> IResult<&str, Ratio<i64>> {
    map_res(
        recognize((opt(char('-')), digit1, opt((char('.'), digit1)))),
        |literal: &str| -> Result<Ratio<i64>, &'static str> {
            if let Some((integer, fraction)) = literal.split_once('.') {
                let scale = 10i64
                    .checked_pow(fraction.len().try_into().map_err(|_| "decimal too long")?)
                    .ok_or("decimal too long")?;
                let integer = integer.parse::<i64>().map_err(|_| "invalid integer")?;
                let fraction = fraction.parse::<i64>().map_err(|_| "invalid fraction")?;
                let signed_fraction = if literal.starts_with('-') {
                    -fraction
                } else {
                    fraction
                };
                let numerator = integer
                    .checked_mul(scale)
                    .and_then(|n| n.checked_add(signed_fraction))
                    .ok_or("decimal overflow")?;
                Ok(Ratio::new(numerator, scale))
            } else {
                literal
                    .parse::<i64>()
                    .map(Ratio::from_integer)
                    .map_err(|_| "invalid integer")
            }
        },
    )
    .parse(input)
}
