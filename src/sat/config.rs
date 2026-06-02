use std::fmt::Display;

use clap::Parser;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum SolverEngine {
    #[default]
    Tableau,
    Fol,
    Smt,
}

impl Display for SolverEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolverEngine::Tableau => write!(f, "tableau"),
            SolverEngine::Fol => write!(f, "fol"),
            SolverEngine::Smt => write!(f, "smt"),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum SolverStrategy {
    Auto,
    #[default]
    Z3,
    DL,
}

impl Display for SolverStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolverStrategy::Auto => write!(f, "auto"),
            SolverStrategy::Z3 => write!(f, "z3"),
            SolverStrategy::DL => write!(f, "dl"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GeneralOptions {
    pub engine: SolverEngine,
    pub mltl: bool,
    pub smtlib_result: bool,
}

#[derive(Clone, Debug)]
pub struct TableauOptions {
    pub max_depth: usize,
    pub solver: SolverStrategy,
    pub graph_output: Option<String>,
    pub memoization: bool,
    pub simple_first: bool,
    pub formula_optimizations: bool,
    pub jump_rule_enabled: bool,
    pub formula_simplifications: bool,
    pub unsat_core_extraction: bool,
    pub trace_extraction: bool,
}

impl Default for TableauOptions {
    fn default() -> Self {
        TableauOptions {
            max_depth: 1000000,
            solver: SolverStrategy::Z3,
            graph_output: None,
            memoization: true,
            simple_first: true,
            formula_optimizations: true,
            jump_rule_enabled: true,
            formula_simplifications: true,
            unsat_core_extraction: false,
            trace_extraction: false,
        }
    }
}

#[derive(Parser)]
#[command(name = "stlsat")]
#[command(about = "STLSAT - Signal Temporal Logic Satisfiability Checker")]
pub struct CliArgs {
    /// Input formula file
    pub formula_file: String,

    #[arg(long, default_value_t = GeneralOptions::default().engine, help_heading = "General Options")]
    pub engine: SolverEngine,

    /// Enable FOL encoding
    #[arg(long, default_value_t = false, help_heading = "General Options")]
    pub fol: bool,

    /// Enable SMT encoding
    #[arg(long, default_value_t = false, help_heading = "General Options")]
    pub smt: bool,

    /// Use MLTL semantics
    #[arg(long, default_value_t = GeneralOptions::default().mltl, help_heading = "General Options")]
    pub mltl: bool,

    /// Print result in smtlib format
    #[arg(long, default_value_t = GeneralOptions::default().smtlib_result, help_heading = "General Options")]
    pub smtlib_result: bool,

    /// The solver to use
    #[arg(long, default_value_t = TableauOptions::default().solver, help_heading = "General Options")]
    pub solver: SolverStrategy,

    /// Enable unsat core extraction
    #[arg(long, default_value_t = TableauOptions::default().unsat_core_extraction, help_heading = "Tableau Options")]
    pub unsat_core_extraction: bool,

    /// Enable trace extraction
    #[arg(long, default_value_t = TableauOptions::default().trace_extraction, help_heading = "Tableau Options")]
    pub trace_extraction: bool,

    /// Output graph to file
    #[arg(long, help_heading = "Tableau Options")]
    pub graph_output: Option<String>,

    /// Maximum depth for tableau construction
    #[arg(long, default_value_t = TableauOptions::default().max_depth, help_heading = "Tableau Options")]
    pub max_depth: usize,

    /// Disable memoization
    #[arg(long = "no-memoization", action = clap::ArgAction::SetFalse, help_heading = "Tableau Options")]
    pub memoization: bool,

    /// Disable process simple formulas first
    #[arg(long = "no-simple-first", action = clap::ArgAction::SetFalse, help_heading = "Tableau Options")]
    pub simple_first: bool,

    /// Disable formula syntactic optimizations
    #[arg(long = "no-formula-optimizations", action = clap::ArgAction::SetFalse, help_heading = "Tableau Options")]
    pub formula_optimizations: bool,

    /// Disable jump rule
    #[arg(long = "no-jump-rule", action = clap::ArgAction::SetFalse, help_heading = "Tableau Options")]
    pub jump_rule_enabled: bool,

    /// Disable formula syntactic simplifications
    #[arg(long = "no-formula-simplifications", action = clap::ArgAction::SetFalse, help_heading = "Tableau Options")]
    pub formula_simplifications: bool,
}

pub enum ConfigSource {
    Cli,
}

#[must_use]
pub fn get_config(source: ConfigSource) -> (GeneralOptions, TableauOptions, String) {
    match source {
        ConfigSource::Cli => {
            let args = CliArgs::parse();

            let general = GeneralOptions {
                engine: args.engine,
                mltl: args.mltl,
                smtlib_result: args.smtlib_result,
            };

            let tableau = TableauOptions {
                max_depth: args.max_depth,
                solver: args.solver,
                graph_output: args.graph_output,
                memoization: args.memoization,
                simple_first: args.simple_first,
                formula_optimizations: args.formula_optimizations,
                jump_rule_enabled: args.jump_rule_enabled,
                formula_simplifications: args.formula_simplifications,
                unsat_core_extraction: args.unsat_core_extraction,
                trace_extraction: args.trace_extraction,
            };
            (general, tableau, args.formula_file)
        }
    }
}
