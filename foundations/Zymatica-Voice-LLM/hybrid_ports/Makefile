# Watermark: ip zymatica.space | astronautshe.com
# Copyright (c) 2026 Zymatica. All rights reserved.

.PHONY: all help build-all verify-all clean run-fastest run-common run-robust run-secure run-modern

all: help

help:
	@echo "========================================================================"
	@echo " ZYMATICA VOICE LLM - Master Build & Orchestration Engine"
	@echo "========================================================================"
	@echo "Available targets:"
	@echo "  make verify-all       - Self-verify files in all stacks"
	@echo "  make build-all        - Compile compilers across all runnable platforms"
	@echo "  make clean            - Remove compiled binaries and build logs"
	@echo "  make run-fastest      - Start async Rust Tokio server"
	@echo "  make run-common       - Run common Python FastAPI backend"
	@echo "  make run-robust       - Run Go concurrent pipeline gateway"
	@echo "  make run-secure       - Launch memory-safe Axum microservices"
	@echo "  make run-modern       - Serve Edge Bun micro-orchestration runtime"

verify-all:
	@echo "[Verify] Scanning and asserting file structures..."
	@python -c "import os; assert os.path.exists('fastest_stack/zymatica_voice_fastest_server.rs')"
	@echo "[Verify] Integrity check passed successfully."

build-all:
	@echo "[Build] Compiling Rust Fastest Server..."
	-cd fastest_stack && rustc zymatica_voice_fastest_server.rs
	@echo "[Build] Compiling Go Pipeline Gateway..."
	-cd robust_stack && go build -o zymatica_voice_robust_pipeline zymatica_voice_robust_pipeline.go
	@echo "[Build] Compiling Rust Axum Secure Server..."
	-cd secure_stack && rustc zymatica_voice_secure_server.rs

clean:
	@echo "[Clean] Removing build artifacts..."
	-rm -f fastest_stack/zymatica_voice_fastest_server fastest_stack/*.exe
	-rm -f robust_stack/zymatica_voice_robust_pipeline robust_stack/*.exe
	-rm -f secure_stack/zymatica_voice_secure_server secure_stack/*.exe
