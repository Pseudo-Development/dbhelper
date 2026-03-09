.PHONY: setup build check test lint fmt fmt-check clippy pre-commit clean

## First-time setup: install pre-commit hooks
setup:
	prek install

## Build the entire workspace
build:
	cargo build --workspace

## Type-check without codegen (faster than build)
check:
	cargo check --workspace

## Run all tests
test:
	cargo test --workspace

## Run all lints (fmt check + clippy)
lint: fmt-check clippy

## Auto-format all code
fmt:
	cargo fmt --all

## Check formatting without modifying files
fmt-check:
	cargo fmt --all -- --check

## Run clippy with warnings as errors
clippy:
	cargo clippy --workspace -- -D warnings

## Run pre-commit hooks on all files
pre-commit:
	prek run --all-files

## Remove build artifacts
clean:
	cargo clean
