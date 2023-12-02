#!/bin/sh

# Install the llvm-tools-preview component
rustup component add llvm-tools-preview

# Ensure that the coverage is enabled
export RUSTFLAGS="-Cinstrument-coverage"

# Build the project
cargo build --all-features

# Ensure each test runs gets its own profile information 
export LLVM_PROFILE_FILE="autore-%p-%m.profraw"

# Run tests
cargo test

# Generate a coverage report from coverage artifacts
grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/

