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

The main STL satisfiability checker. It reads an STL formula from the first line of
the input file and reports whether it is satisfiable.

```bash
stlsat [OPTIONS] <FORMULA_FILE>
```

For example:

```bash
stlsat --engine tableau --trace-extraction example.stl
```

The general options are:

| Option | Description |
| --- | --- |
| `--engine <tableau\|fol\|smt>` | Select the solving engine. `tableau` (the default) uses STLSat's tableau procedure; `fol` translates the formula to a first-order encoding; `smt` uses a bounded SMT encoding. The latter two encodings are solved with Z3. |
| `--mltl` | Interpret the input using finite-trace MLTL semantics. The default is STL semantics. |
| `--smtlib-result` | Print only `sat`, `unsat`, or `unknown`, instead of the engine-specific result and running time. |
| `--solver <auto\|z3\|dl>` | Choose the arithmetic solver used by the tableau engine. `auto` (the default) uses the built-in difference-logic solver when the constraints are supported and Z3 otherwise. `dl` forces the difference-logic solver; unsupported constraints then cause an error. |
| `-h`, `--help` | Print the command-line help. |

The remaining options apply only to the `tableau` engine:

| Option | Description |
| --- | --- |
| `--unsat-core-extraction` | Extract and print an unsatisfiable core when the result is unsatisfiable. |
| `--trace-extraction` | Extract and print a satisfying trace when one is found. |
| `--graph-output <FILE>` | Write the constructed tableau as a Graphviz DOT file. |
| `--max-depth <N>` | Limit tableau construction depth. The default is `1000000`; reaching the limit can produce an unknown result. |
| `--no-memoization` | Disable memoization of previously visited tableau states. |
| `--no-simple-first` | Disable the heuristic that processes simple formulas first. |
| `--no-formula-optimizations` | Disable syntactic optimizations used during tableau construction. |
| `--no-jump-rule` | Disable the tableau jump rule. |
| `--no-formula-simplifications` | Disable formula simplification before tableau construction. |

Run `stlsat --help` to see the options and their current defaults.

#### STL input syntax

An input file must contain one complete formula on its first line. Additional
lines are ignored. In particular, do not put a blank or comment line before the
formula.

The accepted syntax can be summarized as follows:

```text
formula     ::= implication
implication ::= disjunction (("->" | "<->") implication)?
disjunction ::= conjunction (("||" | "|") conjunction)*
conjunction ::= temporal (("&&" | "&") temporal)*
temporal    ::= unary (("U" | "R") interval unary)*
unary       ::= "!" unary
              | "X" unary
              | ("G" | "F") interval unary
              | "(" formula ")"
              | atom
atom        ::= "true" | "false" | identifier
              | arithmetic relation arithmetic
relation    ::= "<" | "<=" | ">" | ">=" | "==" | "!="
arithmetic  ::= term (("+" | "-") term)*
term        ::= number | identifier | "|" arithmetic "|"
              | "(" arithmetic ")"
interval    ::= "[" unsigned-integer "," unsigned-integer "]"
```

The formula operators are:

| Syntax | Meaning |
| --- | --- |
| `! p` | Negation |
| `p && q`, `p & q` | Conjunction |
| `p || q`, `p \| q` | Disjunction |
| `p -> q` | Implication |
| `p <-> q` | Equivalence |
| `X p` | Next (`F[1,1] p`) |
| `G[a,b] p` | Globally between offsets `a` and `b`, inclusive |
| `F[a,b] p` | Finally between offsets `a` and `b`, inclusive |
| `p U[a,b] q` | Until over the closed interval `[a,b]` |
| `p R[a,b] q` | Release over the closed interval `[a,b]` |

`G`, `F`, `X`, `U`, and `R` are uppercase and interval bounds are non-negative
integers. The interval must immediately follow its temporal operator: write
`G[0,5] p`, not `G [0,5] p`. Use bounds with `a <= b`.

Identifiers start with a letter and may then contain letters, digits, and
underscores, for example `request`, `x2`, or `sensor_ready`. Boolean constants
`true` and `false` are case-insensitive. Numeric literals may be integers,
non-negative decimals such as `0.75`, or exact fractions such as `3/4`.
Integers and fraction components may have a leading minus sign; write a negative
decimal as subtraction, for example `0 - 0.75`. Arithmetic expressions support
`+`, `-`, parentheses, and absolute value (`|x - y|`); multiplication and general
division are not part of the input language.

From highest to lowest, formula precedence is unary operators, `U`/`R`,
conjunction, disjunction, then implication/equivalence. `U`, `R`, conjunction,
and disjunction associate to the left; implication and equivalence associate to
the right. Parentheses are recommended whenever the intended grouping is not
obvious.

Some example inputs:

```text
G[0,20] (temperature <= 100)
```

```text
(request -> F[1,5] grant) && G[0,20] !(error)
```

```text
G[0,10] (|x - target| <= 3/4) && (ready U[2,8] done)
```

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
