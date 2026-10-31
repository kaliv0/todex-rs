_default:
    @just --list

run *args="":
    cargo run -- {{args}}

check:
    cargo check

fmt:
    cargo fmt

lint:
    cargo clippy --all-targets -- -D warnings

test:
    cargo test
build-release:
    cargo build --release

install:
    cargo install --path .

clean:
    cargo clean
    rm -rf dist

all: fmt lint test
