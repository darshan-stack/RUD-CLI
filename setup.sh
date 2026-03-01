#!/bin/bash
# Quick start script for RUD-CLI

set -e

echo "RUD-CLI Production-Ready Setup"
echo "================================"
echo ""

# Build with all features
echo "Building RUD-CLI..."
cargo build --release

echo ""
echo " Build complete!"
echo ""

# Create config directory
CONFIG_DIR="$HOME/.config/rud"
mkdir -p "$CONFIG_DIR"

# Copy example config if not exists
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    echo "Creating default configuration..."
    cp config.example.toml "$CONFIG_DIR/config.toml"
    echo " Configuration created at $CONFIG_DIR/config.toml"
fi

echo ""
echo "Installation complete!"
echo ""
echo "Quick Start:"
echo "  1. Check status:    ./target/release/rud status"
echo "  2. Launch TUI:      ./target/release/rud tui"
echo "  3. Discover nodes:  ./target/release/rud scan"
echo ""
echo "For production use with real discovery:"
echo "  export RUD_REAL_DISCOVERY=true"
echo "  export ROS_DOMAIN_ID=0"
echo "  export MQTT_BROKER=localhost"
echo ""
echo "For LLM-powered remediation:"
echo "  export RUD_LLM_ENABLED=true"
echo "  export OPENAI_API_KEY=your-key-here"
echo ""
echo "See README.md for full documentation."
