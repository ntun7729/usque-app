#!/usr/bin/env bash
# Build and verify only the arm64-v8a (aarch64-linux-android) Android APK.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

pwsh -NoProfile -File ./tool/build_android_rust.ps1 -AbiFilter arm64-v8a -CargoAction clippy
pwsh -NoProfile -File ./tool/build_android_rust.ps1 -AbiFilter arm64-v8a -Release

cd "$ROOT_DIR/apps/usque_gui"
flutter --suppress-analytics pub get --enforce-lockfile
flutter --suppress-analytics build apk \
  --release \
  --split-per-abi \
  --target-platform android-arm64 \
  --no-pub \
  --split-debug-info=build/symbols/android-arm64-v8a
