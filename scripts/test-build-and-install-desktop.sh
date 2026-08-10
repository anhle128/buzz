#!/usr/bin/env bash
set -euo pipefail

[[ "$(uname -s)" == "Darwin" ]] || {
  echo "build-and-install desktop test skipped outside macOS"
  exit 0
}

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
grep -Fq 'pnpm install --frozen-lockfile --no-optimistic-repeat-install' \
  "$repo_root/scripts/build-and-install-desktop.sh"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

create_app() {
  local path=$1 id=$2 marker=$3
  mkdir -p "$path/Contents/MacOS" "$path/Contents/Resources"
  cat >"$path/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>buzz-desktop</string>
  <key>CFBundleIdentifier</key>
  <string>$id</string>
  <key>CFBundleShortVersionString</key>
  <string>0.0.0-test</string>
</dict>
</plist>
EOF
  cp /usr/bin/true "$path/Contents/MacOS/buzz-desktop"
  printf '%s\n' "$marker" >"$path/Contents/Resources/marker"
}

mkdir -p "$tmp/bin" "$tmp/source" "$tmp/install"
cat >"$tmp/bin/pgrep" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
chmod +x "$tmp/bin/pgrep"

source_app="$tmp/source/Buzz.app"
target_app="$tmp/install/Buzz.app"
create_app "$source_app" xyz.block.buzz.app new
create_app "$target_app" xyz.block.buzz.app old

PATH="$tmp/bin:$PATH" "$repo_root/scripts/build-and-install-desktop.sh" \
  --source "$source_app" --target "$target_app" --no-launch
[[ "$(<"$target_app/Contents/Resources/marker")" == new ]]
codesign --verify --deep --strict "$target_app"
for leftover in "$target_app".install.* "$target_app".rollback.*; do
  [[ ! -e "$leftover" ]] || { echo "temporary app was not cleaned up: $leftover" >&2; exit 1; }
done

create_app "$target_app" com.example.not-buzz untouched
if PATH="$tmp/bin:$PATH" "$repo_root/scripts/build-and-install-desktop.sh" \
  --source "$source_app" --target "$target_app" --no-launch >/dev/null 2>&1; then
  echo "installer replaced an app with the wrong bundle ID" >&2
  exit 1
fi
[[ "$(<"$target_app/Contents/Resources/marker")" == untouched ]]

echo "build-and-install desktop contract passed"
