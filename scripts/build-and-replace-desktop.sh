#!/usr/bin/env bash
# Build an unsigned local Buzz Desktop bundle and replace the installed macOS app.
#
# This is intended for developer/tester machines that need to dogfood a local
# source checkout before a signed release is available. It preserves app data;
# only the /Applications/Buzz.app bundle is replaced.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

APP_NAME="Buzz.app"
INSTALL_DIR="/Applications"
TARGET=""
DRY_RUN=0
SKIP_BUILD=0
QUIT_APP=1
BACKUP_EXISTING=1
OPEN_AFTER=0

usage() {
  cat <<'USAGE'
Usage: scripts/build-and-replace-desktop.sh [options]

Build an unsigned local Buzz Desktop bundle, quit the running app, back up the
installed app bundle, and install the freshly built bundle in its place.

Options:
  --target <triple>      Rust/Tauri target triple. Defaults to `rustc -vV` host.
                         Example: aarch64-apple-darwin or x86_64-apple-darwin.
  --install-dir <path>   Directory containing Buzz.app. Default: /Applications.
  --app-name <name>      App bundle name. Default: Buzz.app.
  --skip-build           Install an already-built bundle from target/<triple>/...
  --dry-run              Print what would happen without building or installing.
  --no-quit              Do not ask a running Buzz app to quit first.
  --no-backup            Remove the old app instead of moving it to a timestamped backup.
  --open                 Open the installed app after replacement.
  -h, --help             Show this help.

Examples:
  scripts/build-and-replace-desktop.sh
  scripts/build-and-replace-desktop.sh --target aarch64-apple-darwin --open
  scripts/build-and-replace-desktop.sh --skip-build --install-dir "$HOME/Applications"
USAGE
}

log() {
  printf '==> %s\n' "$*"
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

run() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '+ '
    printf '%q ' "$@"
    printf '\n'
    return 0
  fi
  "$@"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      [[ $# -ge 2 ]] || fail "--target requires a value"
      TARGET="$2"
      shift 2
      ;;
    --install-dir)
      [[ $# -ge 2 ]] || fail "--install-dir requires a value"
      INSTALL_DIR="$2"
      shift 2
      ;;
    --app-name)
      [[ $# -ge 2 ]] || fail "--app-name requires a value"
      APP_NAME="$2"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --no-quit)
      QUIT_APP=0
      shift
      ;;
    --no-backup)
      BACKUP_EXISTING=0
      shift
      ;;
    --open)
      OPEN_AFTER=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || fail "this script only replaces the macOS desktop app"

export PATH="${REPO_ROOT}/bin:${PATH}"

if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -vV | sed -n 's/^host: //p')"
fi
[[ -n "$TARGET" ]] || fail "could not determine host target from rustc -vV; pass --target"
[[ "$TARGET" == *-apple-darwin ]] || fail "target must be a macOS target, got: $TARGET"

SOURCE_APP="${REPO_ROOT}/desktop/src-tauri/target/${TARGET}/release/bundle/macos/${APP_NAME}"
DEST_APP="${INSTALL_DIR%/}/${APP_NAME}"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP_APP="${DEST_APP}.backup-${TIMESTAMP}"
BACKUP_CREATED=""

cd "$REPO_ROOT"

log "Repo: $REPO_ROOT"
log "Target: $TARGET"
log "Source app: $SOURCE_APP"
log "Destination app: $DEST_APP"

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "Dry run: skipping build and install side effects"
else
  [[ -x "${REPO_ROOT}/bin/just" || -n "$(command -v just || true)" ]] || fail "just is required; run from a checkout with Hermit bin/ available"
fi

if [[ "$SKIP_BUILD" -eq 0 && "$DRY_RUN" -eq 0 ]]; then
  log "Building unsigned local desktop bundle"
  run just desktop-release-build "$TARGET"
elif [[ "$SKIP_BUILD" -eq 1 ]]; then
  log "Skipping build; using existing bundle"
fi

if [[ "$DRY_RUN" -eq 0 && ! -d "$SOURCE_APP" ]]; then
  fail "built app not found at $SOURCE_APP"
fi

if [[ "$QUIT_APP" -eq 1 ]]; then
  log "Quitting running Buzz app if present"
  run osascript -e 'tell application "Buzz" to quit' >/dev/null 2>&1 || true
  if [[ "$DRY_RUN" -eq 0 ]]; then
    for _ in {1..20}; do
      if ! pgrep -x "Buzz" >/dev/null 2>&1; then
        break
      fi
      sleep 0.5
    done
    if pgrep -x "Buzz" >/dev/null 2>&1; then
      fail "Buzz is still running; quit it manually or pass --no-quit if you know it is safe"
    fi
  fi
fi

log "Installing replacement app bundle"
run mkdir -p "$INSTALL_DIR"

if [[ -e "$DEST_APP" ]]; then
  if [[ "$BACKUP_EXISTING" -eq 1 ]]; then
    if [[ "$DRY_RUN" -eq 0 ]]; then
      backup_candidate="$BACKUP_APP"
      backup_index=1
      while [[ -e "$backup_candidate" ]]; do
        backup_candidate="${BACKUP_APP}.${backup_index}"
        backup_index=$((backup_index + 1))
      done
      BACKUP_APP="$backup_candidate"
    fi
    log "Moving existing app to $BACKUP_APP"
    run mv "$DEST_APP" "$BACKUP_APP"
    BACKUP_CREATED="$BACKUP_APP"
  else
    log "Removing existing app without backup"
    run rm -rf "$DEST_APP"
  fi
fi

if ! run ditto "$SOURCE_APP" "$DEST_APP"; then
  if [[ "$DRY_RUN" -eq 0 && "$BACKUP_EXISTING" -eq 1 && -d "$BACKUP_APP" && ! -e "$DEST_APP" ]]; then
    printf 'restore: moving backup back into place\n' >&2
    mv "$BACKUP_APP" "$DEST_APP"
  fi
  fail "failed to copy replacement app"
fi

if [[ "$DRY_RUN" -eq 0 ]]; then
  xattr -dr com.apple.quarantine "$DEST_APP" >/dev/null 2>&1 || true
fi

if [[ "$OPEN_AFTER" -eq 1 ]]; then
  log "Opening $DEST_APP"
  run open "$DEST_APP"
fi

log "Installed local Buzz Desktop build at $DEST_APP"
if [[ -n "$BACKUP_CREATED" ]]; then
  log "Previous app backup: $BACKUP_CREATED"
fi
