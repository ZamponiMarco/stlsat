//! Formula grammar, ordered from lowest to highest operator precedence.

#[cfg(test)]
mod tests;

use super::{
    expression::parse_expr,
    interval::{ParsedInterval, parse_interval},
    normalization::ParsedFormula,
};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, space0},
    combinator::map,
    multi::fold_many0,
    sequence::{delimited, pair, preceded, terminated},
};
use num_rational::Ratio;

pub(super) fn parse_implication(input: &str) -> IResult<&str, ParsedFormula> {
    let (input, init) = parse_logical_or(input)?;

    fold_many0(
        pair(
            preceded(space0, alt((tag("->"), tag("<->")))),
            preceded(space0, parse_implication),
        ),
        move || init.clone(),
        |acc, (op_out, right)| match op_out {
            "->" => ParsedFormula::imply(acc, right),
            "<->" => ParsedFormula::or(vec![
                ParsedFormula::and(vec![acc.clone(), right.clone()]),
                ParsedFormula::and(vec![ParsedFormula::not(acc), ParsedFormula::not(right)]),
            ]),
            _ => unreachable!(),
        },
    )
    .parse(input)
}

fn parse_logical_or(input: &str) -> IResult<&str, ParsedFormula> {
    let (input, init) = parse_logical_and(input)?;

    fold_many0(
        pair(
            preceded(space0, alt((tag("||"), tag("|")))),
            preceded(space0, parse_logical_and),
        ),
        move || init.clone(),
        |acc, (_, right)| ParsedFormula::or(vec![acc, right]),
    )
    .parse(input)
}

fn parse_logical_and(input: &str) -> IResult<&str, ParsedFormula> {
    let (input, init) = parse_binary_temporal(input)?;

    fold_many0(
        pair(
            preceded(space0, alt((tag("&&"), tag("&")))),
            preceded(space0, parse_binary_temporal),
        ),
        move || init.clone(),
        |acc, (_, right)| ParsedFormula::and(vec![acc, right]),
    )
    .parse(input)
}

fn parse_binary_temporal(input: &str) -> IResult<&str, ParsedFormula> {
    let (input, init) = parse_formula_term(input)?;

    fold_many0(
        pair(
            alt((
                map(
                    pair(preceded(space0, tag("U")), parse_interval),
                    |(_, interval)| ("U", interval),
                ),
                map(
                    pair(preceded(space0, tag("R")), parse_interval),
                    |(_, interval)| ("R", interval),
                ),
            )),
            preceded(space0, parse_formula_term),
        ),
        move || init.clone(),
        |acc, ((op, interval), right)| match op {
            "U" => ParsedFormula::u(interval, acc, right),
            "R" => ParsedFormula::r(interval, acc, right),
            _ => unreachable!(),
        },
    )
    .parse(input)
}

fn parse_formula_term(input: &str) -> IResult<&str, ParsedFormula> {
    alt((
        map(
            (tag("G"), parse_interval, space0, parse_formula_term),
            |(_, interval, _, phi)| ParsedFormula::g(interval, phi),
        ),
        map(
            (tag("F"), parse_interval, space0, parse_formula_term),
            |(_, interval, _, phi)| ParsedFormula::f(interval, phi),
        ),
        map((char('!'), space0, parse_formula_term), |(_, _, phi)| {
            ParsedFormula::not(phi)
        }),
        map((char('X'), space0, parse_formula_term), |(_, _, phi)| {
            ParsedFormula::f(
                ParsedInterval {
                    lower: Ratio::from_integer(1),
                    upper: Ratio::from_integer(1),
                },
                phi,
            )
        }),
        delimited(
            preceded(space0, char('(')),
            delimited(space0, parse_implication, space0),
            terminated(char(')'), space0),
        ),
        map(parse_expr, ParsedFormula::prop),
    ))
    .parse(input)
}
