[private]
default:
    just --list --unsorted

[group('build')]
[doc('Build the project in release mode')]
release:
    cargo build --release

[group('build')]
[doc('Build the project with profile "profiling", that is release mode with debug info')]
build-profiling:
    cargo build --profile profiling

[group('run')]
[doc('Execute `cargo run` with provided arguments')]
run *args:
    cargo run -- {{args}}

[group('run')]
[doc('Execute `cargo run --release` with provided arguments')]
run-release *args:
    cargo run --release -- {{args}}

[group('test')]
[doc('Run `cargo test` with provided arguments')]
test *args:
    cargo test {{args}}

[group('lint')]
[doc('Format code using `cargo fmt`')]
format:
    cargo fmt

[group('lint')]
[doc('Check for errors using `cargo check`')]
check:
    cargo check

[group('lint')]
[doc('Run clippy linter')]
clippy:
    cargo clippy

[group('lint')]
[doc('Run clippy linter and automatically fix issues')]
clippy-fix:
    cargo clippy --fix

[group('lint')]
[doc('Run clippy linter in pedantic mode')]
clippy-pedantic:
    cargo clippy -- -W clippy::pedantic

[group('lint')]
[doc('Run clippy linter in pedantic mode and automatically fix issues')]
clippy-pedantic-fix:
    cargo clippy --fix -- -W clippy::pedantic