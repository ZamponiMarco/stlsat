use std::{fmt::Display, sync::Arc};

use clap::Parser;
use num_rational::Ratio;
use rand::{Rng, seq::IndexedRandom, seq::SliceRandom};
use stlsat::formula::{AExpr, ArithOp, Expr, ExprKind, Formula, Interval, RelOp, VariableName};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ConstraintType {
    Simple,
    #[clap(name = "dl")]
    DifferenceLogic,
    Linear,
    #[default]
    All,
}

impl Display for ConstraintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintType::Simple => write!(f, "simple"),
            ConstraintType::DifferenceLogic => write!(f, "dl"),
            ConstraintType::Linear => write!(f, "linear"),
            ConstraintType::All => write!(f, "all"),
        }
    }
}

#[derive(Parser, Debug)]
pub struct GeneratorArgs {
    /// Output folder for generated .stl files
    #[arg(short = 'o', long)]
    pub output_folder: String,

    /// File prefix for generated files
    #[arg(short = 'p', long, default_value = "formula")]
    pub file_prefix: String,

    /// Number of formulas to generate
    #[arg(short = 'n', long, default_value_t = 5)]
    pub num_formulas: usize,

    /// Number of conjunctions per formula
    #[arg(short = 'j', long, default_value_t = 5)]
    pub num_conjunctions: i32,

    /// Number of boolean variables (atoms a0, a1, …)
    #[arg(short = 'b', long, default_value_t = 0)]
    pub num_bool_vars: usize,

    /// Number of real-valued variables (x0, x1, …)
    #[arg(short = 'r', long, default_value_t = 3)]
    pub num_real_vars: usize,

    /// Maximum number of real constraints per formula
    #[arg(short = 'c', long, default_value_t = 5)]
    pub max_real_constraints: usize,

    /// Maximum temporal horizon (upper bound of intervals)
    #[arg(short = 'l', long, default_value_t = 100)]
    pub max_horizon: i32,

    /// Maximum interval length (upper bound of interval upper - lower)
    #[arg(long, default_value_t = 50)]
    pub max_interval: i32,

    /// Base probability of stopping recursion (approx inverse of expected depth)
    #[arg(long, default_value_t = 0.1)]
    pub p_stop_base: f64,

    /// Probability that a chosen operator is temporal (G,F,U,R)
    #[arg(long, default_value_t = 0.5)]
    pub p_temporal: f64,

    /// Whether to enforce intervals starting at zero
    #[arg(long, default_value_t = false)]
    pub zero_interval_start: bool,

    /// Which type to use for real constraints
    #[arg(long, default_value_t = ConstraintType::All)]
    pub constraint_type: ConstraintType,
}

pub struct RandomGenerator {
    constraints: Vec<ExprKind>,
    conjuncts: i32,
    max_horizon: i32,
    max_interval: i32,
    p_stop_base: f64,
    p_temporal: f64,
    zero_interval_start: bool,
}

impl RandomGenerator {
    #[must_use]
    pub fn new(args: &GeneratorArgs) -> Self {
        let bool_vars: Vec<VariableName> = (0..args.num_bool_vars)
            .map(|i| Arc::from(format!("a{i}")))
            .collect();
        let real_vars: Vec<VariableName> = (0..args.num_real_vars)
            .map(|i| Arc::from(format!("x{i}")))
            .collect();

        assert!(
            !(bool_vars.is_empty() && real_vars.is_empty()),
            "At least one boolean or real variable must be defined."
        );

        let real_constraints: Vec<ExprKind> = (0..args.max_real_constraints)
            .map(|_| random_rel(args.constraint_type, &real_vars))
            .collect();
        let bool_constraints = bool_vars
            .iter()
            .map(|v| ExprKind::Atom(v.clone()))
            .collect();

        Self {
            constraints: [real_constraints, bool_constraints].concat(),
            conjuncts: args.num_conjunctions,
            max_horizon: args.max_horizon,
            max_interval: args.max_interval,
            p_stop_base: args.p_stop_base,
            p_temporal: args.p_temporal,
            zero_interval_start: args.zero_interval_start,
        }
    }

    fn random_interval(&self, horizon: i32) -> Interval {
        let mut rng = rand::rng();
        let mode = if self.zero_interval_start {
            1
        } else {
            rng.random_range(1..=3)
        };

        match mode {
            1 => {
                let upper = rng.random_range(0..=self.max_interval.min(self.max_horizon - horizon));
                Interval { lower: 0, upper }
            }
            2 => {
                let lower = rng.random_range(0..=self.max_interval.min(self.max_horizon - horizon));
                let upper =
                    rng.random_range(lower..=self.max_interval.min(self.max_horizon - horizon));
                Interval { lower, upper }
            }
            3 => {
                let lower = rng.random_range(0..=self.max_interval.min(self.max_horizon - horizon));
                Interval {
                    lower,
                    upper: self.max_horizon - horizon,
                }
            }
            _ => unreachable!(),
        }
    }

    fn random_proposition(&self) -> Formula {
        let mut rng = rand::rng();

        let kind = self.constraints.choose(&mut rng).unwrap().clone();
        let negated: bool = rng.random_bool(0.5);
        if negated {
            return Formula::not(Formula::Prop(Expr::from_expr(kind)));
        }
        Formula::Prop(Expr::from_expr(kind))
    }

