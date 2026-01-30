# Makefile for dotmatrix

.PHONY: all build release clean install uninstall run help test

# Default target
all: release

# Build in debug mode
build:
	@echo "🔨 Building dotmatrix (debug mode)..."
	@cargo build
	@echo "✓ Debug build complete: ./target/debug/dotmatrix"

# Build in release mode (optimized)
release:
	@echo "🔨 Building dotmatrix (release mode)..."
	@cargo build --release
	@echo "✓ Release build complete: ./target/release/dotmatrix"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@echo "✓ Clean complete!"

# Install to system
install: release
	@echo "📦 Installing dotmatrix..."
	@cargo install --path .
	@echo "✓ Installed! You can now run 'dotmatrix' from anywhere"

# Uninstall from system
uninstall:
	@echo "🗑️  Uninstalling dotmatrix..."
	@cargo uninstall dotmatrix
	@echo "✓ Uninstalled!"

# Run in development mode
run:
	@cargo run -- $(ARGS)

# Run tests
test:
	@echo "🧪 Running tests..."
	@cargo test

# Check code without building
check:
	@echo "🔍 Checking code..."
	@cargo check

# Format code
fmt:
	@echo "✨ Formatting code..."
	@cargo fmt

# Run clippy linter
lint:
	@echo "🔎 Running clippy..."
	@cargo clippy -- -D warnings

# Show help
help:
	@echo "dotmatrix - Makefile targets:"
	@echo ""
	@echo "  make build      - Build in debug mode"
	@echo "  make release    - Build in release mode (optimized)"
	@echo "  make clean      - Remove build artifacts"
	@echo "  make install    - Install to system (~/.cargo/bin/)"
	@echo "  make uninstall  - Remove from system"
	@echo "  make run ARGS='init' - Run with arguments"
	@echo "  make test       - Run tests"
	@echo "  make check      - Check code without building"
	@echo "  make fmt        - Format code with rustfmt"
	@echo "  make lint       - Run clippy linter"
	@echo "  make help       - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make              # Build release version"
	@echo "  make run ARGS='init'"
	@echo "  make run ARGS='scan'"
	@echo "  make install      # Install system-wide"
