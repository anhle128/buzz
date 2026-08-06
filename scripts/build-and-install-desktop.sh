#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bundle_id="xyz.block.buzz.app"
source_app="$repo_root/desktop/src-tauri/target/release/bundle/macos/Buzz.app"
target_app="/Applications/Buzz.app"
build=true
launch=true

usage() {
  cat <<'EOF'
Build Buzz Desktop and replace the locally installed macOS app.

Usage: scripts/build-and-install-desktop.sh [options]

Options:
  --source <Buzz.app>  Install an existing app bundle instead of building.
  --target <Buzz.app>  Install location (default: /Applications/Buzz.app).
  --no-launch          Do not launch Buzz after installation.
  -h, --help           Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source)
      [[ $# -ge 2 ]] || { echo "Error: --source requires a path." >&2; exit 2; }
      source_app=$2
      build=false
      shift 2
      ;;
    --target)
      [[ $# -ge 2 ]] || { echo "Error: --target requires a path." >&2; exit 2; }
      target_app=$2
      shift 2
      ;;
    --no-launch)
      launch=false
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || {
  echo "Error: local desktop replacement is supported only on macOS." >&2
  exit 1
}

absolute_app_path() {
  local path=$1 parent name
  parent=$(dirname "$path")
  name=$(basename "$path")
  [[ "$name" == "Buzz.app" ]] || {
    echo "Error: app path must end in Buzz.app: $path" >&2
    return 1
  }
  [[ -d "$parent" ]] || {
    echo "Error: app parent directory does not exist: $parent" >&2
    return 1
  }
  printf '%s/%s\n' "$(cd "$parent" && pwd -P)" "$name"
}

validate_app() {
  local app=$1 label=$2 plist executable actual_id
  plist="$app/Contents/Info.plist"
  [[ -f "$plist" ]] || { echo "Error: $label has no Info.plist: $app" >&2; return 1; }
  actual_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist" 2>/dev/null) || {
    echo "Error: cannot read $label bundle ID: $app" >&2
    return 1
  }
  [[ "$actual_id" == "$bundle_id" ]] || {
    echo "Error: refusing $label with bundle ID '$actual_id' (expected '$bundle_id')." >&2
    return 1
  }
  executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$plist" 2>/dev/null) || {
    echo "Error: cannot read $label executable: $app" >&2
    return 1
  }
  [[ "$executable" != */* && -x "$app/Contents/MacOS/$executable" ]] || {
    echo "Error: $label executable is missing or not executable: $executable" >&2
    return 1
  }
}

ensure_valid_signature() {
  local app=$1
  if codesign --verify --deep --strict "$app" >/dev/null 2>&1; then
    return 0
  fi
  codesign --force --deep --sign - \
    --entitlements "$repo_root/desktop/src-tauri/Entitlements.plist" \
    "$app" >/dev/null
  codesign --verify --deep --strict "$app"
}

build_app() {
  cd "$repo_root"
  # shellcheck source=/dev/null
  source "$repo_root/bin/activate-hermit"
  pnpm install --frozen-lockfile
  cargo build --release \
    -p buzz-acp \
    -p buzz-agent \
    -p buzz-backend-kubernetes \
    -p buzz-dev-mcp \
    -p git-credential-nostr \
    -p buzz-cli
  "$repo_root/scripts/bundle-sidecars.sh"
  export MESH_LLM_NATIVE_RUNTIME_CACHE_DIR
  MESH_LLM_NATIVE_RUNTIME_CACHE_DIR=$("$repo_root/scripts/ensure-mesh-native-runtime.sh")
  export CMAKE_POLICY_VERSION_MINIMUM=3.5
  export MACOSX_DEPLOYMENT_TARGET=10.15
  export CMAKE_OSX_DEPLOYMENT_TARGET=10.15
  (
    cd "$repo_root/desktop"
    pnpm tauri build --no-sign --bundles app --features mesh-llm
  )
}

wait_for_app_to_quit() {
  pgrep -x buzz-desktop >/dev/null 2>&1 || return 0
  osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null
  for _ in {1..40}; do
    pgrep -x buzz-desktop >/dev/null 2>&1 || return 0
    sleep 0.25
  done
  echo "Error: Buzz is still running. Close it and run the script again." >&2
  return 1
}

if [[ "$build" == true ]]; then
  build_app
fi

source_app=$(absolute_app_path "$source_app")
target_app=$(absolute_app_path "$target_app")
[[ ! -L "$target_app" ]] || { echo "Error: refusing to replace symlink: $target_app" >&2; exit 1; }
validate_app "$source_app" "source app"
if [[ -e "$target_app" ]]; then
  validate_app "$target_app" "installed app"
fi

staged_app="${target_app}.install.$$"
rollback_app="${target_app}.rollback.$$"
cleanup() {
  [[ ! -e "$staged_app" ]] || rm -rf -- "$staged_app"
  if [[ -e "$rollback_app" ]]; then
    [[ ! -e "$target_app" ]] || rm -rf -- "$target_app"
    mv "$rollback_app" "$target_app"
  fi
}
trap cleanup EXIT INT TERM

ditto "$source_app" "$staged_app"
validate_app "$staged_app" "staged app"
ensure_valid_signature "$staged_app"
wait_for_app_to_quit

if [[ -e "$target_app" ]]; then
  mv "$target_app" "$rollback_app"
fi
if ! mv "$staged_app" "$target_app"; then
  echo "Error: install failed; restoring the previous app." >&2
  exit 1
fi
if ! validate_app "$target_app" "installed app"; then
  rm -rf -- "$target_app"
  echo "Error: installed app validation failed; restoring the previous app." >&2
  exit 1
fi
[[ ! -e "$rollback_app" ]] || rm -rf -- "$rollback_app"
trap - EXIT INT TERM

version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$target_app/Contents/Info.plist")
echo "Installed Buzz $version at $target_app"
if [[ "$launch" == true ]]; then
  open "$target_app"
fi