    #[must_use]
    pub fn generate_single_formula(&self, depth: usize, horizon: i32) -> Formula {
        let mut rng = rand::rng();

        // stop condition
        if rng.random::<f64>() < self.p_stop_base * (depth as f64) {
            return self.random_proposition();
        }

        if rng.random::<f64>() < self.p_temporal && horizon < self.max_horizon {
            // Temporal operator
            let interval = self.random_interval(horizon);
            let top = rng.random_range(1..=4);
            match top {
                1 => {
                    let phi = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    Formula::g(interval, phi)
                }
                2 => {
                    let phi = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    Formula::f(interval, phi)
                }
                3 => {
                    let left = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    let right = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    Formula::u(interval, left, right)
                }
                4 => {
                    let left = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    let right = self.generate_single_formula(depth + 1, horizon + interval.upper);
                    Formula::r(interval, left, right)
                }
                _ => unreachable!(),
            }
        } else {
            // Non temporal operator
            let op = rng.random_range(1..=3);
            match op {
                1 => {
                    let left = self.generate_single_formula(depth + 1, horizon);
                    let right = self.generate_single_formula(depth + 1, horizon);
                    Formula::and(vec![left, right])
                }
                2 => {
                    let left = self.generate_single_formula(depth + 1, horizon);
                    let right = self.generate_single_formula(depth + 1, horizon);
                    Formula::or(vec![left, right])
                }
                3 => {
                    let left = self.generate_single_formula(depth + 1, horizon);
                    let right = self.generate_single_formula(depth + 1, horizon);
                    Formula::imply(left, right)
                }
                _ => unreachable!(),
            }
        }
    }

    #[must_use]
    pub fn generate_formula(&self) -> Formula {
        Formula::and(
            (0..self.conjuncts)
                .map(|_| self.generate_single_formula(0, 0))
                .collect(),
        )
    }
}

fn main() {
    let args = GeneratorArgs::parse();
    let rng = RandomGenerator::new(&args);

    // Create output folder if it doesn't exist
    std::fs::create_dir_all(&args.output_folder).expect("Failed to create output folder");

    println!("Args: {args:#?}");
    println!(
        "Generating {} formulas to folder: {}",
        args.num_formulas, args.output_folder
    );

    for i in 0..args.num_formulas {
        let formula = rng.generate_formula();
        let filename = format!("{}/{}_{}.stl", args.output_folder, args.file_prefix, i + 1);
        std::fs::write(&filename, format!("{formula}")).expect("Failed to write formula to file");
        println!("Generated: {filename}");
    }

    println!("Done!");
}

#[must_use]
fn random_rel(constraint_type: ConstraintType, real_vars: &[VariableName]) -> ExprKind {
    let mut rng = rand::rng();
    match constraint_type {
        ConstraintType::Simple => random_simple(real_vars),
        ConstraintType::DifferenceLogic => match rng.random_range(0..2) {
            0 => random_simple(real_vars),
            _ => random_diff_logic(real_vars),
        },
        ConstraintType::Linear | ConstraintType::All => match rng.random_range(0..3) {
            0 => random_simple(real_vars),
            1 => random_diff_logic(real_vars),
            _ => random_linear(real_vars),
        },
    }
}

#[must_use]
pub fn random_rel_op() -> RelOp {
    let mut rng = rand::rng();
    [
        RelOp::Lt,
        RelOp::Le,
        RelOp::Gt,
        RelOp::Ge,
        RelOp::Eq,
        RelOp::Ne,
    ]
    .choose(&mut rng)
    .unwrap()
    .clone()
}

fn random_simple(real_vars: &[VariableName]) -> ExprKind {
    let mut rng = rand::rng();
    let op = random_rel_op();
    let left = AExpr::Var(real_vars.choose(&mut rng).unwrap().clone());
    let right = AExpr::Num(Ratio::new(
        rng.random_range(-10..10),
        rng.random_range(1..10),
    ));
    ExprKind::Rel { op, left, right }
}

fn random_diff_logic(real_vars: &[VariableName]) -> ExprKind {
    let mut rng = rand::rng();
    let op = random_rel_op();
    assert!(
        (real_vars.len() >= 2),
        "Need at least two variables for a difference logic constraint"
    );

    let mut vars = real_vars.to_owned();
    vars.shuffle(&mut rng);
    let v1 = AExpr::Var(vars[0].clone());
    let v2 = AExpr::Var(vars[1].clone());

    let diff = AExpr::BinOp {
        op: ArithOp::Sub,
        left: Box::new(v1),
        right: Box::new(v2),
    };

    let c = AExpr::Num(Ratio::new(rng.random_range(0..10), rng.random_range(1..10)));
    ExprKind::Rel {
        op,
        left: diff,
        right: c,
    }
}

fn random_linear(real_vars: &[VariableName]) -> ExprKind {
    let mut rng = rand::rng();
    let op = random_rel_op();

    let n_terms = rng.random_range(1..=real_vars.len().max(1));
    let mut vars = real_vars.to_owned();
    vars.shuffle(&mut rng);

    let mut sum = AExpr::Var(vars[0].clone());
    for v in vars.into_iter().skip(1).take(n_terms - 1) {
        sum = AExpr::BinOp {
            op: ArithOp::Add,
            left: Box::new(sum),
            right: Box::new(AExpr::Var(v)),
        };
    }

    let right = AExpr::Num(Ratio::new(
        rng.random_range(-10..10),
        rng.random_range(1..10),
    ));
    ExprKind::Rel {
        op,
        left: sum,
        right,
    }
}
