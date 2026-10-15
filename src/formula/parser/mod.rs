//! Parse STL formulas, then normalize rational time bounds to integer time units.

use crate::formula::Formula;
use nom::IResult;

mod expression;
mod file;
mod interval;
mod normalization;
mod numbers;
mod syntax;

pub use file::parse_stl_file;

#[cfg(test)]
mod tests;

pub fn parse_formula(input: &str) -> IResult<&str, Formula> {
    let (remaining, parsed) = syntax::parse_implication(input)?;
    let formula = parsed.normalize();
    formula.map(|formula| (remaining, formula)).ok_or_else(|| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Verify))
    })
}
