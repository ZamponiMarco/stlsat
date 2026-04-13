use std::fs;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tracing_subscriber::prelude::*;

use stlsat::sat::config::{
    ConfigSource, ExecutionMode, GeneralOptions, TableauOptions, get_config,
};
use stlsat::sat::smt::SmtSolver;
use stlsat::sat::tableau::Tableau;
use stlsat::sat::tableau::node::NODE_ID;
use stlsat::util::join_with;

fn main() {
    init_logger();

    let _span =
        tracing::info_span!("main", trace_id = format!("{:x}", rand::random::<u128>())).entered();

    let (mode, options, tableau_options, filename) = get_config(ConfigSource::Cli);
    let file_content = fs::read_to_string(&filename).unwrap();
    let formula = file_content.lines().next().unwrap();

    tracing::info!(filename = %filename, "file_read");

    match mode {
        ExecutionMode::Fol => run_fol(formula, options),
        ExecutionMode::Tableau => run_tableau(formula, options, tableau_options),
    }
}

fn run_fol(example: &str, options: GeneralOptions) {
    let start = Instant::now();
    let mut smt_solver = SmtSolver::new(options);
    let res = smt_solver.make_smt_from_str(example);
    let duration = start.elapsed();

    tracing::info!(duration = %duration.as_secs_f64(), result = ?res, "fol_solved");

    if std::env::var("STLSAT_SILENT").as_deref() == Ok("1") {
        // Silent mode
    } else if smt_solver.options.smtlib_result {
        match res {
            Some(true) => println!("sat"),
            Some(false) => println!("unsat"),
            None => println!("unknown"),
        }
    } else {
        println!("FOL result: {res:?}");
        println!("DURATION_SEC: {:.6}", duration.as_secs_f64());
    }
}

fn run_tableau(example: &str, options: GeneralOptions, tableau_options: TableauOptions) {
    let start = Instant::now();
    let mut tableau = Tableau::new(options, tableau_options);
    let res = tableau.make_tableau_from_str(example);
    let duration = start.elapsed();

    tracing::info!(duration = %duration.as_secs_f64(), result = ?res, "tableau_solved");

    if std::env::var("STLSAT_SILENT").as_deref() == Ok("1") {
        return;
    } else if tableau.options.smtlib_result {
        match res {
            Some(true) => println!("sat"),
            Some(false) => println!("unsat"),
            None => println!("unknown"),
        }
    } else {
        println!("Tableau result: {res:?}");
        println!("DURATION_SEC: {:.6}", duration.as_secs_f64());
    }

    if let Some(filename) = &tableau.tableau_options.graph_output
        && let Some(graph) = &tableau.graph
        && let Ok(dot) = graph.to_dot_string()
    {
        let nodes = NODE_ID.load(Ordering::Relaxed) - 1;
        println!("Node count: {nodes}");
        fs::write(filename, &dot).expect("Unable to write file");
    }

    if let Some(core) = &tableau.unsat_core
        && matches!(res, Some(false))
    {
        println!(
            "Unsat core: {}",
            join_with(core.get_unsat_core().as_slice(), " && ")
        );
    }

    if let Some(trace) = &tableau.trace {
        println!("Trace length: {}", trace.length());
        println!("[");
        for (i, seq) in trace.full_trace().iter().enumerate() {
            let inner = seq
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let comma = if i + 1 < trace.full_trace().len() {
                ","
            } else {
                ""
            };
            println!("  [{inner}]{comma}");
        }
        println!("]");
    }
}

/// Initialize and configure tracing/logging.
/// Output is configured via the `RUST_LOG` environment variable, for example:
/// ```RUST_LOG=stlsat=trace cargo run --release -- <args>```
/// See <https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html>
fn init_logger() {
    let base_format_layer = tracing_subscriber::fmt::layer()
        // Disable colors
        .with_ansi(false)
        // Include target (module path)
        .with_target(true)
        // Log span close events, which will include attributes and duration
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE);

    // Depending on RUST_LOG_FORMAT env variable, choose between json or compact format.
    let format_layer = match std::env::var("RUST_LOG_FORMAT").as_deref() {
        Ok("json") => base_format_layer.json().boxed(),
        _ => base_format_layer.compact().boxed(),
    };

    tracing_subscriber::registry()
        .with(format_layer)
        // Make filter configurable via RUST_LOG env variable
        .with(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();
}
