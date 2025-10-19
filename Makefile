.PHONY: help build build-lsm build-locker schema codegen clean test check deploy-devnet

# Default target
help:
	@echo "Available targets:"
	@echo ""
	@echo "Build & Deploy:"
	@echo "  make build              - Build & optimize all contracts (WASM in artifacts/)"
	@echo "  make build-lsm          - Build & optimize lsm-staking contract only"
	@echo "  make build-locker       - Build & optimize proposal-option-locker only"
	@echo "  make deploy-devnet      - Deploy contracts to devnet"
	@echo ""
	@echo "Code Generation:"
	@echo "  make schema             - Generate JSON schemas"
	@echo "  make codegen            - Generate TypeScript clients"
	@echo "  make generate           - Generate schemas + TypeScript clients"
	@echo ""
	@echo "Development:"
	@echo "  make test               - Run all tests"
	@echo "  make check              - Check code without building"
	@echo "  make clean              - Clean build artifacts"
	@echo ""
	@echo "Quick workflow:"
	@echo "  make build && ./deploy-devnet.sh"

# Build & optimize all contracts using cosmwasm/optimizer (Docker)
build:
	@echo "Building and optimizing all contracts with cosmwasm/optimizer..."
	@if ! command -v docker &> /dev/null; then \
		echo "Error: Docker is required for building"; \
		exit 1; \
	fi
	@docker run --rm -v "$(PWD)":/code \
		--mount type=volume,source="$(notdir $(PWD))_cache",target=/target \
		--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
		cosmwasm/optimizer:0.16.0
	@echo "✓ Optimized WASM files available in artifacts/"
	@ls -lh artifacts/*.wasm 2>/dev/null || true

# Build & optimize individual contracts
build-lsm: artifacts
	@echo "Building and optimizing lsm-staking contract..."
	@cargo build --package lsm-staking --target wasm32-unknown-unknown --release
	@if command -v wasm-opt &> /dev/null; then \
		wasm-opt -Oz \
			target/wasm32-unknown-unknown/release/lsm_staking.wasm \
			-o artifacts/lsm_staking.wasm; \
		echo "✓ Optimized: artifacts/lsm_staking.wasm"; \
		ls -lh artifacts/lsm_staking.wasm; \
	else \
		cp target/wasm32-unknown-unknown/release/lsm_staking.wasm artifacts/; \
		echo "⚠ wasm-opt not found, copied unoptimized WASM"; \
		echo "  Install binaryen for optimization: apt install binaryen"; \
	fi

build-locker: artifacts
	@echo "Building and optimizing proposal-option-locker contract..."
	@cargo build --package proposal-option-locker --target wasm32-unknown-unknown --release
	@if command -v wasm-opt &> /dev/null; then \
		wasm-opt -Oz \
			target/wasm32-unknown-unknown/release/proposal_option_locker.wasm \
			-o artifacts/proposal_option_locker.wasm; \
		echo "✓ Optimized: artifacts/proposal_option_locker.wasm"; \
		ls -lh artifacts/proposal_option_locker.wasm; \
	else \
		cp target/wasm32-unknown-unknown/release/proposal_option_locker.wasm artifacts/; \
		echo "⚠ wasm-opt not found, copied unoptimized WASM"; \
		echo "  Install binaryen for optimization: apt install binaryen"; \
	fi

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

# Deploy to devnet
deploy-devnet:
	@if [ ! -f "artifacts/lsm_staking.wasm" ] || [ ! -f "artifacts/proposal_option_locker.wasm" ]; then \
		echo "Error: WASM files not found in artifacts/"; \
		echo "Run 'make build' first"; \
		exit 1; \
	fi
	@./deploy-devnet.sh

# Clean
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@rm -rf artifacts/*.wasm
	@rm -f deployment-info.json
	@echo "Clean complete!"

# Setup artifacts directory
artifacts:
	@mkdir -p artifacts
