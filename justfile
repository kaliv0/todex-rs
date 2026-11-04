_default:
    @just --list

run *args="":
    cargo run -- {{args}}

check:
    cargo check

fix:
    cargo fix --allow-dirty

fmt:
    cargo fmt

lint:
    cargo clippy --fix --allow-dirty

test:
    cargo test

build-release:
    cargo build --release

install:
    cargo install --path .

publish: all
    cargo publish #--allow-dirty

clean:
    cargo clean
    rm -rf dist

all: fix fmt lint test
