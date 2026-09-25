#!/bin/sh
# Build an opt-in debug acceptance package. Does not change TCC, launch apps or record audio.
set -eu
test "$(uname -s)" = Darwin || { echo "This build requires macOS" >&2; exit 1; }
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"
if [ -z "${OPUS_LIB_DIR:-}" ] && command -v brew >/dev/null 2>&1; then
  OPUS_LIB_DIR=$(brew --prefix opus)
  export OPUS_LIB_DIR
fi
export OPUS_STATIC=1 VITE_ATDD=1
npm run tauri build -- --debug --features atdd --bundles app \
  --config '{"bundle":{"createUpdaterArtifacts":false,"macOS":{"signingIdentity":"-"}}}'
fixture_path="${TMPDIR:-/tmp}/ptt-atdd-speech.aiff"
/usr/bin/say -v Tingting -r 180 -o "$fixture_path" \
  '这是一段自动验收测试语音。我们正在验证麦克风录音，语音识别，以及跨应用自动输入。测试编号，一二三四五。'
printf 'Audio fixture: %s\nApp: %s/src-tauri/target/debug/bundle/macos/PushToTalk.app\n' "$fixture_path" "$project_dir"
