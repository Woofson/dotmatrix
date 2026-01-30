#!/bin/bash
# Build script for dotmatrix

set -e

# Check if clean argument is provided
if [ "$1" = "clean" ]; then
    echo "🧹 Cleaning build artifacts..."
    cargo clean
    echo ""
    echo "✓ Clean complete!"
    echo ""
    echo "All build artifacts removed from ./target/"
    exit 0
fi

echo "🔨 Building dotmatrix..."
cargo build --release

echo ""
echo "✓ Build complete!"
echo ""
echo "Binary location: ./target/release/dotmatrix"
echo ""
echo "To install system-wide:"
echo "  cargo install --path ."
echo ""
echo "To run:"
echo "  ./target/release/dotmatrix --help"
echo "  or: cargo run -- --help"
echo ""
echo "To clean build artifacts:"
echo "  ./build.sh clean"
