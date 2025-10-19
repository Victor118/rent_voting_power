.PHONY: help build build-all optimize optimize-all schema codegen clean test check

# Default target
help:
	@echo "Available targets:"
	@echo "  make build              - Build all contracts in debug mode"
	@echo "  make build-lsm          - Build lsm-staking contract"
	@echo "  make build-locker       - Build proposal-option-locker contract"
	@echo "  make optimize           - Build optimized WASM binaries for all contracts"
	@echo "  make optimize-lsm       - Build optimized WASM for lsm-staking"
	@echo "  make optimize-locker    - Build optimized WASM for proposal-option-locker"
	@echo "  make schema             - Generate JSON schemas for all contracts"
	@echo "  make codegen            - Generate TypeScript clients"
	@echo "  make generate           - Generate schemas + TypeScript clients"
	@echo "  make test               - Run all tests"
	@echo "  make check              - Check code without building"
	@echo "  make clean              - Clean build artifacts"

# Build targets
build:
	@echo "Building all contracts..."
	@cargo build --target wasm32-unknown-unknown --release

build-lsm:
	@echo "Building lsm-staking contract..."
	@cargo build --package lsm-staking --target wasm32-unknown-unknown --release

build-locker:
	@echo "Building proposal-option-locker contract..."
	@cargo build --package proposal-option-locker --target wasm32-unknown-unknown --release

# Optimize WASM binaries using cosmwasm/optimizer
optimize:
	@echo "Optimizing all contracts with cosmwasm/optimizer..."
	@if ! command -v docker &> /dev/null; then \
		echo "Error: Docker is required for optimization"; \
		exit 1; \
	fi
	@docker run --rm -v "$(PWD)":/code \
		--mount type=volume,source="$(notdir $(PWD))_cache",target=/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		cosmwasm/optimizer:0.16.0

optimize-lsm:
	@echo "Optimizing lsm-staking contract..."
	@cargo build --package lsm-staking --target wasm32-unknown-unknown --release
	@wasm-opt -Oz \
		target/wasm32-unknown-unknown/release/lsm_staking.wasm \
		-o artifacts/lsm_staking.wasm
	@echo "Optimized: artifacts/lsm_staking.wasm"

optimize-locker:
	@echo "Optimizing proposal-option-locker contract..."
	@cargo build --package proposal-option-locker --target wasm32-unknown-unknown --release
	@wasm-opt -Oz \
		target/wasm32-unknown-unknown/release/proposal_option_locker.wasm \
		-o artifacts/proposal_option_locker.wasm
	@echo "Optimized: artifacts/proposal_option_locker.wasm"

# Schema generation
schema:
	@echo "Generating JSON schemas..."
	@cd contracts/lsm-staking && cargo run --example schema
	@cd contracts/proposal-option-locker && cargo run --example schema
	@echo "Schemas generated successfully!"

# TypeScript codegen
codegen:
	@echo "Generating TypeScript clients..."
	@node codegen.js

generate: schema codegen

# Testing
test:
	@echo "Running tests..."
	@cargo test

check:
	@echo "Checking code..."
	@cargo check

# Clean
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@rm -rf artifacts/*.wasm
	@echo "Clean complete!"

# Setup artifacts directory
artifacts:
	@mkdir -p artifacts
