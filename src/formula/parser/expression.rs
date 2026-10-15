//! Arithmetic expressions and atomic propositions.

#[cfg(test)]
mod tests;

use super::numbers::{parse_decimal, parse_ratio};
use crate::formula::{AExpr, ArithOp, Expr, RelOp};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, tag_no_case},
    character::complete::{alpha1, char, digit1, space0},
    combinator::{map, recognize},
    multi::{fold_many0, many0},
    sequence::{delimited, pair},
};

fn parse_arith_op(input: &str) -> IResult<&str, ArithOp> {
    alt((
        map(char('+'), |_| ArithOp::Add),
        map(char('-'), |_| ArithOp::Sub),
    ))
    .parse(input)
}

fn parse_rel_op(input: &str) -> IResult<&str, RelOp> {
    alt((
        map(tag("<="), |_| RelOp::Le),
        map(tag("<"), |_| RelOp::Lt),
        map(tag(">="), |_| RelOp::Ge),
        map(tag(">"), |_| RelOp::Gt),
        map(tag("=="), |_| RelOp::Eq),
        map(tag("!="), |_| RelOp::Ne),
    ))
    .parse(input)
}

fn parse_aexpr(input: &str) -> IResult<&str, AExpr> {
    fn aexpr_term(input: &str) -> IResult<&str, AExpr> {
        alt((
            // A rational coefficient multiplying a variable.
            map(
                (
                    alt((parse_ratio, parse_decimal)),
                    delimited(space0, char('*'), space0),
                    recognize(pair(alpha1, many0(alt((alpha1, digit1, tag("_")))))),
                ),
                |(coefficient, _, name): (_, _, &str)| AExpr::Var(name.into(), coefficient),
            ),
            // Number
            map(
                alt((
                    parse_ratio,   // Try a/b first
                    parse_decimal, // Fallback to integer or float
                )),
                AExpr::Num,
            ),
            // Variable
            map(
                recognize(pair(alpha1, many0(alt((alpha1, digit1, tag("_")))))),
                |s: &str| AExpr::Var(s.into(), num_rational::Ratio::from_integer(1)),
            ),
            // Absolute value: |expr|
            map(delimited(char('|'), parse_aexpr, char('|')), |expr| {
                AExpr::Abs(Box::new(expr))
            }),
            // Parenthesized expression
            delimited(char('('), parse_aexpr, char(')')),
        ))
        .parse(input)
    }
    let (input, init) = aexpr_term(input)?;

    fold_many0(
        pair(delimited(space0, parse_arith_op, space0), aexpr_term),
        move || init.clone(),
        |acc, (op, rhs)| AExpr::BinOp {
            op,
            left: Box::new(acc),
            right: Box::new(rhs),
        },
    )
    .parse(input)
}

pub(super) fn parse_expr(input: &str) -> IResult<&str, Expr> {
    alt((
        map(
            (parse_aexpr, space0, parse_rel_op, space0, parse_aexpr),
            |(left, _, op, _, right)| Expr::real(op, left, right),
        ),
        map(tag_no_case("true"), |_| Expr::true_expr()),
        map(tag_no_case("false"), |_| Expr::false_expr()),
        map(
            recognize(pair(alpha1, many0(alt((alpha1, digit1, tag("_")))))),
            |s: &str| Expr::bool(s.into()),
        ),
    ))
    .parse(input)
}
