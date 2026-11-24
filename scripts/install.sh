#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

cargo install librespot
cargo install --path .
rustify init

echo "edit ~/.config/rustify/config.toml, then:"
echo "  export RUSTIFY_SPOTIFY_PASSWORD='your password'"
echo "  rustify doctor"
echo "  rustify start"
