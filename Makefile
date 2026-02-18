# Makefile for dotmatrix

.PHONY: all build release clean install uninstall install-man run help test windows_release linux_release

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
	@echo "✓ Release build complete:"
	@echo "    ./target/release/dotmatrix  (CLI/TUI/GUI)"
	@echo "    ./target/release/dmgui      (GUI-only, no console on Windows)"

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
	@echo "💡 Run 'sudo make install-man' to install the man page"

# Install man page (requires sudo)
install-man:
	@echo "📖 Installing man page..."
	@install -d /usr/local/share/man/man1
	@install -m 644 dotmatrix.1 /usr/local/share/man/man1/
	@echo "✓ Man page installed! Try 'man dotmatrix'"

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

# Build Windows release: exe, zip, and Inno Setup installer
windows_release: release
	@echo "🪟 Building Windows release..."
	@$(eval VERSION := $(shell cargo metadata --no-deps --format-version 1 | python -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])"))
	@echo "   Version: $(VERSION)"

	@echo "   Creating release directory..."
	@mkdir -p release

	@echo "   Building zip archive..."
	@mkdir -p release/dotmatrix-$(VERSION)-windows-x86_64
	@cp target/release/dotmatrix.exe release/dotmatrix-$(VERSION)-windows-x86_64/
	@cp target/release/dmgui.exe release/dotmatrix-$(VERSION)-windows-x86_64/
	@cp README.md CHANGELOG.md LICENSE release/dotmatrix-$(VERSION)-windows-x86_64/
	@cp example-config.toml release/dotmatrix-$(VERSION)-windows-x86_64/
	@cd release && zip -r dotmatrix-$(VERSION)-windows-x86_64.zip dotmatrix-$(VERSION)-windows-x86_64/
	@rm -rf release/dotmatrix-$(VERSION)-windows-x86_64
	@echo "   ✓ Zip: release/dotmatrix-$(VERSION)-windows-x86_64.zip"

	@echo "   Compiling installer..."
	@iscc dotmatrix-installer.iss
	@echo "   ✓ Installer: release/dotmatrix-$(VERSION)-setup-windows-x86_64.exe"

	@echo "✓ Windows release complete!"
	@echo ""
	@echo "  release/dotmatrix-$(VERSION)-windows-x86_64.zip"
	@echo "  release/dotmatrix-$(VERSION)-setup-windows-x86_64.exe"
	@echo ""
	@echo "  Includes: dotmatrix.exe (CLI/TUI/GUI) + dmgui.exe (GUI-only)"

# Build Linux release tarball
linux_release: release
	@echo "🐧 Building Linux release..."
	@$(eval VERSION := $(shell cargo metadata --no-deps --format-version 1 | python -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])"))
	@echo "   Version: $(VERSION)"

	@echo "   Creating release directory..."
	@mkdir -p release

	@echo "   Building tarball..."
	@mkdir -p release/dotmatrix-$(VERSION)-linux-x86_64
	@cp target/release/dotmatrix release/dotmatrix-$(VERSION)-linux-x86_64/
	@cp target/release/dmgui release/dotmatrix-$(VERSION)-linux-x86_64/
	@cp README.md CHANGELOG.md LICENSE release/dotmatrix-$(VERSION)-linux-x86_64/
	@cp example-config.toml release/dotmatrix-$(VERSION)-linux-x86_64/
	@cp dotmatrix.desktop release/dotmatrix-$(VERSION)-linux-x86_64/
	@cd release && tar -czvf dotmatrix-$(VERSION)-linux-x86_64.tar.gz dotmatrix-$(VERSION)-linux-x86_64/
	@echo "   ✓ Tarball: release/dotmatrix-$(VERSION)-linux-x86_64.tar.gz"

	@echo "✓ Linux release complete!"
	@echo ""
	@echo "  release/dotmatrix-$(VERSION)-linux-x86_64.tar.gz"
	@echo ""
	@echo "  Includes: dotmatrix + dmgui + dotmatrix.desktop"

# Show help
help:
	@echo "dotmatrix - Makefile targets:"
	@echo ""
	@echo "  make build           - Build in debug mode"
	@echo "  make release         - Build in release mode (dotmatrix + dmgui)"
	@echo "  make clean           - Remove build artifacts"
	@echo "  make install         - Install binary to ~/.cargo/bin/"
	@echo "  make install-man     - Install man page (requires sudo)"
	@echo "  make uninstall       - Remove from system"
	@echo "  make run ARGS=''     - Run with arguments"
	@echo "  make test            - Run tests"
	@echo "  make check           - Check code without building"
	@echo "  make fmt             - Format code with rustfmt"
	@echo "  make lint            - Run clippy linter"
	@echo "  make linux_release   - Build Linux tarball in ./release/"
	@echo "  make windows_release - Build Windows zip + installer (requires Inno Setup)"
	@echo "  make help            - Show this help message"
	@echo ""
	@echo "Binaries:"
	@echo "  dotmatrix  - Full CLI/TUI/GUI (with console on Windows)"
	@echo "  dmgui      - GUI-only (no console window on Windows)"
	@echo ""
	@echo "Examples:"
	@echo "  make                   # Build release version"
	@echo "  make run ARGS='init'"
	@echo "  make install && sudo make install-man"
	@echo "  make linux_release     # Produces tarball in ./release/"
	@echo "  make windows_release   # Produces zip + installer in ./release/"
