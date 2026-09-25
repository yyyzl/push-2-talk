#!/bin/sh
set -eu
test "$(uname -s)" = Darwin || { echo "This test requires macOS frameworks" >&2; exit 1; }
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/ptt-native-tests.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT
xcrun clang -fobjc-arc -fblocks -Wno-deprecated-declarations \
  "$project_dir/src-tauri/tests/macos_native_keyboard.m" \
  -framework AppKit -framework ApplicationServices -framework AVFoundation -framework Carbon \
  -o "$test_dir/native-keyboard"
"$test_dir/native-keyboard"
