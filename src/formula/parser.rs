use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, tag_no_case},
    character::complete::{alpha1, char, digit1, space0},
    combinator::{map, map_res, opt, recognize},
    multi::{fold_many0, many0},
    sequence::{delimited, pair, preceded, terminated},
};
use num_rational::Ratio;
use std::{fs, path::Path, str::FromStr};

use crate::formula::{AExpr, ArithOp, Expr, Formula, Interval, RelOp};

#[cfg(test)]
mod tests;

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

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    alt((
        map(
            (
                parse_aexpr,
                nom::character::complete::space0,
                parse_rel_op,
                nom::character::complete::space0,
                parse_aexpr,
            ),
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

fn parse_interval(input: &str) -> IResult<&str, Interval> {
    map(
        delimited(
            char('['),
            (
                preceded(space0, parse_interval_number),
                preceded(delimited(space0, char(','), space0), parse_interval_number),
            ),
            preceded(space0, char(']')),
        ),
        |(lower, upper)| Interval { lower, upper },
    )
    .parse(input)
}

fn parse_interval_number(input: &str) -> IResult<&str, i32> {
    map_res(digit1, |s: &str| i32::from_str(s)).parse(input)
}

fn parse_number(input: &str) -> IResult<&str, i64> {
    map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| {
        i64::from_str(s)
    })
    .parse(input)
}

fn parse_ratio(input: &str) -> IResult<&str, Ratio<i64>> {
    map(
        pair(parse_number, preceded(char('/'), parse_number)),
        |(num, den)| Ratio::new(num, den),
    )
    .parse(input)
}

fn parse_decimal(input: &str) -> IResult<&str, Ratio<i64>> {
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

pub fn parse_formula(input: &str) -> IResult<&str, Formula> {
    fn parse_implication(input: &str) -> IResult<&str, Formula> {
        let (input, init) = parse_logical_or(input)?;

        fold_many0(
            pair(
                preceded(space0, alt((tag("->"), tag("<->")))),
                preceded(space0, parse_implication),
            ),
            move || init.clone(),
            |acc, (op_out, right)| match op_out {
                "->" => Formula::imply(acc, right),
                "<->" => Formula::or(vec![
                    Formula::and(vec![acc.clone(), right.clone()]),
                    Formula::and(vec![Formula::not(acc), Formula::not(right)]),
                ]),
                _ => unreachable!(),
            },
        )
        .parse(input)
    }

    fn parse_logical_or(input: &str) -> IResult<&str, Formula> {
        let (input, init) = parse_logical_and(input)?;

        fold_many0(
            pair(
                preceded(space0, alt((tag("||"), tag("|")))),
                preceded(space0, parse_logical_and),
            ),
            move || init.clone(),
            |acc, (_, right)| Formula::or(vec![acc, right]),
        )
        .parse(input)
    }

    fn parse_logical_and(input: &str) -> IResult<&str, Formula> {
        let (input, init) = parse_binary_temporal(input)?;

        fold_many0(
            pair(
                preceded(space0, alt((tag("&&"), tag("&")))),
                preceded(space0, parse_binary_temporal),
            ),
            move || init.clone(),
            |acc, (_, right)| Formula::and(vec![acc, right]),
        )
        .parse(input)
    }

    fn parse_binary_temporal(input: &str) -> IResult<&str, Formula> {
        let (input, init) = parse_formula_term(input)?;

        fold_many0(
            pair(
                alt((
                    map(
                        pair(preceded(space0, tag("U")), parse_interval),
                        |(_, interval)| ("U", Some(interval)),
                    ),
                    map(
                        pair(preceded(space0, tag("R")), parse_interval),
                        |(_, interval)| ("R", Some(interval)),
                    ),
                )),
                preceded(space0, parse_formula_term),
            ),
            move || init.clone(),
            |acc, ((op, interval), right)| match op {
                "U" => Formula::u(interval.unwrap(), acc, right),
                "R" => Formula::r(interval.unwrap(), acc, right),
                _ => panic!(), // Should not happen
            },
        )
        .parse(input)
    }

    fn parse_formula_term(input: &str) -> IResult<&str, Formula> {
        alt((
            map(
                (tag("G"), parse_interval, space0, parse_formula_term),
                |(_, interval, _, phi)| Formula::g(interval, phi),
            ),
            map(
                (tag("F"), parse_interval, space0, parse_formula_term),
                |(_, interval, _, phi)| Formula::f(interval, phi),
            ),
            map((char('!'), space0, parse_formula_term), |(_, _, phi)| {
                Formula::not(phi)
            }),
            map((char('X'), space0, parse_formula_term), |(_, _, phi)| {
                Formula::f(Interval { lower: 1, upper: 1 }, phi)
            }),
            delimited(
                preceded(space0, char('(')),
                delimited(space0, parse_formula, space0),
                terminated(char(')'), space0),
            ),
            map(parse_expr, Formula::prop),
        ))
        .parse(input)
    }

    parse_implication(input)
}

pub fn parse_stl_file(filename: &str) {
    let path = Path::new("resources").join(filename);
    println!("\n=== Parsing {filename} ===");

    match fs::read_to_string(&path) {
        Ok(content) => {
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue; // Skip empty lines and comments
                }

                println!("\nLine {}: {}", line_num + 1, line);
                match parse_formula(line) {
                    Ok((remaining, formula)) => {
                        println!("  ✓ Parsed: {formula:?}");
                        if !remaining.is_empty() {
                            println!("  ⚠ Remaining: '{remaining}'");
                        }
                    }
                    Err(e) => {
                        println!("  ✗ Parse error: {e:?}");
                    }
                }
            }
        }
        Err(e) => {
            println!("Error reading file {filename}: {e}");
        }
    }
}
