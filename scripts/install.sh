#!/usr/bin/env bash
# Rustify installer - Linux
# Safe to re-run. Set RUSTIFY_SRC to install from a local checkout.
set -euo pipefail

REPO_URL="https://github.com/fakerdoeslinux/Rustify.git"
SRC_DIR="${RUSTIFY_SRC:-$HOME/.local/src/Rustify}"

say() { printf '\n==> %s\n' "$*"; }

if ! command -v cargo >/dev/null 2>&1; then
  say "Rust is missing. Installing rustup (minimal profile)."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

# Running from a checkout? Use it. Otherwise clone/fetch the repo.
if [[ ! -f Cargo.toml ]] || ! grep -q '^name = "rustify"' Cargo.toml; then
  say "Getting the source."
  if [[ ! -d "$SRC_DIR/.git" ]]; then
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone "$REPO_URL" "$SRC_DIR"
  fi
  cd "$SRC_DIR"
else
  say "Using local checkout: $(pwd)"
fi

# ALSA headers are needed to build librespot's default rodio backend.
if command -v sudo >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    say "Installing build dependencies (apt)."
    sudo apt-get update -qq
    sudo apt-get install -y -qq build-essential libasound2-dev pkg-config
  elif command -v dnf >/dev/null 2>&1; then
    say "Installing build dependencies (dnf)."
    sudo dnf install -y gcc make alsa-lib-devel
  elif command -v pacman >/dev/null 2>&1; then
    say "Installing build dependencies (pacman)."
    sudo pacman -S --needed --noconfirm base-devel alsa-lib
  fi
fi

say "Installing librespot (the actual audio engine)."
cargo install librespot --locked

say "Installing rustify."
cargo install --path . --locked

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/rustify"
if [[ ! -f "$CONFIG_DIR/config.toml" ]]; then
  say "Writing your first config."
  rustify init
fi

say "Done. Next:"
echo "  rustify login    # one-time browser sign-in"
echo "  rustify doctor   # check everything"
echo "  rustify start    # become a Spotify Connect speaker"
echo "Then open Spotify on your phone and pick 'Rustify' under Connect."
