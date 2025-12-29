# stlsat

A satisfiability checker for Signal Temporal Logic (STL) formulas.

## Installation

1. Install Rust: https://rustup.rs/
2. Install Z3 theorem prover: The program requires Z3 executable to be installed in your system: https://github.com/Z3Prover/z3
3. Install the executables:

```bash
cargo install --git https://github.com/ZamponiMarco/stlsat.git
```

This installs three binaries: `stlsat` (main checker), `scanner` (scanner tool), and `rb` (random benchmark tool) to your Cargo bin directory (usually `~/.cargo/bin/`).

## Running

### stlsat

The main STL satisfiability checker. Takes a filename as an argument containing the STL formula to check for satisfiability.

```bash
stlsat <filename>
```

For example: `stlsat resources/formulas.stl`

Run `stlsat --help` for available options.

### scanner

Scans directories for STL files and processes them (e.g., parses and analyzes formulas). The output is a csv file with data about the formulas.

```bash
scanner [options]
```

Run `scanner --help` for available options.

### rb

Random benchmark generator. Generates random STL formulas and saves them to files.

```bash
rb [options]
```

Run `rb --help` for available options.

## Development

For development, clone the repository and use the [justfile](./justfile) for common tasks:

```bash
git clone https://github.com/ZamponiMarco/stlsat.git
cd stlsat
just  # See available tasks
```

The [justfile](./justfile) provides an overview of and easy access to common development tasks
like running linters and tests via the [just](https://github.com/casey/just) command runner.
After [installation](https://github.com/casey/just?tab=readme-ov-file#installation),
for example via `cargo install just`, run `just` to see the available tasks.

## Using as a Library

You can use `stlsat` as a Rust library in your projects for STL formula processing and satisfiability checking.

Add to your `Cargo.toml`:

```toml
[dependencies]
stlsat = { git = "https://github.com/ZamponiMarco/stlsat" }
```

Then in your Rust code:

```rust
use stlsat::formula;
use stlsat::sat;
use stlsat::util;
```

For API details, refer to the source code.

## Tracing

Parts of the program use the [`tracing`](https://crates.io/crates/tracing) and [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber) crates for structured logging.
You can configure the logging level by setting the `RUST_LOG` environment variable. For example:

```bash
RUST_LOG=stlsat=trace cargo run --release -- <args>
```

will enable trace-level logging for the `stlsat` crate.

By default, logs are output to `stderr` in a human-readable format.
To change the output format to JSON, you can set the `RUST_LOG_FORMAT` environment variable to `json`:

```bash
RUST_LOG_FORMAT=json RUST_LOG=stlsat=info cargo run --release -- <args>
```

The tracing subscriber is configured to log the `CLOSE` events of all spans, which can be useful for performance analysis since it captures the duration of operations.
However, enabling tracing has a sizeable performance overhead.

The root span includes a field `trace_id` which is a random unique identifier set at the start of the execution.
The field is inherited by all child spans to facilitate correlation of events belonging to the same execution if a centralized logging system is used.

Another environment variable that is useful for tracing is `STLSAT_SILENT`.
When set to `1`, the program suppresses standard output of results to prevent cluttering the logs.
