#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
output_dir="$repo_dir/src-tauri/binaries"
binary_name="howisit-statusline"
target_triple=${1:-${TAURI_ENV_TARGET_TRIPLE:-}}

if [ -z "$target_triple" ]; then
  target_triple=$(rustc -vV | sed -n 's/^host: //p')
fi

if [ -z "$target_triple" ]; then
  echo "Unable to determine the Rust target triple" >&2
  exit 1
fi

mkdir -p "$output_dir"

if [ "$target_triple" = "universal-apple-darwin" ]; then
  arm_target="aarch64-apple-darwin"
  intel_target="x86_64-apple-darwin"
  cargo build --release -p statusline-bridge --target "$arm_target"
  cargo build --release -p statusline-bridge --target "$intel_target"
  install -m 755 \
    "$repo_dir/target/$arm_target/release/$binary_name" \
    "$output_dir/$binary_name-$arm_target"
  install -m 755 \
    "$repo_dir/target/$intel_target/release/$binary_name" \
    "$output_dir/$binary_name-$intel_target"
  lipo -create \
    "$output_dir/$binary_name-$arm_target" \
    "$output_dir/$binary_name-$intel_target" \
    -output "$output_dir/$binary_name-$target_triple"
else
  cargo build --release -p statusline-bridge --target "$target_triple"
  install -m 755 \
    "$repo_dir/target/$target_triple/release/$binary_name" \
    "$output_dir/$binary_name-$target_triple"
fi
