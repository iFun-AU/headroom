#!/bin/sh
# Builds, ad-hoc signs and verifies the local universal Headroom app.
#
# Usage: scripts/build-app.sh [--no-oauth] [--icons] [--clean] [--install]
#   --no-oauth  build without the opt-in Claude usage API (claude-oauth feature)
#   --icons     regenerate bundle icons from src-tauri/icons/app-icon.svg
#               (requires rsvg-convert, e.g. `brew install librsvg`)
#   --clean     remove build caches and temp files first (full rebuild)
#   --install   replace /Applications/Headroom.app and launch it
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
target="universal-apple-darwin"
app="$repo_dir/target/$target/release/bundle/macos/Headroom.app"
installed="/Applications/Headroom.app"

oauth=1
icons=0
clean=0
install=0
for argument in "$@"; do
  case "$argument" in
    --no-oauth) oauth=0 ;;
    --icons) icons=1 ;;
    --clean) clean=1 ;;
    --install) install=1 ;;
    -h|--help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $argument (see --help)" >&2; exit 2 ;;
  esac
done

step() { printf '\n==> %s\n' "$1"; }
fail() { echo "error: $1" >&2; exit 1; }

cd "$repo_dir"

step "Checking prerequisites"
command -v cargo >/dev/null || fail "cargo not found; install Rust from https://rustup.rs"
cargo tauri --version >/dev/null 2>&1 || fail "Tauri CLI not found; run: cargo install tauri-cli --version '^2'"
command -v npm >/dev/null || fail "npm not found; install Node.js"
for rust_target in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -qx "$rust_target" \
    || fail "Rust target $rust_target missing; run: rustup target add $rust_target"
done

if [ "$clean" -eq 1 ]; then
  step "Cleaning build caches and temp files"
  cargo clean
  rm -rf ui/dist src-tauri/binaries
  find . -name .DS_Store -not -path './.git/*' -not -path './ui/node_modules/*' -delete
fi

if [ ! -d ui/node_modules ]; then
  step "Installing UI dependencies"
  npm --prefix ui ci
fi

if [ "$icons" -eq 1 ]; then
  step "Regenerating app icons"
  command -v rsvg-convert >/dev/null || fail "rsvg-convert not found; run: brew install librsvg"
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
  rsvg-convert -w 1024 -h 1024 src-tauri/icons/app-icon.svg -o "$work/icon-1024.png"
  cargo tauri icon "$work/icon-1024.png" -o "$work/out" >/dev/null
  for icon in 32x32.png 128x128.png 128x128@2x.png icon.icns icon.png; do
    cp "$work/out/$icon" src-tauri/icons/
  done
fi

step "Building $target"
if [ "$oauth" -eq 1 ]; then
  cargo tauri build --target "$target" --features claude-oauth
else
  cargo tauri build --target "$target"
fi

step "Signing (ad-hoc)"
codesign --force --deep -s - "$app"
codesign --verify --deep --strict --verbose=2 "$app"

step "Verifying architectures"
for executable in headroom headroom-statusline; do
  archs=$(lipo -archs "$app/Contents/MacOS/$executable")
  case "$archs" in
    *x86_64*arm64*|*arm64*x86_64*) echo "$executable: $archs" ;;
    *) fail "$executable is not universal ($archs)" ;;
  esac
done

if [ "$install" -eq 1 ]; then
  step "Installing to /Applications"
  if pgrep -x headroom >/dev/null; then
    osascript -e 'tell application id "dev.headroom.app" to quit' >/dev/null 2>&1 || true
    attempts=0
    while pgrep -x headroom >/dev/null && [ "$attempts" -lt 20 ]; do
      sleep 0.5
      attempts=$((attempts + 1))
    done
    pgrep -x headroom >/dev/null && fail "Headroom is still running; quit it and retry"
  fi
  rm -rf "$installed"
  ditto "$app" "$installed"
  open "$installed"
  echo "Installed and launched: $installed"
else
  echo "Built and signed: $app"
fi
