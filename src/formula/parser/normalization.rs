//! Intermediate formula tree and conversion from rational times to integer times.
//!
//! Parsing builds this tree without changing time units. Once parsing finishes,
//! one least common denominator is used for every interval in the tree.

#[cfg(test)]
mod tests;

use super::interval::ParsedInterval;
use crate::formula::{Expr, Formula, Interval};
use num_rational::Ratio;

#[derive(Clone)]
pub(super) enum ParsedFormula {
    Prop(Expr),
    And(Vec<Self>),
    Or(Vec<Self>),
    Imply(Box<Self>, Box<Self>),
    Not(Box<Self>),
    G(ParsedInterval, Box<Self>),
    F(ParsedInterval, Box<Self>),
    U(ParsedInterval, Box<Self>, Box<Self>),
    R(ParsedInterval, Box<Self>, Box<Self>),
}

impl ParsedFormula {
    /// Use a single time unit for the whole formula, including nested subformulas.
    pub(super) fn normalize(self) -> Option<Formula> {
        let mut time_scale = 1;
        self.update_time_scale(&mut time_scale)?;
        self.into_integer_formula(time_scale)
    }

    pub(super) fn prop(expr: Expr) -> Self {
        Self::Prop(expr)
    }

    pub(super) fn and(operands: Vec<Self>) -> Self {
        Self::And(operands)
    }

    pub(super) fn or(operands: Vec<Self>) -> Self {
        Self::Or(operands)
    }

    pub(super) fn imply(left: Self, right: Self) -> Self {
        Self::Imply(Box::new(left), Box::new(right))
    }

    pub(super) fn not(phi: Self) -> Self {
        Self::Not(Box::new(phi))
    }

    pub(super) fn g(interval: ParsedInterval, phi: Self) -> Self {
        Self::G(interval, Box::new(phi))
    }

    pub(super) fn f(interval: ParsedInterval, phi: Self) -> Self {
        Self::F(interval, Box::new(phi))
    }

    pub(super) fn u(interval: ParsedInterval, left: Self, right: Self) -> Self {
        Self::U(interval, Box::new(left), Box::new(right))
    }

    pub(super) fn r(interval: ParsedInterval, left: Self, right: Self) -> Self {
        Self::R(interval, Box::new(left), Box::new(right))
    }

    /// Include every interval in this subtree in the shared least common denominator.
    fn update_time_scale(&self, time_scale: &mut i64) -> Option<()> {
        match self {
            Self::Prop(_) => {}
            Self::And(operands) | Self::Or(operands) => {
                for operand in operands {
                    operand.update_time_scale(time_scale)?;
                }
            }
            Self::Imply(left, right) => {
                left.update_time_scale(time_scale)?;
                right.update_time_scale(time_scale)?;
            }
            Self::Not(phi) => phi.update_time_scale(time_scale)?,
            Self::G(interval, phi) | Self::F(interval, phi) => {
                update_time_scale_for_interval(interval, time_scale)?;
                phi.update_time_scale(time_scale)?;
            }
            Self::U(interval, left, right) | Self::R(interval, left, right) => {
                update_time_scale_for_interval(interval, time_scale)?;
                left.update_time_scale(time_scale)?;
                right.update_time_scale(time_scale)?;
            }
        }
        Some(())
    }

    fn into_integer_formula(self, time_scale: i64) -> Option<Formula> {
        Some(match self {
            Self::Prop(expr) => Formula::prop(expr),
            Self::And(operands) => Formula::and(
                operands
                    .into_iter()
                    .map(|operand| operand.into_integer_formula(time_scale))
                    .collect::<Option<_>>()?,
            ),
            Self::Or(operands) => Formula::or(
                operands
                    .into_iter()
                    .map(|operand| operand.into_integer_formula(time_scale))
                    .collect::<Option<_>>()?,
            ),
            Self::Imply(left, right) => Formula::imply(
                left.into_integer_formula(time_scale)?,
                right.into_integer_formula(time_scale)?,
            ),
            Self::Not(phi) => Formula::not(phi.into_integer_formula(time_scale)?),
            Self::G(interval, phi) => Formula::g(
                scale_interval(interval, time_scale)?,
                phi.into_integer_formula(time_scale)?,
            ),
            Self::F(interval, phi) => Formula::f(
                scale_interval(interval, time_scale)?,
                phi.into_integer_formula(time_scale)?,
            ),
            Self::U(interval, left, right) => Formula::u(
                scale_interval(interval, time_scale)?,
                left.into_integer_formula(time_scale)?,
                right.into_integer_formula(time_scale)?,
            ),
            Self::R(interval, left, right) => Formula::r(
                scale_interval(interval, time_scale)?,
                left.into_integer_formula(time_scale)?,
                right.into_integer_formula(time_scale)?,
            ),
        })
    }
}

/// Extend the shared time scale to make both interval bounds integers.
fn update_time_scale_for_interval(interval: &ParsedInterval, time_scale: &mut i64) -> Option<()> {
    for bound in [interval.lower, interval.upper] {
        *time_scale = checked_lcm(*time_scale, *bound.denom())?;
    }
    Some(())
}

/// Divide before multiplying to avoid unnecessary intermediate overflow.
fn checked_lcm(left: i64, right: i64) -> Option<i64> {
    (left / num_integer::gcd(left, right)).checked_mul(right)
}

fn scale_interval(interval: ParsedInterval, time_scale: i64) -> Option<Interval> {
    Some(Interval {
        lower: scale_bound(interval.lower, time_scale)?,
        upper: scale_bound(interval.upper, time_scale)?,
    })
}

fn scale_bound(value: Ratio<i64>, time_scale: i64) -> Option<i32> {
    // The common denominator is divisible by every reduced bound's denominator.
    let integer = value.numer().checked_mul(time_scale / value.denom())?;
    i32::try_from(integer).ok()
}
