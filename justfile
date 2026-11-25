# Show available recipes
[private]
default:
    @just --list

# Static checks CI runs
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    just feature-check
    cargo machete
    node tools/style/no-iife.mjs

# Format everything
fmt:
    cargo fmt --all

# Check each feature independently, without workspace feature unification
feature-check:
    cargo clippy -p redn --all-targets --no-default-features -- -D warnings
    for feature in macros uuid chrono; do cargo clippy -p redn --all-targets --no-default-features --features "$feature" -- -D warnings || exit $?; done

# Run all tests across the workspace
unit:
    cargo test --workspace --all-features

# Exercise minimal and independently enabled capabilities
feature-test:
    cargo test -p redn --no-default-features --lib --tests
    for feature in macros uuid chrono; do cargo test -p redn --no-default-features --features "$feature" --lib --tests || exit $?; done

# Build documentation with warnings as errors
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# Run all test suites
test: unit feature-test

# Full CI check suite
check: lint test doc
